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
            file: "src/main.rs".to_string(),
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
            file: "src/main.rs".to_string(),
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
            file: "src/main.rs".to_string(),
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
            file: "src/main.rs".to_string(),
            branch: true,
            remote: "upstream".to_string(),
        })
    );
}

#[test]
fn no_colon_returns_full_path() {
    let (file, lines) = parse_file_arg("src/main.rs").unwrap();
    assert_eq!(file, "src/main.rs");
    assert!(lines.is_none());
}

#[test]
fn colon_with_single_line() {
    let (file, lines) = parse_file_arg("src/main.rs:42").unwrap();
    assert_eq!(file, "src/main.rs");
    let lines = lines.unwrap();
    assert_eq!(lines.start().get(), 42);
    assert_eq!(lines.end().get(), 42);
}

#[test]
fn colon_with_line_range() {
    let (file, lines) = parse_file_arg("src/main.rs:42-55").unwrap();
    assert_eq!(file, "src/main.rs");
    let lines = lines.unwrap();
    assert_eq!(lines.start().get(), 42);
    assert_eq!(lines.end().get(), 55);
}

#[test]
fn colon_with_non_numeric_spec_returns_full_string() {
    let (file, lines) = parse_file_arg("src/main.rs:notanumber").unwrap();
    assert_eq!(file, "src/main.rs:notanumber");
    assert!(lines.is_none());
}

#[test]
fn absolute_path_with_line() {
    let (file, lines) = parse_file_arg("/home/user/project/src/main.rs:10").unwrap();
    assert_eq!(file, "/home/user/project/src/main.rs");
    let lines = lines.unwrap();
    assert_eq!(lines.start().get(), 10);
}

#[test]
fn line_zero_is_an_error() {
    assert!(parse_file_arg("src/main.rs:0").is_err());
}

#[test]
fn backwards_range_is_an_error() {
    assert!(parse_file_arg("src/main.rs:55-42").is_err());
}
