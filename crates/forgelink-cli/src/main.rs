mod config;

use std::fmt;
use std::num::NonZero;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Args as ClapArgs, Parser, Subcommand};
use config::Config;
use forgelink::{Lines, RefSpec};

#[derive(Debug, Parser)]
#[command(
    name = "forgelink",
    version,
    about = "Generate shareable URLs to files in hosted git repositories",
    after_help = concat!(
        "Examples:\n",
        "  forgelink print src/main.rs\n",
        "  forgelink print src/main.rs:42\n",
        "  forgelink print --branch src/main.rs\n",
        "  forgelink print --remote upstream src/main.rs\n",
        "  forgelink --config config.toml print src/main.rs",
    )
)]
struct Args {
    /// Use this configuration file
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, PartialEq, Subcommand)]
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

#[derive(Debug, PartialEq, ClapArgs)]
struct FileArgs {
    /// File path, optionally with line number(s): src/main.rs, src/main.rs:42, src/main.rs:42-55
    file: FileSpec,

    /// Use the current branch name instead of the commit SHA
    #[arg(long)]
    branch: bool,

    /// Git remote to use
    #[arg(long, default_value = "origin")]
    remote: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSpec {
    path: String,
    lines: Option<Lines>,
}

#[derive(Debug)]
struct FileSpecError(String);

impl fmt::Display for FileSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FileSpecError {}

impl FromStr for FileSpec {
    type Err = FileSpecError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if let Some(colon) = raw.rfind(':')
            && let Some(lines) = parse_line_spec(&raw[colon + 1..])?
        {
            return Ok(Self {
                path: raw[..colon].into(),
                lines: Some(lines),
            });
        }
        Ok(Self {
            path: raw.into(),
            lines: None,
        })
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn parse_line_spec(spec: &str) -> Result<Option<Lines>, FileSpecError> {
    if let Some((start, end)) = spec.split_once('-')
        && is_digits(start)
        && is_digits(end)
    {
        Lines::range(parse_line(start)?, parse_line(end)?)
            .map(Some)
            .map_err(|error| FileSpecError(error.to_string()))
    } else if is_digits(spec) {
        Ok(Some(Lines::single(parse_line(spec)?)))
    } else {
        Ok(None)
    }
}

fn parse_line(s: &str) -> Result<NonZero<u32>, FileSpecError> {
    s.parse().map_err(|_| {
        FileSpecError(format!(
            "invalid line number '{s}': expected an integer from 1 to {}",
            u32::MAX
        ))
    })
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::load(args.config.as_deref())?;
    let cwd = std::env::current_dir()?;

    match args.command {
        Command::Print(file_args) => {
            let url = build_url(&cwd, &file_args, &config)?;
            println!("{url}");
        }
        #[cfg(feature = "clipboard")]
        Command::Copy(file_args) => {
            let url = build_url(&cwd, &file_args, &config)?;
            arboard::Clipboard::new()?.set_text(url)?;
        }
        #[cfg(feature = "browser")]
        Command::Open(file_args) => {
            let url = build_url(&cwd, &file_args, &config)?;
            open::that(&url)?;
        }
    }

    Ok(())
}

fn build_url(
    cwd: &std::path::Path,
    file_args: &FileArgs,
    config: &Config,
) -> anyhow::Result<String> {
    let git_ref = if file_args.branch {
        RefSpec::Branch
    } else {
        RefSpec::Commit
    };
    forgelink::build_link(
        cwd,
        &file_args.remote,
        &file_args.file.path,
        file_args.file.lines.clone(),
        git_ref,
        |host| config.target_for(host),
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests;
