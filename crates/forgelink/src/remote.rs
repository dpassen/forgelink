use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

use crate::{Error, GitRef, Result};

pub fn discover(path: &Path) -> Result<gix::Repository> {
    gix::discover(path).map_err(|e| Error::RepositoryNotFound(Box::new(e)))
}

pub fn remote(repo: &gix::Repository, remote_name: &str) -> Result<(String, String)> {
    let config = repo.config_snapshot();
    let url_bytes = config
        .string_by("remote", Some(remote_name.into()), "url")
        .ok_or_else(|| Error::NoRemote(remote_name.to_string()))?;
    let raw = url_bytes
        .to_str()
        .map_err(|_| Error::InvalidRemoteUrl("remote url is not valid UTF-8".to_string()))?;
    parse_remote_url(raw)
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

fn parse_remote_url(raw: &str) -> Result<(String, String)> {
    let url = gix::url::parse(raw.into()).map_err(|e| Error::InvalidRemoteUrl(e.to_string()))?;

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
