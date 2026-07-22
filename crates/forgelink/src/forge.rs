use crate::{ForgeTarget, GitRef, Lines, LinkRequest};

/// Looks for an exact match against a known forge host.
///
/// This does not match subdomains.
fn detect_by_known_host(host: &str) -> Option<Forge> {
    [
        ("git.sr.ht", Forge::SourceHut),
        ("codeberg.org", Forge::Codeberg),
        ("forge.fedoraproject.org", Forge::Codeberg),
    ]
    .into_iter()
    .find_map(|(known, forge)| host.eq_ignore_ascii_case(known).then_some(forge))
}

/// Looks for a forge name in a complete DNS label.
///
/// This does not match when more than one forge name is present.
fn detect_by_forge_label(host: &str) -> Option<Forge> {
    let mut matches = [
        ("github", Forge::GitHub),
        ("gitlab", Forge::GitLab),
        ("bitbucket", Forge::Bitbucket),
    ]
    .into_iter()
    .filter_map(|(label, forge)| {
        host.split('.')
            .any(|part| part.eq_ignore_ascii_case(label))
            .then_some(forge)
    });

    let forge = matches.next()?;
    matches.next().is_none().then_some(forge)
}

pub(crate) fn detect(host: &str) -> Option<Forge> {
    let host = host.split(':').next().unwrap_or(host);

    if let Some(forge) = detect_by_known_host(host) {
        return Some(forge);
    }
    detect_by_forge_label(host)
}

/// A supported forge URL format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Forge {
    GitHub,
    GitLab,
    SourceHut,
    Bitbucket,
    Codeberg,
}

impl Forge {
    pub(crate) fn file_url(self, target: &ForgeTarget, req: &LinkRequest) -> String {
        match self {
            Forge::GitHub => github(target, req),
            Forge::GitLab => gitlab(target, req),
            Forge::SourceHut => sourcehut(target, req),
            Forge::Bitbucket => bitbucket(target, req),
            Forge::Codeberg => codeberg(target, req),
        }
    }
}

fn git_ref_str(git_ref: &GitRef) -> &str {
    match git_ref {
        GitRef::Branch(branch) => branch,
        GitRef::Commit(commit) => commit,
    }
}

fn finish(mut url: url::Url, fragment: Option<String>) -> String {
    url.set_fragment(fragment.as_deref());
    url.into()
}

fn github(target: &ForgeTarget, req: &LinkRequest) -> String {
    let path = format!(
        "{}/blob/{}/{}",
        req.dir,
        git_ref_str(&req.git_ref),
        req.file
    );
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("L{line}"),
        Lines::Range(start, end) => format!("L{start}-L{end}"),
    });
    finish(target.with_path(&path), fragment)
}

fn gitlab(target: &ForgeTarget, req: &LinkRequest) -> String {
    let path = format!(
        "{}/-/blob/{}/{}",
        req.dir,
        git_ref_str(&req.git_ref),
        req.file
    );
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("L{line}"),
        Lines::Range(start, end) => format!("L{start}-{end}"),
    });
    finish(target.with_path(&path), fragment)
}

fn sourcehut(target: &ForgeTarget, req: &LinkRequest) -> String {
    let path = format!(
        "{}/tree/{}/{}",
        req.dir,
        git_ref_str(&req.git_ref),
        req.file
    );
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("L{line}"),
        Lines::Range(start, end) => format!("L{start}-{end}"),
    });
    finish(target.with_path(&path), fragment)
}

fn bitbucket(target: &ForgeTarget, req: &LinkRequest) -> String {
    let path = format!("{}/src/{}/{}", req.dir, git_ref_str(&req.git_ref), req.file);
    let basename = req.file.rsplit('/').next().unwrap_or(&req.file);
    let basename = urlencoding::encode(basename);
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("{basename}-{line}"),
        Lines::Range(start, end) => format!("{basename}-{start}:{end}"),
    });
    finish(target.with_path(&path), fragment)
}

fn codeberg(target: &ForgeTarget, req: &LinkRequest) -> String {
    let path = match &req.git_ref {
        GitRef::Branch(branch) => format!("{}/src/{branch}/{}", req.dir, req.file),
        GitRef::Commit(commit) => format!("{}/src/commit/{commit}/{}", req.dir, req.file),
    };
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("L{line}"),
        Lines::Range(start, end) => format!("L{start}-L{end}"),
    });
    finish(target.with_path(&path), fragment)
}

#[cfg(test)]
mod tests;
