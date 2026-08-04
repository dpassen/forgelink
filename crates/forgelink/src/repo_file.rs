use std::path::{Path, PathBuf};

use crate::{Error, Result, remote};

pub(super) struct RepoFile {
    pub(super) repo: gix::Repository,
    pub(super) path: String,
}

pub(super) fn resolve(base: &Path, file: &str) -> Result<RepoFile> {
    let file_path = dunce::simplified(Path::new(file));
    let base = dunce::simplified(base);
    let absolute = absolute_path(base, file_path);
    let search_from = if file_path.is_absolute() {
        absolute.as_path()
    } else {
        base
    };

    let repo = remote::discover(nearest_existing_directory(search_from, file)?)?;
    let path = relative_path(&repo, &absolute, file)?;

    Ok(RepoFile { repo, path })
}

fn absolute_path(base: &Path, file: &Path) -> PathBuf {
    if file.is_absolute() {
        file.to_path_buf()
    } else {
        base.join(file)
    }
}

fn nearest_existing_directory<'a>(path: &'a Path, file: &str) -> Result<&'a Path> {
    path.ancestors().find(|path| path.is_dir()).ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{file}: no existing directory to search for a repository"),
        ))
    })
}

fn relative_path(repo: &gix::Repository, absolute: &Path, file: &str) -> Result<String> {
    let root = remote::root(repo)?;
    let mut resolved = gix::path::realpath(absolute.parent().unwrap_or(absolute))
        .map_err(|error| Error::Io(std::io::Error::other(error)))?;

    if let Some(file_name) = absolute.file_name() {
        resolved.push(file_name);
    }

    resolved
        .strip_prefix(root)
        .map_err(|_| Error::FileOutsideRepository(file.to_string()))?
        .to_str()
        .ok_or(Error::NonUtf8Path)
        .map(|path| path.replace(std::path::MAIN_SEPARATOR, "/"))
}
