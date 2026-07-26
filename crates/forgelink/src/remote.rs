use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

use crate::{Error, GitRef, Result};

pub fn discover(path: &Path) -> Result<gix::Repository> {
    gix::discover(path).map_err(|e| Error::RepositoryNotFound(Box::new(e)))
}

pub fn remote(repo: &gix::Repository, remote_name: &str) -> Result<(String, String)> {
    let remote = repo
        .try_find_remote(remote_name)
        .ok_or_else(|| Error::NoRemote(remote_name.to_string()))?
        .map_err(|error| Error::InvalidRemoteUrl(error.to_string()))?;
    let url = remote
        .url(gix::remote::Direction::Fetch)
        .ok_or_else(|| Error::InvalidRemoteUrl("missing fetch URL".to_string()))?;
    remote_url_parts(url)
}

pub fn root(repo: &gix::Repository) -> Result<PathBuf> {
    let root = repo.workdir().ok_or(Error::BareRepository)?;
    gix::path::realpath(root).map_err(|e| Error::Io(std::io::Error::other(e)))
}

pub fn head_commit(repo: &gix::Repository) -> Result<GitRef> {
    let commit = repo
        .head_commit()
        .map_err(|e| Error::NoCommit(Box::new(e)))?;
    Ok(GitRef::Commit(commit.id.to_hex().to_string()))
}

pub fn current_branch(repo: &gix::Repository) -> Result<GitRef> {
    let name = repo
        .head_name()
        .map_err(|e| Error::NoCommit(Box::new(e)))?
        .ok_or(Error::DetachedHead)?;
    let branch = name
        .shorten()
        .to_str()
        .map_err(|_| Error::NonUtf8Path)?
        .to_string();
    Ok(GitRef::Branch(branch))
}

fn remote_url_parts(url: &gix::Url) -> Result<(String, String)> {
    let host = url
        .host()
        .ok_or_else(|| Error::InvalidRemoteUrl("missing host".to_string()))?;
    let host = match url.port {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };

    let raw_path = url.path.to_str_lossy();
    let dir = raw_path.trim_start_matches('/');
    let dir = dir.strip_suffix(".git").unwrap_or(dir).to_string();

    Ok((host, dir))
}

#[cfg(test)]
mod tests;
