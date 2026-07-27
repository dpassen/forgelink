use super::*;

#[test]
fn requires_subcommand() {
    assert!(Args::try_parse_from(["forgelink", "src/main.rs"]).is_err());
}

#[test]
fn parses_config_before_subcommand() {
    let args = Args::try_parse_from([
        "forgelink",
        "--config",
        "custom.toml",
        "print",
        "src/main.rs",
    ])
    .unwrap();

    assert_eq!(args.config, Some(PathBuf::from("custom.toml")));
}

#[test]
fn parses_config_after_subcommand() {
    let args = Args::try_parse_from([
        "forgelink",
        "print",
        "--config",
        "custom.toml",
        "src/main.rs",
    ])
    .unwrap();

    assert_eq!(args.config, Some(PathBuf::from("custom.toml")));
}

#[test]
fn parses_print_command() {
    let args = Args::try_parse_from(["forgelink", "print", "src/main.rs"]).unwrap();

    assert_eq!(
        args.command,
        Command::Print(FileArgs {
            file: "src/main.rs".parse().unwrap(),
            branch: false,
            remote: "origin".to_string(),
        })
    );
}

#[test]
fn parses_file_options() {
    let args = Args::try_parse_from([
        "forgelink",
        "print",
        "--remote",
        "upstream",
        "--branch",
        "src/main.rs",
    ])
    .unwrap();
    assert_eq!(
        args.command,
        Command::Print(FileArgs {
            file: "src/main.rs".parse().unwrap(),
            branch: true,
            remote: "upstream".to_string(),
        })
    );
}

#[cfg(feature = "clipboard")]
#[test]
fn parses_copy_command() {
    let args = Args::try_parse_from([
        "forgelink",
        "copy",
        "--remote",
        "upstream",
        "--branch",
        "src/main.rs",
    ])
    .unwrap();

    assert_eq!(
        args.command,
        Command::Copy(FileArgs {
            file: "src/main.rs".parse().unwrap(),
            branch: true,
            remote: "upstream".to_string(),
        })
    );
}

#[cfg(feature = "browser")]
#[test]
fn parses_open_command() {
    let args = Args::try_parse_from([
        "forgelink",
        "open",
        "--remote",
        "upstream",
        "--branch",
        "src/main.rs",
    ])
    .unwrap();

    assert_eq!(
        args.command,
        Command::Open(FileArgs {
            file: "src/main.rs".parse().unwrap(),
            branch: true,
            remote: "upstream".to_string(),
        })
    );
}

#[test]
fn no_colon_returns_full_path() {
    let spec: FileSpec = "src/main.rs".parse().unwrap();
    assert_eq!(spec.path, "src/main.rs");
    assert!(spec.lines.is_none());
}

#[test]
fn colon_with_single_line() {
    let spec: FileSpec = "src/main.rs:42".parse().unwrap();
    assert_eq!(spec.path, "src/main.rs");
    let lines = spec.lines.unwrap();
    assert_eq!(lines.start().get(), 42);
    assert_eq!(lines.end().get(), 42);
}

#[test]
fn colon_with_line_range() {
    let spec: FileSpec = "src/main.rs:42-55".parse().unwrap();
    assert_eq!(spec.path, "src/main.rs");
    let lines = spec.lines.unwrap();
    assert_eq!(lines.start().get(), 42);
    assert_eq!(lines.end().get(), 55);
}

#[test]
fn colon_with_non_numeric_spec_returns_full_string() {
    let spec: FileSpec = "src/main.rs:notanumber".parse().unwrap();
    assert_eq!(spec.path, "src/main.rs:notanumber");
    assert!(spec.lines.is_none());
}

#[test]
fn absolute_path_with_line() {
    let spec: FileSpec = "/home/user/project/src/main.rs:10".parse().unwrap();
    assert_eq!(spec.path, "/home/user/project/src/main.rs");
    assert_eq!(spec.lines.unwrap().start().get(), 10);
}

#[test]
fn clap_reports_invalid_line_number() {
    let error = Args::try_parse_from(["forgelink", "print", "src/main.rs:0"]).unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    assert!(error.to_string().contains("invalid line number '0'"));
}

#[test]
fn backwards_range_is_an_error() {
    let error = Args::try_parse_from(["forgelink", "print", "src/main.rs:55-42"]).unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    assert!(
        error
            .to_string()
            .contains("line range end (42) is before start (55)")
    );
}
