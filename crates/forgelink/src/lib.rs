mod forge;
mod remote;
mod target;

pub use forge::Forge;
pub use target::ForgeTarget;

use std::num::NonZero;
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
    #[error("invalid base URL: {0}")]
    InvalidBaseUrl(String),
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
    InvalidLineRange {
        start: NonZero<u32>,
        end: NonZero<u32>,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum GitRef {
    Branch(String),
    Commit(String),
}

#[derive(Debug, Clone, Copy, Default)]
pub enum RefSpec {
    #[default]
    Commit,
    Branch,
}

#[derive(Debug, Clone)]
pub enum Lines {
    #[non_exhaustive]
    Single(NonZero<u32>),
    #[non_exhaustive]
    Range(NonZero<u32>, NonZero<u32>),
}

impl Lines {
    #[must_use]
    pub fn single(line: NonZero<u32>) -> Self {
        Lines::Single(line)
    }

    /// Creates an inclusive line range.
    ///
    /// # Errors
    ///
    /// Fails if `end` comes before `start`.
    pub fn range(start: NonZero<u32>, end: NonZero<u32>) -> Result<Self> {
        if end < start {
            return Err(Error::InvalidLineRange { start, end });
        }
        Ok(Lines::Range(start, end))
    }

    #[must_use]
    pub fn start(&self) -> NonZero<u32> {
        match self {
            Lines::Single(n) | Lines::Range(n, _) => *n,
        }
    }

    #[must_use]
    pub fn end(&self) -> NonZero<u32> {
        match self {
            Lines::Single(n) | Lines::Range(_, n) => *n,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinkRequest {
    pub dir: String,
    pub file: String,
    pub git_ref: GitRef,
    pub lines: Option<Lines>,
}

/// Detects the forge for `host`.
///
/// Returns `None` if the forge is unsupported.
#[must_use]
pub fn detect_forge(host: &str) -> Option<Forge> {
    forge::detect(host)
}

fn detected_target(host: String) -> Result<ForgeTarget> {
    let Some(forge) = detect_forge(&host) else {
        return Err(Error::UnknownForge(host));
    };
    ForgeTarget::new(&format!("https://{host}"), forge)
}

/// Builds a URL for the repository project page.
///
/// `target_for_host` receives the original host parsed from the Git remote. If
/// it returns `None`, the target is inferred from that host.
///
/// # Errors
///
/// Fails if `path` is not in a Git repository, the remote is missing or invalid,
/// or neither `target_for_host` nor automatic detection supplies a target.
pub fn project_link(
    path: &Path,
    remote_name: &str,
    target_for_host: impl FnOnce(&str) -> Option<ForgeTarget>,
) -> Result<String> {
    let repo = remote::discover(path)?;
    let (host, dir) = remote::remote(&repo, remote_name)?;
    let target = match target_for_host(&host) {
        Some(target) => target,
        None => detected_target(host)?,
    };
    Ok(target.project_url(&dir))
}

/// Builds a URL for `file`, optionally with line anchors.
///
/// Relative paths are resolved against `path`. `target_for_host` receives the
/// original host parsed from the Git remote. If it returns `None`, the target is
/// inferred from that host.
///
/// # Errors
///
/// Fails if `path` is not in a Git repository, the remote is missing or invalid,
/// the forge is unsupported, or `file` is outside the repository.
///
/// With [`RefSpec::Branch`], this also fails on a detached `HEAD`.
pub fn build_link(
    path: &Path,
    remote_name: &str,
    file: &str,
    lines: Option<Lines>,
    git_ref: RefSpec,
    target_for_host: impl FnOnce(&str) -> Option<ForgeTarget>,
) -> Result<String> {
    let file_path = dunce::simplified(Path::new(file));
    let base = dunce::simplified(path);
    let absolute = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        base.join(file_path)
    };
    let discovery_path = if file_path.is_absolute() {
        absolute.as_path()
    } else {
        base
    };

    let discovery_start = discovery_path
        .ancestors()
        .find(|p| p.is_dir())
        .ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{file}: no existing directory to search for a repository"),
            ))
        })?;
    let repo = remote::discover(discovery_start)?;

    let (host, dir) = remote::remote(&repo, remote_name)?;
    let root = remote::root(&repo)?;
    let resolved_parent = gix::path::realpath(absolute.parent().unwrap_or(&absolute))
        .map_err(|e| Error::Io(std::io::Error::other(e)))?;
    let resolved = match absolute.file_name() {
        Some(file_name) => resolved_parent.join(file_name),
        None => resolved_parent,
    };
    let git_ref = match git_ref {
        RefSpec::Commit => remote::head_commit(&repo)?,
        RefSpec::Branch => remote::current_branch(&repo)?,
    };

    let relative = resolved
        .strip_prefix(&root)
        .map_err(|_| Error::FileOutsideRepository(file.to_string()))?
        .to_str()
        .ok_or(Error::NonUtf8Path)?
        .replace(std::path::MAIN_SEPARATOR, "/");

    let target = match target_for_host(&host) {
        Some(target) => target,
        None => detected_target(host)?,
    };
    let req = LinkRequest {
        dir,
        file: relative,
        git_ref,
        lines,
    };
    Ok(target.file_url(&req))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

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

    fn automatic_target(_: &str) -> Option<ForgeTarget> {
        None
    }

    #[test]
    fn build_link_end_to_end() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();

        let lines = Lines::single(NonZero::new(3).unwrap());
        let url = build_link(
            dir.path(),
            "origin",
            "src/main.rs",
            Some(lines),
            RefSpec::Commit,
            automatic_target,
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

        let url = build_link(
            dir.path(),
            "origin",
            "src/ghost.rs",
            None,
            RefSpec::Commit,
            automatic_target,
        )
        .unwrap();

        assert!(url.ends_with("/src/ghost.rs"), "got {url}");
    }

    #[test]
    fn build_link_resolves_relative_path_from_subdirectory() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.rs"), "").unwrap();

        let url = build_link(
            &src,
            "origin",
            "main.rs",
            None,
            RefSpec::Commit,
            automatic_target,
        )
        .unwrap();

        assert!(url.ends_with("/src/main.rs"), "got {url}");
    }

    #[test]
    fn build_link_normalizes_nonexistent_component_followed_by_parent() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.rs"), "").unwrap();

        let url = build_link(
            dir.path(),
            "origin",
            "src/missing/../main.rs",
            None,
            RefSpec::Commit,
            automatic_target,
        )
        .unwrap();

        assert!(url.ends_with("/src/main.rs"), "got {url}");
    }

    #[test]
    fn build_link_absolute_path_uses_target_repository() {
        let current = init_repo("https://github.com/current/repo.git");
        commit_empty(current.path());
        let target = init_repo("https://github.com/target/repo.git");
        commit_empty(target.path());
        let file = target.path().join("src").join("main.rs");
        fs::create_dir(file.parent().unwrap()).unwrap();
        fs::write(&file, "").unwrap();

        let url = build_link(
            current.path(),
            "origin",
            file.to_str().unwrap(),
            None,
            RefSpec::Commit,
            automatic_target,
        )
        .unwrap();

        assert!(
            url.starts_with("https://github.com/target/repo/blob/"),
            "got {url}"
        );
        assert!(url.ends_with("/src/main.rs"), "got {url}");
    }

    #[cfg(unix)]
    #[test]
    fn build_link_resolves_symlink_within_repository() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.rs"), "").unwrap();
        std::os::unix::fs::symlink(&src, dir.path().join("source")).unwrap();

        let url = build_link(
            dir.path(),
            "origin",
            "source/main.rs",
            None,
            RefSpec::Commit,
            automatic_target,
        )
        .unwrap();

        assert!(url.ends_with("/src/main.rs"), "got {url}");
    }

    #[cfg(unix)]
    #[test]
    fn build_link_preserves_final_symlink() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.rs"), "").unwrap();
        std::os::unix::fs::symlink("main.rs", src.join("link.rs")).unwrap();

        let url = build_link(
            dir.path(),
            "origin",
            "src/link.rs",
            None,
            RefSpec::Commit,
            automatic_target,
        )
        .unwrap();

        assert!(url.ends_with("/src/link.rs"), "got {url}");
    }

    #[cfg(unix)]
    #[test]
    fn build_link_does_not_link_through_symlink_outside_repository() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("stray.rs"), "").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("outside")).unwrap();

        let result = build_link(
            dir.path(),
            "origin",
            "outside/stray.rs",
            None,
            RefSpec::Commit,
            automatic_target,
        );

        assert!(matches!(result, Err(Error::FileOutsideRepository(_))));
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
            automatic_target,
        );
        assert!(err.is_err());
    }

    #[test]
    fn build_link_rejects_nonexistent_path_traversal_outside_repo() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let root = dir.path().canonicalize().unwrap();

        let err = build_link(
            &root,
            "origin",
            "missing/../../outside.rs",
            None,
            RefSpec::Commit,
            automatic_target,
        );

        assert!(matches!(err, Err(Error::FileOutsideRepository(_))));
    }

    #[test]
    fn build_link_branch_uses_branch_name() {
        let dir = init_repo("https://github.com/user/repo.git");
        commit_empty(dir.path());
        let repo = remote::discover(dir.path()).unwrap();
        let GitRef::Branch(branch) = remote::current_branch(&repo).unwrap() else {
            panic!("expected a branch");
        };

        let url = build_link(
            dir.path(),
            "origin",
            "src/main.rs",
            None,
            RefSpec::Branch,
            automatic_target,
        )
        .unwrap();

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

        let err = build_link(
            dir.path(),
            "origin",
            "src/main.rs",
            None,
            RefSpec::Branch,
            automatic_target,
        );
        assert!(matches!(err, Err(Error::DetachedHead)));
    }

    #[test]
    fn project_link_end_to_end() {
        let dir = init_repo("git@github.com:user/repo.git");
        let url = project_link(dir.path(), "origin", automatic_target).unwrap();
        assert_eq!(url, "https://github.com/user/repo");
    }

    #[test]
    fn build_link_accepts_target_for_ssh_alias() {
        let dir = init_repo("git@gh-work:user/repo.git");
        commit_empty(dir.path());
        let expected_host = "gh-work".to_string();

        let url = build_link(
            dir.path(),
            "origin",
            "src/main.rs",
            None,
            RefSpec::Commit,
            move |host| {
                assert_eq!(host, expected_host);
                drop(expected_host);
                Some(ForgeTarget::new("https://github.com", Forge::GitHub).unwrap())
            },
        )
        .unwrap();

        assert!(
            url.starts_with("https://github.com/user/repo/blob/"),
            "got {url}"
        );
        assert!(url.ends_with("/src/main.rs"), "got {url}");
    }

    #[test]
    fn project_link_accepts_enterprise_target() {
        let dir = init_repo("git@git.company.tld:group/repo.git");

        let url = project_link(dir.path(), "origin", |host| {
            assert_eq!(host, "git.company.tld");
            Some(ForgeTarget::new("https://company.tld/services/gitlab", Forge::GitLab).unwrap())
        })
        .unwrap();

        assert_eq!(url, "https://company.tld/services/gitlab/group/repo");
    }

    #[test]
    fn unknown_host_without_target_still_errors() {
        let dir = init_repo("git@internal:user/repo.git");
        let error = project_link(dir.path(), "origin", automatic_target).unwrap_err();

        assert!(matches!(error, Error::UnknownForge(host) if host == "internal"));
    }

    #[test]
    fn lines_range_accepts_ascending() {
        let lines = Lines::range(NonZero::new(10).unwrap(), NonZero::new(20).unwrap()).unwrap();
        assert_eq!(lines.start().get(), 10);
        assert_eq!(lines.end().get(), 20);
    }

    #[test]
    fn lines_range_allows_equal() {
        let n = NonZero::new(5).unwrap();
        assert!(Lines::range(n, n).is_ok());
    }

    #[test]
    fn lines_range_rejects_backwards() {
        let err = Lines::range(NonZero::new(20).unwrap(), NonZero::new(10).unwrap());
        assert!(matches!(err, Err(Error::InvalidLineRange { .. })));
    }
}
