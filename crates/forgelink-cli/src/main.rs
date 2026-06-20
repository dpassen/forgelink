use std::path::Path;

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

    /// Generate a link to the project homepage instead of a file
    #[arg(long)]
    project: bool,
}

fn parse_file_arg(raw: &str) -> (&str, Option<Lines>) {
    if let Some(colon) = raw.rfind(':') {
        let spec = &raw[colon + 1..];
        if let Some(lines) = parse_line_spec(spec) {
            return (&raw[..colon], Some(lines));
        }
    }
    (raw, None)
}

fn parse_line_spec(spec: &str) -> Option<Lines> {
    if let Some((start, end)) = spec.split_once('-') {
        let s = start.parse::<u32>().ok()?;
        let e = end.parse::<u32>().ok()?;
        Some(Lines::Range(s, e))
    } else {
        let n = spec.parse::<u32>().ok()?;
        Some(Lines::Single(n))
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cwd = std::env::current_dir()?;

    if args.project {
        let url = forgelink::project_link(&cwd, "origin")?;
        println!("{url}");
        return Ok(());
    }

    let raw = args
        .file
        .ok_or_else(|| anyhow::anyhow!("a file argument is required (or use --project)"))?;

    let (file, lines) = parse_file_arg(&raw);

    let file_path = Path::new(file);
    let discovery = if file_path.is_absolute() {
        file_path.parent().unwrap_or(file_path).to_path_buf()
    } else {
        cwd.clone()
    };

    let git_ref = forgelink::resolve_ref(&discovery)?;
    let url = forgelink::build_link(&cwd, "origin", git_ref, file, lines)?;
    println!("{url}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_colon_returns_full_path() {
        let (file, lines) = parse_file_arg("src/main.rs");
        assert_eq!(file, "src/main.rs");
        assert!(lines.is_none());
    }

    #[test]
    fn colon_with_single_line() {
        let (file, lines) = parse_file_arg("src/main.rs:42");
        assert_eq!(file, "src/main.rs");
        assert!(matches!(lines, Some(Lines::Single(42))));
    }

    #[test]
    fn colon_with_line_range() {
        let (file, lines) = parse_file_arg("src/main.rs:42-55");
        assert_eq!(file, "src/main.rs");
        assert!(matches!(lines, Some(Lines::Range(42, 55))));
    }

    #[test]
    fn colon_with_non_numeric_spec_returns_full_string() {
        let (file, lines) = parse_file_arg("src/main.rs:notanumber");
        assert_eq!(file, "src/main.rs:notanumber");
        assert!(lines.is_none());
    }

    #[test]
    fn absolute_path_with_line() {
        let (file, lines) = parse_file_arg("/home/user/project/src/main.rs:10");
        assert_eq!(file, "/home/user/project/src/main.rs");
        assert!(matches!(lines, Some(Lines::Single(10))));
    }
}
