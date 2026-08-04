mod forge;
mod remote;
mod repo_file;
mod target;

pub use forge::Forge;
pub use target::ForgeTarget;

use std::num::NonZero;
use std::path::Path;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("no git repository found")]
    RepositoryNotFound(#[source] BoxError),
    #[error("no '{0}' remote found")]
    NoRemote(String),
    #[error("invalid '{name}' remote")]
    InvalidRemote {
        name: String,
        #[source]
        source: BoxError,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

fn detected_target(host: &str) -> Result<ForgeTarget> {
    let Some(forge) = detect_forge(host) else {
        return Err(Error::UnknownForge(host.to_string()));
    };
    ForgeTarget::new(&format!("https://{host}"), forge)
}

fn resolve_target(
    host: &str,
    target_for_host: impl FnOnce(&str) -> Option<ForgeTarget>,
) -> Result<ForgeTarget> {
    let Some(target) = target_for_host(host) else {
        return detected_target(host);
    };
    Ok(target)
}

/// Builds a URL for the repository project page.
///
/// `target_for_host` receives the hostname from the remote fetch URL after Git
/// URL rewrite rules are applied. If it returns `None`, the target is inferred
/// from that host.
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
    let target = resolve_target(&host, target_for_host)?;
    Ok(target.project_url(&dir))
}

/// Builds a URL for `file`, optionally with line anchors.
///
/// Relative paths are resolved against `path`. `target_for_host` receives the
/// hostname from the remote fetch URL after Git URL rewrite rules are applied.
/// If it returns `None`, the target is inferred from that host.
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
    let file = repo_file::resolve(path, file)?;
    let (host, dir) = remote::remote(&file.repo, remote_name)?;
    let git_ref = match git_ref {
        RefSpec::Commit => remote::head_commit(&file.repo)?,
        RefSpec::Branch => remote::current_branch(&file.repo)?,
    };
    let target = resolve_target(&host, target_for_host)?;
    let req = LinkRequest {
        dir,
        file: file.path,
        git_ref,
        lines,
    };
    Ok(target.file_url(&req))
}

#[cfg(test)]
mod tests;
