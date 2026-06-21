use std::num::NonZeroU32;

use clap::Parser;
use forgelink::Lines;

#[derive(Parser)]
#[command(
    name = "forgelink",
    about = "Generate shareable URLs to files in hosted git repositories"
)]
struct Args {
    /// File path, optionally with line number(s): src/main.rs, src/main.rs:42, src/main.rs:42-55
    file: Option<String>,

    /// Use the current branch name instead of the commit SHA
    #[arg(long)]
    branch: bool,

    /// Generate a link to the project homepage instead of a file
    #[arg(long)]
    project: bool,

    /// Copy the URL to the clipboard in addition to printing it
    #[cfg(feature = "clipboard")]
    #[arg(long)]
    copy: bool,

    /// Open the URL in the default browser in addition to printing it
    #[cfg(feature = "browser")]
    #[arg(long)]
    open: bool,
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

fn parse_line_spec(spec: &str) -> anyhow::Result<Option<Lines>> {
    let is_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());

    if let Some((start, end)) = spec.split_once('-') {
        if is_digits(start) && is_digits(end) {
            return Ok(Some(Lines::range(parse_line(start)?, parse_line(end)?)?));
        }
        Ok(None)
    } else if is_digits(spec) {
        Ok(Some(Lines::single(parse_line(spec)?)))
    } else {
        Ok(None)
    }
}

fn parse_line(s: &str) -> anyhow::Result<NonZeroU32> {
    s.parse::<NonZeroU32>()
        .map_err(|_| anyhow::anyhow!("invalid line number '{s}': line numbers start at 1"))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cwd = std::env::current_dir()?;

    let url = if args.project {
        forgelink::project_link(&cwd, "origin")?
    } else {
        let raw = args
            .file
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("a file argument is required (or use --project)"))?;
        let (file, lines) = parse_file_arg(raw)?;
        let git_ref = if args.branch {
            forgelink::RefSpec::Branch
        } else {
            forgelink::RefSpec::Commit
        };
        forgelink::build_link(&cwd, "origin", file, lines, git_ref)?
    };

    println!("{url}");

    #[cfg(feature = "clipboard")]
    if args.copy {
        copy_to_clipboard(&url)?;
    }

    #[cfg(feature = "browser")]
    if args.open {
        open::that(&url)?;
    }

    Ok(())
}

#[cfg(feature = "clipboard")]
fn copy_to_clipboard(url: &str) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(url)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
