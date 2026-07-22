mod config;

use std::num::NonZero;
use std::path::PathBuf;

use clap::{Args as ClapArgs, Parser, Subcommand};
use config::Config;
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
    file: String,

    /// Use the current branch name instead of the commit SHA
    #[arg(long)]
    branch: bool,

    /// Git remote to use
    #[arg(long, default_value = "origin")]
    remote: String,
}

fn parse_file_arg(raw: &str) -> anyhow::Result<(&str, Option<Lines>)> {
    if let Some(colon) = raw.rfind(':')
        && let Some(lines) = parse_line_spec(&raw[colon + 1..])?
    {
        return Ok((&raw[..colon], Some(lines)));
    }
    Ok((raw, None))
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
    let (file, lines) = parse_file_arg(&file_args.file)?;
    let git_ref = if file_args.branch {
        RefSpec::Branch
    } else {
        RefSpec::Commit
    };
    forgelink::build_link(cwd, &file_args.remote, file, lines, git_ref, |host| {
        config.target_for(host)
    })
    .map_err(Into::into)
}

#[cfg(test)]
mod tests;
