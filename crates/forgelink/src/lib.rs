mod forge;
mod remote;

pub use forge::Forge;

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no git remote found")]
    NoRemote,
    #[error("unrecognized forge: {0}")]
    UnknownForge(String),
    #[error("invalid remote URL: {0}")]
    InvalidRemoteUrl(String),
    #[error("git error: {0}")]
    Git(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum GitRef {
    Branch(String),
    Commit(String),
}

#[derive(Debug, Clone)]
pub enum Lines {
    Single(u32),
    Range(u32, u32),
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
    remote::resolve_ref(path)
}

pub fn project_link(path: &Path, remote_name: &str) -> Result<String> {
    let discovery_path = path;
    let (host, dir) = remote::resolve(discovery_path, remote_name)?;
    let forge = detect_forge(&host).ok_or_else(|| Error::UnknownForge(host.clone()))?;
    Ok(forge.project_url(&host, &dir))
}

pub fn build_link(
    path: &Path,
    remote_name: &str,
    git_ref: GitRef,
    file: &str,
    lines: Option<Lines>,
) -> Result<String> {
    let file_path = Path::new(file);
    let absolute = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        path.join(file_path)
    };
    let discovery_path = absolute.parent().unwrap_or(&absolute);

    let (host, dir) = remote::resolve(discovery_path, remote_name)?;
    let root = remote::repo_root(discovery_path)?;

    let canonical = absolute
        .parent()
        .map(|p| {
            p.canonicalize()
                .map(|c| c.join(absolute.file_name().unwrap_or_default()))
        })
        .transpose()
        .map_err(|e| Error::Git(e.to_string()))?
        .unwrap_or(absolute);

    let relative = canonical
        .strip_prefix(&root)
        .map_err(|_| Error::Git(format!("{file} is not inside the repository")))?
        .to_str()
        .ok_or_else(|| Error::Git("file path is not valid UTF-8".to_string()))?
        .to_string();

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
