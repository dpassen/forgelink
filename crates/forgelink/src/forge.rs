use std::fmt::Write as _;

use crate::{GitRef, Lines, LinkRequest};

fn encode(s: &str) -> String {
    s.split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub trait Forge {
    fn file_url(&self, req: &LinkRequest) -> String;
    fn project_url(&self, host: &str, dir: &str) -> String;
}

pub(crate) fn detect(host: &str) -> Option<impl Forge + use<>> {
    let host = host.split(':').next().unwrap_or(host);
    let forge = if host.contains("github") {
        KnownForge::GitHub
    } else if host.contains("gitlab") {
        KnownForge::GitLab
    } else if host == "git.sr.ht" {
        KnownForge::SourceHut
    } else if host.contains("bitbucket") {
        KnownForge::Bitbucket
    } else if host == "codeberg.org" || host == "forge.fedoraproject.org" {
        KnownForge::Codeberg
    } else {
        return None;
    };
    Some(forge)
}

enum KnownForge {
    GitHub,
    GitLab,
    SourceHut,
    Bitbucket,
    Codeberg,
}

impl Forge for KnownForge {
    fn file_url(&self, req: &LinkRequest) -> String {
        match self {
            KnownForge::GitHub => github(req),
            KnownForge::GitLab => gitlab(req),
            KnownForge::SourceHut => sourcehut(req),
            KnownForge::Bitbucket => bitbucket(req),
            KnownForge::Codeberg => codeberg(req),
        }
    }

    fn project_url(&self, host: &str, dir: &str) -> String {
        format!("https://{host}/{dir}")
    }
}

fn git_ref_str(git_ref: &GitRef) -> &str {
    match git_ref {
        GitRef::Branch(b) => b,
        GitRef::Commit(c) => c,
    }
}

fn github(req: &LinkRequest) -> String {
    let encoded_ref = encode(git_ref_str(&req.git_ref));
    let mut url = format!(
        "https://{}/{}/blob/{}/{}",
        req.host,
        req.dir,
        encoded_ref,
        encode(&req.file)
    );
    if let Some(lines) = &req.lines {
        match lines {
            Lines::Single(n) => write!(url, "#L{n}").unwrap(),
            Lines::Range(s, e) => write!(url, "#L{s}-L{e}").unwrap(),
        }
    }
    url
}

fn gitlab(req: &LinkRequest) -> String {
    let encoded_ref = encode(git_ref_str(&req.git_ref));
    let mut url = format!(
        "https://{}/{}/-/blob/{}/{}",
        req.host,
        req.dir,
        encoded_ref,
        encode(&req.file)
    );
    if let Some(lines) = &req.lines {
        match lines {
            Lines::Single(n) => write!(url, "#L{n}").unwrap(),
            Lines::Range(s, e) => write!(url, "#L{s}-{e}").unwrap(),
        }
    }
    url
}

fn sourcehut(req: &LinkRequest) -> String {
    let encoded_ref = encode(git_ref_str(&req.git_ref));
    let mut url = format!(
        "https://{}/{}/tree/{}/{}",
        req.host,
        req.dir,
        encoded_ref,
        encode(&req.file)
    );
    if let Some(lines) = &req.lines {
        match lines {
            Lines::Single(n) => write!(url, "#L{n}").unwrap(),
            Lines::Range(s, e) => write!(url, "#L{s}-{e}").unwrap(),
        }
    }
    url
}

fn bitbucket(req: &LinkRequest) -> String {
    let encoded_ref = encode(git_ref_str(&req.git_ref));
    let basename = req.file.rsplit('/').next().unwrap_or(&req.file);
    let basename = encode(basename);
    let mut url = format!(
        "https://{}/{}/src/{}/{}",
        req.host,
        req.dir,
        encoded_ref,
        encode(&req.file)
    );
    if let Some(lines) = &req.lines {
        match lines {
            Lines::Single(n) => write!(url, "#{basename}-{n}").unwrap(),
            Lines::Range(s, e) => write!(url, "#{basename}-{s}:{e}").unwrap(),
        }
    }
    url
}

fn codeberg(req: &LinkRequest) -> String {
    let ref_segment = match &req.git_ref {
        GitRef::Branch(b) => encode(b),
        GitRef::Commit(c) => format!("commit/{}", encode(c)),
    };
    let mut url = format!(
        "https://{}/{}/src/{}/{}",
        req.host,
        req.dir,
        ref_segment,
        encode(&req.file)
    );
    if let Some(lines) = &req.lines {
        match lines {
            Lines::Single(n) => write!(url, "#L{n}").unwrap(),
            Lines::Range(s, e) => write!(url, "#L{s}-L{e}").unwrap(),
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GitRef, Lines, LinkRequest};

    fn req(host: &str, dir: &str, file: &str, git_ref: GitRef) -> LinkRequest {
        LinkRequest {
            host: host.into(),
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
        assert!(detect("github.com").is_some());
        assert!(detect("github.enterprise.com").is_some());
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
    }

    #[test]
    fn detects_bitbucket() {
        assert!(detect("bitbucket.org").is_some());
    }

    #[test]
    fn detects_codeberg() {
        assert!(detect("codeberg.org").is_some());
        assert!(detect("forge.fedoraproject.org").is_some());
    }

    #[test]
    fn unknown_host_returns_none() {
        assert!(detect("example.com").is_none());
    }

    // --- github ---

    #[test]
    fn github_commit_no_lines() {
        let forge = detect("github.com").unwrap();
        let request = req("github.com", "user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/main.rs"
        );
    }

    #[test]
    fn github_commit_single_line() {
        let forge = detect("github.com").unwrap();
        let mut request = req("github.com", "user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(42)));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/main.rs#L42"
        );
    }

    #[test]
    fn github_commit_line_range() {
        let forge = detect("github.com").unwrap();
        let mut request = req("github.com", "user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(42), nz(55)));
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/main.rs#L42-L55"
        );
    }

    #[test]
    fn github_encodes_special_chars() {
        let forge = detect("github.com").unwrap();
        let request = req(
            "github.com",
            "user/repo",
            "src/my file.rs",
            commit("abc123"),
        );
        assert_eq!(
            forge.file_url(&request),
            "https://github.com/user/repo/blob/abc123/src/my%20file.rs"
        );
    }

    // --- gitlab ---

    #[test]
    fn gitlab_commit_no_lines() {
        let forge = detect("gitlab.com").unwrap();
        let request = req("gitlab.com", "user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs"
        );
    }

    #[test]
    fn gitlab_commit_line_range() {
        let forge = detect("gitlab.com").unwrap();
        let mut request = req("gitlab.com", "user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(10), nz(20)));
        assert_eq!(
            forge.file_url(&request),
            "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs#L10-20"
        );
    }

    // --- sourcehut ---

    #[test]
    fn sourcehut_commit_no_lines() {
        let forge = detect("git.sr.ht").unwrap();
        let request = req("git.sr.ht", "~user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs"
        );
    }

    #[test]
    fn sourcehut_commit_line_range() {
        let forge = detect("git.sr.ht").unwrap();
        let mut request = req("git.sr.ht", "~user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(5), nz(15)));
        assert_eq!(
            forge.file_url(&request),
            "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs#L5-15"
        );
    }

    // --- bitbucket ---

    #[test]
    fn bitbucket_commit_no_lines() {
        let forge = detect("bitbucket.org").unwrap();
        let request = req(
            "bitbucket.org",
            "user/repo",
            "src/main.rs",
            commit("abc123"),
        );
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/abc123/src/main.rs"
        );
    }

    #[test]
    fn bitbucket_commit_line_range() {
        let forge = detect("bitbucket.org").unwrap();
        let mut request = req(
            "bitbucket.org",
            "user/repo",
            "src/main.rs",
            commit("abc123"),
        );
        request.lines = Some(Lines::Range(nz(10), nz(20)));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/abc123/src/main.rs#main.rs-10:20"
        );
    }

    #[test]
    fn bitbucket_branch_no_lines() {
        let forge = detect("bitbucket.org").unwrap();
        let request = req("bitbucket.org", "user/repo", "src/main.rs", branch("main"));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/main/src/main.rs"
        );
    }

    // --- codeberg ---

    #[test]
    fn codeberg_branch_no_lines() {
        let forge = detect("codeberg.org").unwrap();
        let request = req("codeberg.org", "user/repo", "src/main.rs", branch("main"));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/main/src/main.rs"
        );
    }

    #[test]
    fn codeberg_commit_prefixes_path() {
        let forge = detect("codeberg.org").unwrap();
        let request = req("codeberg.org", "user/repo", "src/main.rs", commit("abc123"));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs"
        );
    }

    #[test]
    fn codeberg_commit_line_range() {
        let forge = detect("codeberg.org").unwrap();
        let mut request = req("codeberg.org", "user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Range(nz(1), nz(10)));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs#L1-L10"
        );
    }

    #[test]
    fn gitlab_single_line() {
        let forge = detect("gitlab.com").unwrap();
        let mut request = req("gitlab.com", "user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs#L7"
        );
    }

    #[test]
    fn sourcehut_single_line() {
        let forge = detect("git.sr.ht").unwrap();
        let mut request = req("git.sr.ht", "~user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs#L7"
        );
    }

    #[test]
    fn bitbucket_single_line() {
        let forge = detect("bitbucket.org").unwrap();
        let mut request = req(
            "bitbucket.org",
            "user/repo",
            "src/main.rs",
            commit("abc123"),
        );
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://bitbucket.org/user/repo/src/abc123/src/main.rs#main.rs-7"
        );
    }

    #[test]
    fn codeberg_single_line() {
        let forge = detect("codeberg.org").unwrap();
        let mut request = req("codeberg.org", "user/repo", "src/main.rs", commit("abc123"));
        request.lines = Some(Lines::Single(nz(7)));
        assert_eq!(
            forge.file_url(&request),
            "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs#L7"
        );
    }

    #[test]
    fn branch_with_slash_keeps_slash() {
        let forge = detect("github.com").unwrap();
        let request = req(
            "github.com",
            "user/repo",
            "src/main.rs",
            branch("feature/x"),
        );
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
        let forge = detect("github.com").unwrap();
        assert_eq!(
            forge.project_url("github.com", "user/repo"),
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
            let forge = detect(host).unwrap();
            assert_eq!(
                forge.project_url(host, "user/repo"),
                format!("https://{host}/user/repo")
            );
        }
    }

    #[test]
    fn fedora_forge_uses_codeberg_format() {
        let forge = detect("forge.fedoraproject.org").unwrap();
        let request = req(
            "forge.fedoraproject.org",
            "user/repo",
            "src/main.rs",
            commit("abc123"),
        );
        assert_eq!(
            forge.file_url(&request),
            "https://forge.fedoraproject.org/user/repo/src/commit/abc123/src/main.rs"
        );
    }
}
