use super::*;

#[test]
fn parses_alias_and_enterprise_targets() {
    let config = Config::parse(
        r#"
            [[hosts]]
            host = "gh-work"
            base-url = "https://github.com"
            forge = "github"

            [[hosts]]
            host = "git.company.tld"
            base-url = "https://company.tld/services/gitlab"
            forge = "gitlab"
        "#,
    )
    .unwrap();

    let alias = config.target_for("GH-WORK").unwrap();
    assert_eq!(alias.base_url(), "https://github.com/");
    assert_eq!(alias.forge(), Forge::GitHub);

    let enterprise = config.target_for("git.company.tld").unwrap();
    assert_eq!(enterprise.base_url(), "https://company.tld/services/gitlab");
    assert_eq!(enterprise.forge(), Forge::GitLab);
}

#[test]
fn parses_every_supported_forge() {
    for (name, expected) in [
        ("github", Forge::GitHub),
        ("gitlab", Forge::GitLab),
        ("sourcehut", Forge::SourceHut),
        ("bitbucket", Forge::Bitbucket),
        ("codeberg", Forge::Codeberg),
    ] {
        let config = Config::parse(&format!(
            r#"
                [[hosts]]
                host = "internal"
                base-url = "https://git.example.com"
                forge = "{name}"
            "#
        ))
        .unwrap();

        assert_eq!(config.target_for("internal").unwrap().forge(), expected);
    }
}

#[test]
fn missing_hosts_defaults_to_empty() {
    let config = Config::parse("").unwrap();
    assert!(config.target_for("github.com").is_none());
}

#[test]
fn rejects_missing_required_field() {
    let result = Config::parse(
        r#"
            [[hosts]]
            host = "gh-work"
            forge = "github"
        "#,
    );
    assert!(result.is_err());
}

#[test]
fn rejects_unknown_field() {
    let result = Config::parse(
        r#"
            [[hosts]]
            host = "gh-work"
            base-url = "https://github.com"
            forge = "github"
            typo = true
        "#,
    );
    assert!(result.is_err());
}

#[test]
fn rejects_unknown_top_level_field() {
    let result = Config::parse("typo = true");
    assert!(result.is_err());
}

#[test]
fn rejects_unknown_forge() {
    let result = Config::parse(
        r#"
            [[hosts]]
            host = "internal"
            base-url = "https://git.example.com"
            forge = "unknown"
        "#,
    );
    assert!(result.is_err());
}

#[test]
fn rejects_invalid_base_url() {
    let result = Config::parse(
        r#"
            [[hosts]]
            host = "internal"
            base-url = "git.example.com"
            forge = "gitlab"
        "#,
    );
    assert!(result.is_err());
}

#[test]
fn rejects_case_insensitive_duplicate_hosts() {
    let result = Config::parse(
        r#"
            [[hosts]]
            host = "GH-WORK"
            base-url = "https://github.com"
            forge = "github"

            [[hosts]]
            host = "gh-work"
            base-url = "https://github.com"
            forge = "github"
        "#,
    );
    assert!(result.is_err());
}

#[test]
fn rejects_empty_or_padded_hosts() {
    for host in ["", "   ", " gh-work", "gh-work "] {
        let result = Config::parse(&format!(
            r#"
                [[hosts]]
                host = "{host}"
                base-url = "https://github.com"
                forge = "github"
            "#
        ));
        assert!(result.is_err(), "accepted host {host:?}");
    }
}

#[test]
fn reports_duplicate_before_validating_duplicate_target() {
    let error = Config::parse(
        r#"
            [[hosts]]
            host = "internal"
            base-url = "https://github.com"
            forge = "github"

            [[hosts]]
            host = "INTERNAL"
            base-url = "not a URL"
            forge = "unknown"
        "#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("duplicate host"));
}

#[test]
fn load_reports_config_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "not valid toml [").unwrap();

    let error = Config::load(Some(&path)).unwrap_err();
    assert!(error.to_string().contains(&path.display().to_string()));
}

#[test]
fn explicit_path_has_precedence() {
    let explicit = Path::new("custom.toml");
    let xdg = Path::new("xdg");
    let native = Path::new("native");

    assert_eq!(
        select_path(Some(explicit), Some(xdg), Some(native)),
        Some(explicit.to_path_buf())
    );
}

#[test]
fn xdg_path_has_precedence_over_native_path() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let native = dir.path().join("native");

    assert_eq!(
        select_path(None, Some(&xdg), Some(&native)),
        Some(xdg.join("forgelink/config.toml"))
    );
}

#[test]
fn relative_xdg_path_falls_back_to_native_path() {
    let xdg = Path::new("relative");
    let native = Path::new("native");

    assert_eq!(
        select_path(None, Some(xdg), Some(native)),
        Some(native.join("forgelink/config.toml"))
    );
}

#[test]
fn native_path_is_the_fallback() {
    let native = Path::new("native");

    assert_eq!(
        select_path(None, None, Some(native)),
        Some(native.join("forgelink/config.toml"))
    );
}

#[test]
fn missing_default_file_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.toml");

    let config = Config::load_path(&path, false).unwrap();
    assert!(config.target_for("github.com").is_none());
}

#[test]
fn malformed_default_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "not valid toml [").unwrap();

    assert!(Config::load_path(&path, false).is_err());
}

#[test]
fn missing_explicit_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.toml");

    assert!(Config::load_path(&path, true).is_err());
}
