use std::path::{Path, PathBuf};

use gix::{bstr::ByteSlice, error::MetadataValue};

use crate::{Error, GitRef, RemoteInfo, Result};

pub fn discover(path: &Path) -> Result<gix::Repository> {
    gix::discover(path).map_err(|e| Error::RepositoryNotFound(Box::new(e)))
}

pub fn location(repo: &gix::Repository, remote_name: &str) -> Result<RemoteInfo> {
    let mut remote = repo
        .try_find_remote_without_url_rewrite(remote_name)
        .ok_or_else(|| Error::NoRemote(remote_name.to_string()))?
        .map_err(|source| invalid_remote(source, remote_name))?;
    remote
        .rewrite_urls()
        .map_err(|source| Error::InvalidRemoteUrl(source.to_string()))?;
    let url = remote
        .url(gix::remote::Direction::Fetch)
        .ok_or_else(|| Error::InvalidRemoteUrl("missing fetch URL".to_string()))?;
    location_from_url(url)
}

fn invalid_remote(source: gix::Error, remote_name: &str) -> Error {
    let invalid_url = source.metadata().any(|metadata| {
        matches!(
            metadata.get("key"),
            Some(MetadataValue::String(key))
                if key == "remote.<name>.url" || key == "remote.<name>.pushUrl"
        )
    });
    if invalid_url {
        Error::InvalidRemoteUrl(source.to_string())
    } else {
        Error::InvalidRemote {
            name: remote_name.to_string(),
            source: Box::new(source),
        }
    }
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

fn repository_path(raw_path: &str) -> Result<String> {
    let mut segments: Vec<_> = raw_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let repository_name = segments.pop().unwrap_or_default();
    let repository_name = repository_name
        .strip_suffix(".git")
        .unwrap_or(repository_name);
    if repository_name.is_empty() {
        return Err(Error::InvalidRemoteUrl(
            "missing repository path".to_string(),
        ));
    }
    segments.push(repository_name);
    Ok(segments.join("/"))
}

fn location_from_url(url: &gix::Url) -> Result<RemoteInfo> {
    let hostname = url
        .host()
        .ok_or_else(|| Error::InvalidRemoteUrl("missing host".to_string()))?
        .to_string();
    let repository = repository_path(&url.path.to_str_lossy())?;

    Ok(RemoteInfo {
        hostname,
        port: url.port,
        scheme: url.scheme.clone(),
        repository,
    })
}

#[cfg(test)]
mod tests;
