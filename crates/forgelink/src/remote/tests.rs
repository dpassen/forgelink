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
