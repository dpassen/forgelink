use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

use crate::{Error, GitRef, Result};

pub fn resolve(path: &Path, remote_name: &str) -> Result<(String, String)> {
    let repo = gix::discover(path).map_err(|e| Error::Git(e.to_string()))?;

    let config = repo.config_snapshot();
    let url_bytes = config
        .string_by("remote", Some(remote_name.into()), "url")
        .ok_or(Error::NoRemote)?;

    let url =
        gix::url::parse(url_bytes.as_ref()).map_err(|e| Error::InvalidRemoteUrl(e.to_string()))?;

    let host = url.host().ok_or(Error::NoRemote)?.to_string();

    let raw_path = url.path.to_str_lossy();
    let dir = raw_path
        .trim_start_matches('/')
        .trim_end_matches(".git")
        .to_string();

    Ok((host, dir))
}

pub fn repo_root(path: &Path) -> Result<PathBuf> {
    let repo = gix::discover(path).map_err(|e| Error::Git(e.to_string()))?;
    let root = repo
        .workdir()
        .ok_or_else(|| Error::Git("bare repositories are not supported".to_string()))?;
    root.canonicalize().map_err(|e| Error::Git(e.to_string()))
}

pub fn resolve_ref(path: &Path) -> Result<GitRef> {
    let repo = gix::discover(path).map_err(|e| Error::Git(e.to_string()))?;
    let commit = repo.head_commit().map_err(|e| Error::Git(e.to_string()))?;
    Ok(GitRef::Commit(commit.id.to_hex().to_string()))
}
