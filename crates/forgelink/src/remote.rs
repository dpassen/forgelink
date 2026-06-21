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
    Ok(root.canonicalize()?)
}

pub fn head_commit(repo: &gix::Repository) -> Result<GitRef> {
    let commit = repo
        .head_commit()
        .map_err(|e| Error::NoCommit(Box::new(e)))?;
    Ok(GitRef::Commit(commit.id.to_hex().to_string()))
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
mod tests {
    use super::*;

    #[test]
    fn https_url() {
        let (host, dir) = parse_remote_url("https://github.com/user/repo.git").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(dir, "user/repo");
    }

    #[test]
    fn https_url_without_dot_git() {
        let (host, dir) = parse_remote_url("https://github.com/user/repo").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(dir, "user/repo");
    }

    #[test]
    fn scp_like_ssh_url() {
        let (host, dir) = parse_remote_url("git@github.com:user/repo.git").unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(dir, "user/repo");
    }

    #[test]
    fn ssh_url_with_scheme() {
        let (host, dir) = parse_remote_url("ssh://git@codeberg.org/user/repo.git").unwrap();
        assert_eq!(host, "codeberg.org");
        assert_eq!(dir, "user/repo");
    }

    #[test]
    fn preserves_non_default_port() {
        let (host, dir) = parse_remote_url("https://git.example.com:8443/user/repo.git").unwrap();
        assert_eq!(host, "git.example.com:8443");
        assert_eq!(dir, "user/repo");
    }

    #[test]
    fn gitlab_subgroups() {
        let (host, dir) = parse_remote_url("https://gitlab.com/group/subgroup/repo.git").unwrap();
        assert_eq!(host, "gitlab.com");
        assert_eq!(dir, "group/subgroup/repo");
    }

    #[test]
    fn strips_only_one_trailing_dot_git() {
        // A repo literally named "repo.git" should keep one ".git".
        let (_, dir) = parse_remote_url("https://github.com/user/repo.git.git").unwrap();
        assert_eq!(dir, "user/repo.git");
    }

    #[test]
    fn sourcehut_tilde_owner() {
        let (host, dir) = parse_remote_url("https://git.sr.ht/~user/repo").unwrap();
        assert_eq!(host, "git.sr.ht");
        assert_eq!(dir, "~user/repo");
    }
}
