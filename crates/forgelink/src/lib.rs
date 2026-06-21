mod forge;
mod remote;

pub use forge::Forge;

use std::num::NonZeroU32;
use std::path::Path;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no git repository found")]
    RepositoryNotFound(#[source] BoxError),
    #[error("no '{0}' remote found")]
    NoRemote(String),
    #[error("invalid remote URL: {0}")]
    InvalidRemoteUrl(String),
    #[error("unrecognized forge: {0}")]
    UnknownForge(String),
    #[error("bare repositories are not supported")]
    BareRepository,
    #[error("could not resolve HEAD to a commit")]
    NoCommit(#[source] BoxError),
    #[error("{0} is not inside the repository")]
    FileOutsideRepository(String),
    #[error("path is not valid UTF-8")]
    NonUtf8Path,
    #[error("HEAD is detached; use a commit SHA instead")]
    DetachedHead,
    #[error("line range end ({end}) is before start ({start})")]
    InvalidLineRange { start: NonZeroU32, end: NonZeroU32 },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum GitRef {
    Branch(String),
    Commit(String),
}

#[derive(Debug, Clone, Default)]
pub enum RefSpec {
    #[default]
    Commit,
    Branch,
}

#[derive(Debug, Clone)]
pub enum Lines {
    #[non_exhaustive]
    Single(NonZeroU32),
    #[non_exhaustive]
    Range(NonZeroU32, NonZeroU32),
}

impl Lines {
    pub fn single(line: NonZeroU32) -> Self {
        Lines::Single(line)
    }

    pub fn range(start: NonZeroU32, end: NonZeroU32) -> Result<Self> {
        if end < start {
            return Err(Error::InvalidLineRange { start, end });
        }
        Ok(Lines::Range(start, end))
    }

    pub fn start(&self) -> NonZeroU32 {
        match self {
            Lines::Single(n) | Lines::Range(n, _) => *n,
        }
    }

    pub fn end(&self) -> NonZeroU32 {
        match self {
            Lines::Single(n) | Lines::Range(_, n) => *n,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinkRequest {
    pub host: String,
    pub dir: String,
    pub file: String,
    pub git_ref: GitRef,
    pub lines: Option<Lines>,
}

pub fn detect_forge(host: &str) -> Option<impl Forge + use<>> {
    forge::detect(host)
}

pub fn resolve_ref(path: &Path) -> Result<GitRef> {
    let repo = remote::discover(path)?;
    remote::head_commit(&repo)
}

pub fn current_branch(path: &Path) -> Result<GitRef> {
    let repo = remote::discover(path)?;
    remote::current_branch(&repo)
}

pub fn project_link(path: &Path, remote_name: &str) -> Result<String> {
    let repo = remote::discover(path)?;
    let (host, dir) = remote::remote(&repo, remote_name)?;
    let forge = detect_forge(&host).ok_or_else(|| Error::UnknownForge(host.clone()))?;
    Ok(forge.project_url(&host, &dir))
}

pub fn build_link(
    path: &Path,
    remote_name: &str,
    file: &str,
    lines: Option<Lines>,
    git_ref: RefSpec,
) -> Result<String> {
    let file_path = Path::new(file);
    let absolute = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        path.join(file_path)
    };

    let discovery_start = nearest_existing_dir(&absolute).ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{file}: no existing directory to search for a repository"),
        ))
    })?;
    let repo = remote::discover(discovery_start)?;

    let (host, dir) = remote::remote(&repo, remote_name)?;
    let root = remote::root(&repo)?;
    let git_ref = match git_ref {
        RefSpec::Commit => remote::head_commit(&repo)?,
        RefSpec::Branch => remote::current_branch(&repo)?,
    };

    let canonical = match absolute.parent() {
        Some(parent) => canonicalize_lenient(parent).join(absolute.file_name().unwrap_or_default()),
        None => absolute,
    };

    let relative = canonical
        .strip_prefix(&root)
        .map_err(|_| Error::FileOutsideRepository(file.to_string()))?
        .components()
        .map(|c| c.as_os_str().to_str().ok_or(Error::NonUtf8Path))
        .collect::<Result<Vec<_>>>()?
        .join("/");

    let forge = detect_forge(&host).ok_or_else(|| Error::UnknownForge(host.clone()))?;
    let req = LinkRequest {
        host,
        dir,
        file: relative,
        git_ref,
        lines,
    };
    Ok(forge.file_url(&req))
}

fn nearest_existing_dir(path: &Path) -> Option<&Path> {
    path.ancestors().find(|p| p.is_dir())
}

fn canonicalize_lenient(path: &Path) -> std::path::PathBuf {
    let mut suffix = Vec::new();
    let mut current = path;
    loop {
        if let Ok(base) = current.canonicalize() {
            let mut result = base;
            while let Some(name) = suffix.pop() {
                result.push(name);
            }
            return result;
        }
        match (current.file_name(), current.parent()) {
            (Some(name), Some(parent)) => {
                suffix.push(name.to_owned());
                current = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    fn init_repo(url: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        gix::init(dir.path()).unwrap();
        let config = dir.path().join(".git").join("config");
        let mut f = fs::OpenOptions::new().append(true).open(config).unwrap();
        writeln!(
            f,
            "[user]\n\tname = Test\n\temail = test@example.com\n[remote \"origin\"]\n\turl = {url}"
        )
        .unwrap();
        dir
    }

    fn commit_empty(dir: &Path) {
        let repo = gix::open(dir).unwrap();
        let tree = repo
            .write_object(gix::objs::Tree::default())
            .unwrap()
            .detach();
        repo.commit("HEAD", "init", tree, gix::commit::NO_PARENT_IDS)
            .unwrap();
    }

    #[test]
    fn nearest_existing_dir_walks_up_past_missing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("a").join("b").join("file.rs");
        assert_eq!(nearest_existing_dir(&missing), Some(dir.path()));
    }

    #[test]
    fn nearest_existing_dir_skips_the_file_itself() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.rs");
        fs::write(&file, "").unwrap();
        assert_eq!(nearest_existing_dir(&file), Some(dir.path()));
    }

    #[test]
    fn canonicalize_lenient_resolves_existing() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("a").join("b");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(canonicalize_lenient(&sub), sub.canonicalize().unwrap());
    }

    #[test]
    fn canonicalize_lenient_appends_missing_tail() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("a");
        fs::create_dir_all(&existing).unwrap();
        let missing = existing.join("b").join("c");
        let expected: PathBuf = existing.canonicalize().unwrap().join("b").join("c");
        assert_eq!(canonicalize_lenient(&missing), expected);
    }

    #[test]
    fn build_link_end_to_end() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();

        let lines = Lines::single(NonZeroU32::new(3).unwrap());
        let url = build_link(
            dir.path(),
            "origin",
            "src/main.rs",
            Some(lines),
            RefSpec::Commit,
        )
        .unwrap();

        assert!(
            url.starts_with("https://github.com/user/repo/blob/"),
            "got {url}"
        );
        assert!(url.ends_with("/src/main.rs#L3"), "got {url}");
    }

    #[test]
    fn build_link_works_for_nonexistent_file() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());

        let url = build_link(dir.path(), "origin", "src/ghost.rs", None, RefSpec::Commit).unwrap();

        assert!(url.ends_with("/src/ghost.rs"), "got {url}");
    }

    #[test]
    fn build_link_rejects_file_outside_repo() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let outside = tempfile::tempdir().unwrap();
        let stray = outside.path().join("stray.rs");
        fs::write(&stray, "").unwrap();

        let err = build_link(
            dir.path(),
            "origin",
            stray.to_str().unwrap(),
            None,
            RefSpec::Commit,
        );
        assert!(err.is_err());
    }

    #[test]
    fn build_link_branch_uses_branch_name() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let GitRef::Branch(branch) = current_branch(dir.path()).unwrap() else {
            panic!("expected a branch");
        };

        let url = build_link(dir.path(), "origin", "src/main.rs", None, RefSpec::Branch).unwrap();

        assert_eq!(
            url,
            format!("https://github.com/user/repo/blob/{branch}/src/main.rs")
        );
    }

    #[test]
    fn build_link_branch_errors_on_detached_head() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let repo = gix::open(dir.path()).unwrap();
        let sha = repo.head_commit().unwrap().id.to_hex().to_string();
        fs::write(dir.path().join(".git").join("HEAD"), format!("{sha}\n")).unwrap();

        let err = build_link(dir.path(), "origin", "src/main.rs", None, RefSpec::Branch);
        assert!(matches!(err, Err(Error::DetachedHead)));
    }

    #[test]
    fn project_link_end_to_end() {
        let dir = init_repo("git@github.com:user/repo.git");
        let url = project_link(dir.path(), "origin").unwrap();
        assert_eq!(url, "https://github.com/user/repo");
    }

    #[test]
    fn lines_range_accepts_ascending() {
        let lines =
            Lines::range(NonZeroU32::new(10).unwrap(), NonZeroU32::new(20).unwrap()).unwrap();
        assert_eq!(lines.start().get(), 10);
        assert_eq!(lines.end().get(), 20);
    }

    #[test]
    fn lines_range_allows_equal() {
        let n = NonZeroU32::new(5).unwrap();
        assert!(Lines::range(n, n).is_ok());
    }

    #[test]
    fn lines_range_rejects_backwards() {
        let err = Lines::range(NonZeroU32::new(20).unwrap(), NonZeroU32::new(10).unwrap());
        assert!(matches!(err, Err(Error::InvalidLineRange { .. })));
    }
}
