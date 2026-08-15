use super::*;

fn remote_info(
    hostname: &str,
    port: Option<u16>,
    scheme: gix::url::Scheme,
    repository: &str,
) -> RemoteInfo {
    RemoteInfo {
        hostname: hostname.to_string(),
        port,
        scheme,
        repository: repository.to_string(),
    }
}

#[test]
fn https_url() {
    let expected = remote_info("github.com", None, gix::url::Scheme::Https, "user/repo");
    let url = gix::url::parse("https://github.com/user/repo.git").unwrap();
    let actual = location_from_url(&url).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn https_url_without_dot_git() {
    let expected = remote_info("github.com", None, gix::url::Scheme::Https, "user/repo");
    let url = gix::url::parse("https://github.com/user/repo").unwrap();
    let actual = location_from_url(&url).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn scp_like_ssh_url() {
    let expected = remote_info("github.com", None, gix::url::Scheme::Ssh, "user/repo");
    let url = gix::url::parse("git@github.com:user/repo.git").unwrap();
    let actual = location_from_url(&url).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn ssh_url_with_scheme() {
    let expected = remote_info(
        "codeberg.org",
        Some(2222),
        gix::url::Scheme::Ssh,
        "user/repo",
    );
    let url = gix::url::parse("ssh://git@codeberg.org:2222/user/repo.git").unwrap();
    let actual = location_from_url(&url).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn preserves_non_default_port() {
    let url = gix::url::parse("https://git.example.com:8443/user/repo.git").unwrap();
    let remote = location_from_url(&url).unwrap();
    assert_eq!("git.example.com:8443", remote.web_authority());
}

#[test]
fn ignores_ssh_port_for_web_authority() {
    let url = gix::url::parse("ssh://git@codeberg.org:2222/user/repo.git").unwrap();
    let remote = location_from_url(&url).unwrap();
    assert_eq!("codeberg.org", remote.web_authority());
}

#[test]
fn gitlab_subgroups() {
    let expected = remote_info(
        "gitlab.com",
        None,
        gix::url::Scheme::Https,
        "group/subgroup/repo",
    );
    let url = gix::url::parse("https://gitlab.com/group/subgroup/repo.git").unwrap();
    let actual = location_from_url(&url).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn strips_only_one_trailing_dot_git() {
    // A repo literally named "repo.git" should keep one ".git".
    let expected = remote_info("github.com", None, gix::url::Scheme::Https, "user/repo.git");
    let url = gix::url::parse("https://github.com/user/repo.git.git").unwrap();
    let actual = location_from_url(&url).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn sourcehut_tilde_owner() {
    let expected = remote_info("git.sr.ht", None, gix::url::Scheme::Https, "~user/repo");
    let url = gix::url::parse("https://git.sr.ht/~user/repo").unwrap();
    let actual = location_from_url(&url).unwrap();
    assert_eq!(expected, actual);
}
