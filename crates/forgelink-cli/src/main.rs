use std::num::NonZero;

use clap::{Args as ClapArgs, Parser, Subcommand};
use forgelink::{Lines, RefSpec};

#[derive(Parser)]
#[command(
    name = "forgelink",
    version,
    about = "Generate shareable URLs to files in hosted git repositories",
    after_help = concat!(
        "Examples:\n",
        "  forgelink print src/main.rs\n",
        "  forgelink print src/main.rs:42\n",
        "  forgelink print --branch src/main.rs\n",
        "  forgelink print --remote upstream src/main.rs",
    )
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
enum Command {
    /// Print a forge URL to standard output
    Print(FileArgs),

    /// Copy a forge URL to the clipboard
    #[cfg(feature = "clipboard")]
    Copy(FileArgs),

    /// Open a forge URL in the default browser
    #[cfg(feature = "browser")]
    Open(FileArgs),
}

#[derive(Debug, PartialEq, Eq, ClapArgs)]
struct FileArgs {
    /// File path, optionally with line number(s): src/main.rs, src/main.rs:42, src/main.rs:42-55
    file: String,

    /// Use the current branch name instead of the commit SHA
    #[arg(long)]
    branch: bool,

    /// Git remote to use
    #[arg(long, default_value = "origin")]
    remote: String,
}

fn parse_file_arg(raw: &str) -> anyhow::Result<(&str, Option<Lines>)> {
    let Some(colon) = raw.rfind(':') else {
        return Ok((raw, None));
    };
    match parse_line_spec(&raw[colon + 1..])? {
        Some(lines) => Ok((&raw[..colon], Some(lines))),
        None => Ok((raw, None)),
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn parse_line_spec(spec: &str) -> anyhow::Result<Option<Lines>> {
    if let Some((start, end)) = spec.split_once('-')
        && is_digits(start)
        && is_digits(end)
    {
        Ok(Some(Lines::range(parse_line(start)?, parse_line(end)?)?))
    } else if is_digits(spec) {
        Ok(Some(Lines::single(parse_line(spec)?)))
    } else {
        Ok(None)
    }
}

fn parse_line(s: &str) -> anyhow::Result<NonZero<u32>> {
    s.parse()
        .map_err(|_| anyhow::anyhow!("invalid line number '{s}': line numbers start at 1"))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cwd = std::env::current_dir()?;

    match args.command {
        Command::Print(file_args) => {
            let url = build_url(&cwd, &file_args)?;
            println!("{url}");
        }
        #[cfg(feature = "clipboard")]
        Command::Copy(file_args) => {
            let url = build_url(&cwd, &file_args)?;
            arboard::Clipboard::new()?.set_text(url)?;
        }
        #[cfg(feature = "browser")]
        Command::Open(file_args) => {
            let url = build_url(&cwd, &file_args)?;
            open::that(&url)?;
        }
    }

    Ok(())
}

fn build_url(cwd: &std::path::Path, file_args: &FileArgs) -> anyhow::Result<String> {
    let (file, lines) = parse_file_arg(&file_args.file)?;
    let git_ref = if file_args.branch {
        RefSpec::Branch
    } else {
        RefSpec::Commit
    };
    forgelink::build_link(cwd, &file_args.remote, file, lines, git_ref).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_subcommand() {
        assert!(Args::try_parse_from(["forgelink", "src/main.rs"]).is_err());
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
}
