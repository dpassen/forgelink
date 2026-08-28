use super::*;

#[test]
fn accepts_https_url() {
    let target = ForgeTarget::new("https://github.com", Forge::GitHub).unwrap();

    assert_eq!(target.base_url(), "https://github.com/");
    assert_eq!(target.forge(), Forge::GitHub);
}

#[test]
fn accepts_http_port_and_path_prefix() {
    let target = ForgeTarget::new(
        "http://git.example.com:8080/services/gitlab/",
        Forge::GitLab,
    )
    .unwrap();

    assert_eq!(
        target.base_url(),
        "http://git.example.com:8080/services/gitlab"
    );
}

#[test]
fn renders_against_base_url_with_path_prefix() {
    let target =
        ForgeTarget::new("https://company.example/services/gitlab", Forge::GitLab).unwrap();
    let request = LinkRequest {
        dir: "group/repo".into(),
        file: "src/main.rs".into(),
        git_ref: crate::GitRef::Commit("abc123".into()),
        lines: None,
    };

    assert_eq!(
        target.file_url(&request).unwrap(),
        "https://company.example/services/gitlab/group/repo/-/blob/abc123/src/main.rs"
    );
    assert_eq!(
        target.project_url("group/repo").unwrap(),
        "https://company.example/services/gitlab/group/repo"
    );
}

#[test]
fn rendering_non_base_url_returns_errors() {
    let target = ForgeTarget {
        base_url: Url::parse("mailto:user@example.com").unwrap(),
        forge: Forge::GitHub,
    };
    let request = LinkRequest {
        dir: "group/repo".into(),
        file: "src/main.rs".into(),
        git_ref: crate::GitRef::Commit("abc123".into()),
        lines: None,
    };

    let project_error = target.project_url("group/repo").unwrap_err();
    assert_eq!(
        project_error.to_string(),
        "invalid base URL: cannot be used as a base"
    );

    let file_error = target.file_url(&request).unwrap_err();
    assert_eq!(
        file_error.to_string(),
        "invalid base URL: cannot be used as a base"
    );
}

#[test]
fn rejects_non_http_scheme() {
    let error = ForgeTarget::new("ssh://git@example.com/repo", Forge::GitHub).unwrap_err();

    assert_eq!(
        error.to_string(),
        "invalid base URL: scheme must be http or https"
    );
}

#[test]
fn rejects_relative_url() {
    assert!(ForgeTarget::new("git.example.com", Forge::GitLab).is_err());
}

#[test]
fn rejects_credentials() {
    assert!(ForgeTarget::new("https://user@example.com", Forge::GitHub).is_err());
    assert!(ForgeTarget::new("https://user:secret@example.com", Forge::GitHub).is_err());
}

#[test]
fn rejects_query() {
    assert!(ForgeTarget::new("https://example.com?theme=dark", Forge::GitLab).is_err());
}

#[test]
fn rejects_fragment() {
    assert!(ForgeTarget::new("https://example.com#readme", Forge::GitLab).is_err());
}
