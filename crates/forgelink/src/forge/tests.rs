use super::*;
use crate::{GitRef, Lines, LinkRequest};

fn target(host: &str, forge: Forge) -> ForgeTarget {
    ForgeTarget::new(&format!("https://{host}"), forge).unwrap()
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

// --- parsing ---

#[test]
fn parses_forge_names() {
    for (name, expected) in [
        ("github", Forge::GitHub),
        ("gitlab", Forge::GitLab),
        ("sourcehut", Forge::SourceHut),
        ("bitbucket", Forge::Bitbucket),
        ("codeberg", Forge::Codeberg),
        ("tangled", Forge::Tangled),
        ("azure-devops", Forge::AzureDevOps),
    ] {
        assert_eq!(name.parse::<Forge>().unwrap(), expected);
    }
}

#[test]
fn rejects_unknown_forge_name() {
    assert!("unknown".parse::<Forge>().is_err());
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
fn detects_tangled() {
    assert_eq!(detect("tangled.org"), Some(Forge::Tangled));
    assert_eq!(detect("TANGLED.ORG"), Some(Forge::Tangled));
    assert!(detect("subdomain.tangled.org").is_none());
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
fn detects_azure_devops_web_host() {
    assert_eq!(detect("dev.azure.com"), Some(Forge::AzureDevOps));
}

#[test]
fn detects_azure_devops_ssh_host() {
    assert_eq!(detect("ssh.dev.azure.com"), Some(Forge::AzureDevOps));
}

#[test]
fn azure_devops_modern_domain_does_not_match_lookalikes() {
    assert!(detect("notdev.azure.com").is_none());
    assert!(detect("dev.azure.com.example.org").is_none());
}

#[test]
fn detects_azure_devops_legacy_web_host() {
    assert_eq!(
        detect("organization.visualstudio.com"),
        Some(Forge::AzureDevOps)
    );
}

#[test]
fn detects_azure_devops_legacy_ssh_host() {
    assert_eq!(detect("vs-ssh.visualstudio.com"), Some(Forge::AzureDevOps));
}

#[test]
fn azure_devops_legacy_domain_does_not_match_lookalikes() {
    assert!(detect("notvisualstudio.com").is_none());
    assert!(detect("visualstudio.com.example.org").is_none());
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

#[test]
fn detects_with_port_in_host() {
    assert!(detect("gitlab.example.com:8443").is_some());
}

// --- github ---

#[test]
fn github_commit_no_lines() {
    let forge = target("github.com", Forge::GitHub);
    let request = req("user/repo", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://github.com/user/repo/blob/abc123/src/main.rs"
    );
}

#[test]
fn github_commit_single_line() {
    let forge = target("github.com", Forge::GitHub);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Single(nz(42)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://github.com/user/repo/blob/abc123/src/main.rs#L42"
    );
}

#[test]
fn github_commit_line_range() {
    let forge = target("github.com", Forge::GitHub);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Range(nz(42), nz(55)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://github.com/user/repo/blob/abc123/src/main.rs#L42-L55"
    );
}

#[test]
fn github_encodes_special_chars() {
    let forge = target("github.com", Forge::GitHub);
    let request = req("user/repo", "src/my file.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://github.com/user/repo/blob/abc123/src/my%20file.rs"
    );
}

#[test]
fn github_encodes_rfc3986_path_characters() {
    let forge = target("github.com", Forge::GitHub);
    let request = req(
        "user/repo",
        "app/[slug]/:@!$&'()*+,;=-._~ café?#%.tsx",
        commit("abc123"),
    );
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://github.com/user/repo/blob/abc123/app/%5Bslug%5D/",
            ":@!$&'()*+,;=-._~%20caf%C3%A9%3F%23%25.tsx"
        )
    );
}

#[test]
fn branch_with_slash_keeps_slash() {
    let forge = target("github.com", Forge::GitHub);
    let request = req("user/repo", "src/main.rs", branch("feature/x"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://github.com/user/repo/blob/feature/x/src/main.rs"
    );
}

// --- gitlab ---

#[test]
fn gitlab_commit_no_lines() {
    let forge = target("gitlab.com", Forge::GitLab);
    let request = req("user/repo", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs"
    );
}

#[test]
fn gitlab_single_line() {
    let forge = target("gitlab.com", Forge::GitLab);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Single(nz(7)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs#L7"
    );
}

#[test]
fn gitlab_commit_line_range() {
    let forge = target("gitlab.com", Forge::GitLab);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Range(nz(10), nz(20)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://gitlab.com/user/repo/-/blob/abc123/src/main.rs#L10-20"
    );
}

// --- tangled ---

#[test]
fn tangled_commit_no_lines() {
    let forge = target("tangled.org", Forge::Tangled);
    let request = req("user/repo", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://tangled.org/user/repo/blob/abc123/src/main.rs"
    );
}

#[test]
fn tangled_commit_single_line() {
    let forge = target("tangled.org", Forge::Tangled);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Single(nz(42)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://tangled.org/user/repo/blob/abc123/src/main.rs#L42"
    );
}

#[test]
fn tangled_commit_line_range() {
    let forge = target("tangled.org", Forge::Tangled);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Range(nz(42), nz(55)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://tangled.org/user/repo/blob/abc123/src/main.rs#L42-55"
    );
}

// --- sourcehut ---

#[test]
fn sourcehut_commit_no_lines() {
    let forge = target("git.sr.ht", Forge::SourceHut);
    let request = req("~user/repo", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs"
    );
}

#[test]
fn sourcehut_single_line() {
    let forge = target("git.sr.ht", Forge::SourceHut);
    let mut request = req("~user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Single(nz(7)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs#L7"
    );
}

#[test]
fn sourcehut_commit_line_range() {
    let forge = target("git.sr.ht", Forge::SourceHut);
    let mut request = req("~user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Range(nz(5), nz(15)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://git.sr.ht/~user/repo/tree/abc123/src/main.rs#L5-15"
    );
}

// --- bitbucket ---

#[test]
fn bitbucket_commit_no_lines() {
    let forge = target("bitbucket.org", Forge::Bitbucket);
    let request = req("user/repo", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://bitbucket.org/user/repo/src/abc123/src/main.rs"
    );
}

#[test]
fn bitbucket_single_line() {
    let forge = target("bitbucket.org", Forge::Bitbucket);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Single(nz(7)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://bitbucket.org/user/repo/src/abc123/src/main.rs#main.rs-7"
    );
}

#[test]
fn bitbucket_commit_line_range() {
    let forge = target("bitbucket.org", Forge::Bitbucket);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Range(nz(10), nz(20)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://bitbucket.org/user/repo/src/abc123/src/main.rs#main.rs-10:20"
    );
}

#[test]
fn bitbucket_encodes_special_chars_in_line_anchor() {
    let forge = target("bitbucket.org", Forge::Bitbucket);
    let mut request = req("user/repo", "src/a:b?#%.rs", commit("abc123"));
    request.lines = Some(Lines::Single(nz(7)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://bitbucket.org/user/repo/src/abc123/src/a:b%3F%23%25.rs#a%3Ab%3F%23%25.rs-7"
    );
}

#[test]
fn bitbucket_branch_no_lines() {
    let forge = target("bitbucket.org", Forge::Bitbucket);
    let request = req("user/repo", "src/main.rs", branch("main"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://bitbucket.org/user/repo/src/main/src/main.rs"
    );
}

// --- codeberg ---

#[test]
fn codeberg_branch_no_lines() {
    let forge = target("codeberg.org", Forge::Codeberg);
    let request = req("user/repo", "src/main.rs", branch("main"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://codeberg.org/user/repo/src/main/src/main.rs"
    );
}

#[test]
fn codeberg_commit_prefixes_path() {
    let forge = target("codeberg.org", Forge::Codeberg);
    let request = req("user/repo", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs"
    );
}

#[test]
fn codeberg_single_line() {
    let forge = target("codeberg.org", Forge::Codeberg);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Single(nz(7)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs#L7"
    );
}

#[test]
fn codeberg_commit_line_range() {
    let forge = target("codeberg.org", Forge::Codeberg);
    let mut request = req("user/repo", "src/main.rs", commit("abc123"));
    request.lines = Some(Lines::Range(nz(1), nz(10)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://codeberg.org/user/repo/src/commit/abc123/src/main.rs#L1-L10"
    );
}

#[test]
fn fedora_forge_uses_codeberg_format() {
    let forge = target("forge.fedoraproject.org", Forge::Codeberg);
    let request = req("user/repo", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        "https://forge.fedoraproject.org/user/repo/src/commit/abc123/src/main.rs"
    );
}

// --- azure devops ---

#[test]
fn azure_devops_commit_no_lines() {
    let forge = target("dev.azure.com", Forge::AzureDevOps);
    let request = req(
        "organization/project/_git/repository",
        "src/main.rs",
        commit("abc123"),
    );
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://dev.azure.com/organization/project/_git/repository",
            "?path=%2Fsrc%2Fmain.rs&version=GCabc123&_a=contents"
        )
    );
}

#[test]
fn azure_devops_branch_no_lines() {
    let forge = target("dev.azure.com", Forge::AzureDevOps);
    let request = req(
        "organization/project/_git/repository",
        "src/main.rs",
        branch("main"),
    );
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://dev.azure.com/organization/project/_git/repository",
            "?path=%2Fsrc%2Fmain.rs&version=GBmain&_a=contents"
        )
    );
}

#[test]
fn azure_devops_encodes_slash_in_branch() {
    let forge = target("dev.azure.com", Forge::AzureDevOps);
    let request = req(
        "organization/project/_git/repository",
        "src/main.rs",
        branch("feature/x"),
    );
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://dev.azure.com/organization/project/_git/repository",
            "?path=%2Fsrc%2Fmain.rs&version=GBfeature%2Fx&_a=contents"
        )
    );
}

#[test]
fn azure_devops_encodes_special_chars_in_file_path() {
    let forge = target("dev.azure.com", Forge::AzureDevOps);
    let request = req(
        "organization/project/_git/repository",
        "src/my file?#%.rs",
        commit("abc123"),
    );
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://dev.azure.com/organization/project/_git/repository",
            "?path=%2Fsrc%2Fmy+file%3F%23%25.rs&version=GCabc123&_a=contents"
        )
    );
}

#[test]
fn azure_devops_single_line() {
    let forge = target("dev.azure.com", Forge::AzureDevOps);
    let mut request = req(
        "organization/project/_git/repository",
        "src/main.rs",
        commit("abc123"),
    );
    request.lines = Some(Lines::Single(nz(42)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://dev.azure.com/organization/project/_git/repository",
            "?path=%2Fsrc%2Fmain.rs&version=GCabc123&line=42&lineEnd=43",
            "&lineStartColumn=1&lineEndColumn=1&lineStyle=plain&_a=contents"
        )
    );
}

#[test]
fn azure_devops_line_range() {
    let forge = target("dev.azure.com", Forge::AzureDevOps);
    let mut request = req(
        "organization/project/_git/repository",
        "src/main.rs",
        commit("abc123"),
    );
    request.lines = Some(Lines::Range(nz(42), nz(55)));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://dev.azure.com/organization/project/_git/repository",
            "?path=%2Fsrc%2Fmain.rs&version=GCabc123&line=42&lineEnd=56",
            "&lineStartColumn=1&lineEndColumn=1&lineStyle=plain&_a=contents"
        )
    );
}

#[test]
fn azure_devops_supports_custom_base_path() {
    let forge = ForgeTarget::new(
        "https://company.tld/tfs/DefaultCollection",
        Forge::AzureDevOps,
    )
    .unwrap();
    let request = req("project/_git/repository", "src/main.rs", commit("abc123"));
    assert_eq!(
        forge.file_url(&request).unwrap(),
        concat!(
            "https://company.tld/tfs/DefaultCollection/project/_git/repository",
            "?path=%2Fsrc%2Fmain.rs&version=GCabc123&_a=contents"
        )
    );
}

// --- project_url ---

#[test]
fn project_url_github() {
    let forge = target("github.com", Forge::GitHub);
    assert_eq!(
        forge.project_url("user/repo").unwrap(),
        "https://github.com/user/repo"
    );
}

#[test]
fn project_url_all_forges_same_format() {
    for (host, kind) in [
        ("github.com", Forge::GitHub),
        ("gitlab.com", Forge::GitLab),
        ("git.sr.ht", Forge::SourceHut),
        ("bitbucket.org", Forge::Bitbucket),
        ("codeberg.org", Forge::Codeberg),
        ("tangled.org", Forge::Tangled),
        ("dev.azure.com", Forge::AzureDevOps),
    ] {
        let forge = target(host, kind);
        assert_eq!(
            forge.project_url("user/repo").unwrap(),
            format!("https://{host}/user/repo")
        );
    }
}
