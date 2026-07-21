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
mod tests {
    use super::*;
    use crate::{GitRef, Lines, LinkRequest};

    fn target(host: &str) -> ForgeTarget {
        ForgeTarget::new(&format!("https://{host}"), detect(host).unwrap()).unwrap()
    }

    fn req(dir: &str, file: &str, git_ref: GitRef) -> LinkRequest {
        LinkRequest {
            dir: dir.into(),
            file: file.into(),
            git_ref,
            lines: None,
        }
    }

    fn commit(sha: &str) -> GitRef {
        GitRef::Commit(sha.into())
    }

    fn branch(name: &str) -> GitRef {
        GitRef::Branch(name.into())
    }

    fn nz(n: u32) -> std::num::NonZero<u32> {
        std::num::NonZero::new(n).unwrap()
    }

    // --- detection ---

    #[test]
    fn detects_github() {
        assert_eq!(detect("github.com"), Some(Forge::GitHub));
        assert_eq!(detect("github.enterprise.com"), Some(Forge::GitHub));
    }

    #[test]
    fn detects_gitlab() {
        assert!(detect("gitlab.com").is_some());
        assert!(detect("gitlab.company.com").is_some());
    }

    #[test]
    fn detects_sourcehut() {
        assert!(detect("git.sr.ht").is_some());
        assert!(detect("sr.ht").is_none());
        assert!(detect("subdomain.git.sr.ht").is_none());
    }

    #[test]
    fn detects_bitbucket() {
        assert!(detect("bitbucket.org").is_some());
    }

    #[test]
    fn detects_codeberg() {
        assert!(detect("codeberg.org").is_some());
        assert!(detect("forge.fedoraproject.org").is_some());
        assert!(detect("subdomain.codeberg.org").is_none());
        assert!(detect("subdomain.forge.fedoraproject.org").is_none());
    }

    #[test]
    fn unknown_host_returns_none() {
        assert!(detect("example.com").is_none());
    }

    #[test]
    fn detection_is_case_insensitive() {
        assert!(detect("GITHUB.COM").is_some());
        assert!(detect("CODEBERG.ORG").is_some());
    }

    #[test]
    fn forge_names_must_be_complete_dns_labels() {
        assert!(detect("notgithub.example").is_none());
        assert!(detect("gitlabish.example").is_none());
        assert!(detect("mybitbucket.example").is_none());
    }

    #[test]
    fn ambiguous_forge_labels_return_none() {
        assert!(detect("github.gitlab.com").is_none());
        assert!(detect("gitlab.bitbucket.example.com").is_none());
    }

    // --- github ---

    #[test]
    fn github_commit_no_lines() {
        let forge = target("github.com");
        let request = req("user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/main.rs"
        );
    }

    #[test]
    fn github_commit_single_line() {
        let forge = target("github.com");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(42)));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/main.rs#L42"
        );
    }

    #[test]
    fn github_commit_line_range() {
        let forge = target("github.com");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(42), nz(55)));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/main.rs#L42-L55"
        );
    }

    #[test]
    fn github_encodes_special_chars() {
        let forge = target("github.com");
        let request = req("user/repo", "src/my file.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/my%20file.rs"
        );
    }

    // --- gitlab ---

    #[test]
    fn gitlab_commit_no_lines() {
        let forge = target("gitlab.com");
        let request = req("user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs"
        );
    }

    #[test]
    fn gitlab_commit_line_range() {
        let forge = target("gitlab.com");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(10), nz(20)));
        assert_eq!(
            forge.file_url(&request),
            "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs#L10-20"
        );
    }

    // --- sourcehut ---

    #[test]
    fn sourcehut_commit_no_lines() {
        let forge = target("git.sr.ht");
        let request = req("~user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs"
        );
    }

    #[test]
    fn sourcehut_commit_line_range() {
        let forge = target("git.sr.ht");
        let mut request = req("~user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(5), nz(15)));
        assert_eq!(
            forge.file_url(&request),
            "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs#L5-15"
        );
    }

    // --- bitbucket ---

    #[test]
    fn bitbucket_commit_no_lines() {
        let forge = target("bitbucket.org");
        let request = req("user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/abc123/src/main.rs"
        );
    }

    #[test]
    fn bitbucket_commit_line_range() {
        let forge = target("bitbucket.org");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(10), nz(20)));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/abc123/src/main.rs#main.rs-10:20"
        );
    }

    #[test]
    fn bitbucket_encodes_special_chars_in_line_anchor() {
        let forge = target("bitbucket.org");
        let mut request = req("user/repo", "src/a:b?#%.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/abc123/src/a:b%3F%23%25.rs#a%3Ab%3F%23%25.rs-7"
        );
    }

    #[test]
    fn bitbucket_branch_no_lines() {
        let forge = target("bitbucket.org");
        let request = req("user/repo", "src/main.rs", branch("main"));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/main/src/main.rs"
        );
    }

    // --- codeberg ---

    #[test]
    fn codeberg_branch_no_lines() {
        let forge = target("codeberg.org");
        let request = req("user/repo", "src/main.rs", branch("main"));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/main/src/main.rs"
        );
    }

    #[test]
    fn codeberg_commit_prefixes_path() {
        let forge = target("codeberg.org");
        let request = req("user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs"
        );
    }

    #[test]
    fn codeberg_commit_line_range() {
        let forge = target("codeberg.org");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(1), nz(10)));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs#L1-L10"
        );
    }

    #[test]
    fn gitlab_single_line() {
        let forge = target("gitlab.com");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs#L7"
        );
    }

    #[test]
    fn sourcehut_single_line() {
        let forge = target("git.sr.ht");
        let mut request = req("~user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs#L7"
        );
    }

    #[test]
    fn bitbucket_single_line() {
        let forge = target("bitbucket.org");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/abc123/src/main.rs#main.rs-7"
        );
    }

    #[test]
    fn codeberg_single_line() {
        let forge = target("codeberg.org");
        let mut request = req("user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs#L7"
        );
    }

    #[test]
    fn branch_with_slash_keeps_slash() {
        let forge = target("github.com");
        let request = req("user/repo", "src/main.rs", branch("feature/x"));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/feature/x/src/main.rs"
        );
    }

    #[test]
    fn detects_with_port_in_host() {
        assert!(detect("gitlab.example.com:8443").is_some());
    }

    // --- project_url ---

    #[test]
    fn project_url_github() {
        let forge = target("github.com");
        assert_eq!(
            forge.project_url("user/repo"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn project_url_all_forges_same_format() {
        for host in &[
            "github.com",
            "gitlab.com",
            "git.sr.ht",
            "bitbucket.org",
            "codeberg.org",
        ] {
            let forge = target(host);
            assert_eq!(
                forge.project_url("user/repo"),
                format!("https://{host}/user/repo")
            );
        }
    }

    #[test]
    fn fedora_forge_uses_codeberg_format() {
        let forge = target("forge.fedoraproject.org");
        let request = req("user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://forge.fedoraproject.org/user/repo/src/commit/abc123/src/main.rs"
        );
    }
}
