use super::*;
use std::fs;
use std::io::Write;

fn init_repo(url: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    gix::init(dir.path()).unwrap();
    let config = dir.path().join(".git").join("config");
    let mut f = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(
        f,
        "[user]\n\tname = Test\n\temail = test@example.com\n[remote \"origin\"]\n\turl = {url}"
    )
    .unwrap();
    dir
}

#[cfg(feature = "sha256")]
fn init_sha256_repo(url: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let options = gix::create::Options {
        object_hash: Some(gix::hash::Kind::Sha256),
        ..Default::default()
    };
    gix::ThreadSafeRepository::init(dir.path(), gix::create::Kind::WithWorktree, options).unwrap();
    let config = dir.path().join(".git").join("config");
    let mut f = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(
        f,
        "[user]\n\tname = Test\n\temail = test@example.com\n[remote \"origin\"]\n\turl = {url}"
    )
    .unwrap();
    dir
}

fn commit_empty(dir: &Path) {
    let repo = gix::open(dir).unwrap();
    let tree = repo
        .write_object(gix::objs::Tree::default())
        .unwrap()
        .detach();
    repo.commit("HEAD", "init", tree, gix::commit::NO_PARENT_IDS)
        .unwrap();
}

fn automatic_target(_: &RemoteInfo) -> Option<ForgeTarget> {
    None
}

#[test]
fn build_link_end_to_end() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "fn main() {}").unwrap();

    let lines = Lines::single(NonZero::new(3).unwrap());
    let url = build_link(
        dir.path(),
        "origin",
        "src/main.rs",
        Some(lines),
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(
        url.starts_with("https://github.com/user/repo/blob/"),
        "got {url}"
    );
    assert!(url.ends_with("/src/main.rs#L3"), "got {url}");
}

#[cfg(feature = "sha256")]
#[test]
fn build_link_supports_sha256_repositories() {
    let dir = init_sha256_repo("https://gitlab.com/user/repo.git");
    commit_empty(dir.path());
    fs::write(dir.path().join("main.rs"), "").unwrap();

    let url = build_link(
        dir.path(),
        "origin",
        "main.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    let repo = remote::discover(dir.path()).unwrap();
    let GitRef::Commit(commit) = remote::head_commit(&repo).unwrap() else {
        panic!("expected a commit");
    };
    assert_eq!(commit.len(), 64);
    assert_eq!(
        url,
        format!("https://gitlab.com/user/repo/-/blob/{commit}/main.rs")
    );
}

#[test]
fn build_link_supports_relative_linked_worktrees() {
    let main = init_repo("https://github.com/user/repo.git");
    commit_empty(main.path());

    let linked = main.path().join("linked");
    fs::create_dir(&linked).unwrap();
    fs::write(linked.join("main.rs"), "").unwrap();

    let branch = gix::open(main.path())
        .unwrap()
        .head_name()
        .unwrap()
        .expect("symbolic HEAD");
    let private_git_dir = main.path().join(".git/worktrees/linked");
    fs::create_dir_all(&private_git_dir).unwrap();
    fs::write(linked.join(".git"), "gitdir: ../.git/worktrees/linked\n").unwrap();
    fs::write(private_git_dir.join("commondir"), "../..\n").unwrap();
    fs::write(private_git_dir.join("gitdir"), "../../../linked/.git\n").unwrap();
    fs::write(private_git_dir.join("HEAD"), format!("ref: {branch}\n")).unwrap();

    let config = main.path().join(".git/config");
    let mut config = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(
        config,
        "[core]\n\trepositoryformatversion = 1\n[extensions]\n\trelativeWorktrees = true"
    )
    .unwrap();

    let url = build_link(
        &linked,
        "origin",
        "main.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    let repo = remote::discover(&linked).unwrap();
    assert_eq!(
        remote::root(&repo).unwrap(),
        gix::path::realpath(&linked).unwrap()
    );
    let GitRef::Commit(commit) = remote::head_commit(&repo).unwrap() else {
        panic!("expected a commit");
    };
    assert_eq!(
        url,
        format!("https://github.com/user/repo/blob/{commit}/main.rs")
    );
}

#[test]
fn build_link_works_for_nonexistent_file() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());

    let url = build_link(
        dir.path(),
        "origin",
        "src/ghost.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(url.ends_with("/src/ghost.rs"), "got {url}");
}

#[test]
fn build_link_resolves_relative_path_from_subdirectory() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("main.rs"), "").unwrap();

    let url = build_link(
        &src,
        "origin",
        "main.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(url.ends_with("/src/main.rs"), "got {url}");
}

#[test]
fn build_link_normalizes_nonexistent_component_followed_by_parent() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("main.rs"), "").unwrap();

    let url = build_link(
        dir.path(),
        "origin",
        "src/missing/../main.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(url.ends_with("/src/main.rs"), "got {url}");
}

#[test]
fn build_link_absolute_path_uses_target_repository() {
    let current = init_repo("https://github.com/current/repo.git");
    commit_empty(current.path());
    let target = init_repo("https://github.com/target/repo.git");
    commit_empty(target.path());
    let file = target.path().join("src").join("main.rs");
    fs::create_dir(file.parent().unwrap()).unwrap();
    fs::write(&file, "").unwrap();

    let url = build_link(
        current.path(),
        "origin",
        file.to_str().unwrap(),
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(
        url.starts_with("https://github.com/target/repo/blob/"),
        "got {url}"
    );
    assert!(url.ends_with("/src/main.rs"), "got {url}");
}

#[cfg(unix)]
#[test]
fn build_link_resolves_symlink_within_repository() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("main.rs"), "").unwrap();
    std::os::unix::fs::symlink(&src, dir.path().join("source")).unwrap();

    let url = build_link(
        dir.path(),
        "origin",
        "source/main.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(url.ends_with("/src/main.rs"), "got {url}");
}

#[cfg(unix)]
#[test]
fn build_link_preserves_final_symlink() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("main.rs"), "").unwrap();
    std::os::unix::fs::symlink("main.rs", src.join("link.rs")).unwrap();

    let url = build_link(
        dir.path(),
        "origin",
        "src/link.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(url.ends_with("/src/link.rs"), "got {url}");
}

#[cfg(unix)]
#[test]
fn build_link_does_not_link_through_symlink_outside_repository() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("stray.rs"), "").unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("outside")).unwrap();

    let result = build_link(
        dir.path(),
        "origin",
        "outside/stray.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    );

    assert!(matches!(result, Err(Error::FileOutsideRepository(_))));
}

#[test]
fn build_link_rejects_file_outside_repo() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let outside = tempfile::tempdir().unwrap();
    let stray = outside.path().join("stray.rs");
    fs::write(&stray, "").unwrap();

    let err = build_link(
        dir.path(),
        "origin",
        stray.to_str().unwrap(),
        None,
        RefSpec::Commit,
        automatic_target,
    );
    assert!(err.is_err());
}

#[test]
fn build_link_rejects_nonexistent_path_traversal_outside_repo() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let root = dir.path().canonicalize().unwrap();

    let err = build_link(
        &root,
        "origin",
        "missing/../../outside.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    );

    assert!(matches!(err, Err(Error::FileOutsideRepository(_))));
}

#[test]
fn build_link_branch_uses_branch_name() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let repo = remote::discover(dir.path()).unwrap();
    let GitRef::Branch(branch) = remote::current_branch(&repo).unwrap() else {
        panic!("expected a branch");
    };

    let url = build_link(
        dir.path(),
        "origin",
        "src/main.rs",
        None,
        RefSpec::Branch,
        automatic_target,
    )
    .unwrap();

    assert_eq!(
        url,
        format!("https://github.com/user/repo/blob/{branch}/src/main.rs")
    );
}

#[test]
fn build_link_branch_errors_on_detached_head() {
    let dir = init_repo("https://github.com/user/repo.git");
    commit_empty(dir.path());
    let repo = gix::open(dir.path()).unwrap();
    let sha = repo.head_commit().unwrap().id.to_hex().to_string();
    fs::write(dir.path().join(".git").join("HEAD"), format!("{sha}\n")).unwrap();

    let err = build_link(
        dir.path(),
        "origin",
        "src/main.rs",
        None,
        RefSpec::Branch,
        automatic_target,
    );
    assert!(matches!(err, Err(Error::DetachedHead)));
}

#[test]
fn project_link_end_to_end() {
    let dir = init_repo("git@github.com:user/repo.git");
    let url = project_link(dir.path(), "origin", automatic_target).unwrap();
    assert_eq!(url, "https://github.com/user/repo");
}

#[test]
fn project_link_does_not_reuse_ssh_port_for_https() {
    let dir = init_repo("ssh://git@github.com:2222/user/repo.git");
    let url = project_link(dir.path(), "origin", automatic_target).unwrap();
    assert_eq!(url, "https://github.com/user/repo");
}

#[test]
fn project_link_applies_git_url_rewrites() {
    let dir = init_repo("forgelink-test-gh:user/repo.git");
    let config = dir.path().join(".git").join("config");
    let mut file = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(
        file,
        "[url \"git@github.com:\"]\n\tinsteadOf = forgelink-test-gh:"
    )
    .unwrap();

    let url = project_link(dir.path(), "origin", automatic_target).unwrap();

    assert_eq!(url, "https://github.com/user/repo");
}

#[test]
fn project_link_reports_invalid_remote_configuration() {
    let dir = init_repo("git@github.com:user/repo.git");
    let config = dir.path().join(".git").join("config");
    let mut file = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(file, "[remote \"origin\"]\n\tfetch = ^").unwrap();

    let error = project_link(dir.path(), "origin", automatic_target).unwrap_err();

    assert!(matches!(
        &error,
        Error::InvalidRemote { name, .. } if name == "origin"
    ));
    assert_eq!(error.to_string(), "invalid 'origin' remote");
    let source = <Error as std::error::Error>::source(&error).unwrap();
    assert!(source.to_string().contains("fetch ref-spec"));
}

#[test]
fn project_link_reports_invalid_remote_tag_option() {
    let dir = init_repo("git@github.com:user/repo.git");
    let config = dir.path().join(".git").join("config");
    let mut file = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(file, "[remote \"origin\"]\n\ttagOpt = invalid").unwrap();

    let error = project_link(dir.path(), "origin", automatic_target).unwrap_err();

    assert!(matches!(
        &error,
        Error::InvalidRemote { name, .. } if name == "origin"
    ));
    let source = <Error as std::error::Error>::source(&error).unwrap();
    assert!(source.to_string().contains("tagOpt"));
}

#[test]
fn project_link_reports_invalid_remote_url() {
    let dir = init_repo("https://[");

    let error = project_link(dir.path(), "origin", automatic_target).unwrap_err();

    assert!(matches!(error, Error::InvalidRemoteUrl(_)));
}

#[test]
fn project_link_reports_invalid_push_url() {
    let dir = init_repo("git@github.com:user/repo.git");
    let config = dir.path().join(".git").join("config");
    let mut file = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(file, "[remote \"origin\"]\n\tpushUrl = https://[").unwrap();

    let error = project_link(dir.path(), "origin", automatic_target).unwrap_err();

    assert!(matches!(error, Error::InvalidRemoteUrl(_)));
}

#[test]
fn project_link_reports_invalid_rewritten_remote_url() {
    let dir = init_repo("forgelink-test-invalid-rewrite:user/repo.git");
    let config = dir.path().join(".git").join("config");
    let mut file = fs::OpenOptions::new().append(true).open(config).unwrap();
    writeln!(
        file,
        "[url \"https://[\"]\n\tinsteadOf = forgelink-test-invalid-rewrite:"
    )
    .unwrap();

    let error = project_link(dir.path(), "origin", automatic_target).unwrap_err();

    assert!(matches!(
        error,
        Error::InvalidRemoteUrl(message) if message.contains("rewritten fetch url")
    ));
}

#[test]
fn build_link_accepts_target_for_ssh_alias() {
    let dir = init_repo("git@gh-work:user/repo.git");
    commit_empty(dir.path());
    let expected = RemoteInfo {
        hostname: "gh-work".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "user/repo".to_string(),
    };

    let url = build_link(
        dir.path(),
        "origin",
        "src/main.rs",
        None,
        RefSpec::Commit,
        move |actual| {
            assert_eq!(&expected, actual);
            drop(expected);
            Some(ForgeTarget::new("https://github.com", Forge::GitHub).unwrap())
        },
    )
    .unwrap();

    assert!(
        url.starts_with("https://github.com/user/repo/blob/"),
        "got {url}"
    );
    assert!(url.ends_with("/src/main.rs"), "got {url}");
}

#[test]
fn project_link_accepts_enterprise_target() {
    let dir = init_repo("git@git.company.tld:group/repo.git");

    let url = project_link(dir.path(), "origin", |remote| {
        assert_eq!(remote.hostname(), "git.company.tld");
        Some(ForgeTarget::new("https://company.tld/services/gitlab", Forge::GitLab).unwrap())
    })
    .unwrap();

    assert_eq!(url, "https://company.tld/services/gitlab/group/repo");
}

#[test]
fn unknown_host_without_target_still_errors() {
    let dir = init_repo("git@internal:user/repo.git");
    let error = project_link(dir.path(), "origin", automatic_target).unwrap_err();

    assert!(matches!(error, Error::UnknownForge(host) if host == "internal"));
}

#[test]
fn azure_devops_ssh_uses_web_host() {
    let remote = RemoteInfo {
        hostname: "ssh.dev.azure.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "v3/organization/project/repository".to_string(),
    };

    let resolved = resolve_remote(&remote, automatic_target).unwrap();

    assert_eq!(resolved.target.base_url(), "https://dev.azure.com/");
}

#[test]
fn azure_devops_ssh_normalizes_repository_path() {
    let remote = RemoteInfo {
        hostname: "ssh.dev.azure.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "v3/organization/project/repository".to_string(),
    };

    let resolved = resolve_remote(&remote, automatic_target).unwrap();

    assert_eq!(resolved.repository, "organization/project/_git/repository");
}

#[test]
fn azure_devops_legacy_ssh_uses_web_host() {
    let remote = RemoteInfo {
        hostname: "vs-ssh.visualstudio.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "v3/organization/project/repository".to_string(),
    };

    let resolved = resolve_remote(&remote, automatic_target).unwrap();

    assert_eq!(
        resolved.target.base_url(),
        "https://organization.visualstudio.com/"
    );
}

#[test]
fn azure_devops_legacy_ssh_normalizes_repository_path() {
    let remote = RemoteInfo {
        hostname: "vs-ssh.visualstudio.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "v3/organization/project/repository".to_string(),
    };

    let resolved = resolve_remote(&remote, automatic_target).unwrap();

    assert_eq!(resolved.repository, "project/_git/repository");
}

#[test]
fn azure_devops_https_preserves_web_location() {
    let remote = RemoteInfo {
        hostname: "dev.azure.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Https,
        repository: "organization/project/_git/repository".to_string(),
    };

    let resolved = resolve_remote(&remote, automatic_target).unwrap();

    assert_eq!(resolved.target.base_url(), "https://dev.azure.com/");
    assert_eq!(resolved.repository, remote.repository);
}

#[test]
fn azure_devops_https_organization_named_v3_is_not_an_ssh_path() {
    let remote = RemoteInfo {
        hostname: "dev.azure.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Https,
        repository: "v3/project/_git/repository".to_string(),
    };

    let resolved = resolve_remote(&remote, automatic_target).unwrap();

    assert_eq!(resolved.repository, remote.repository);
}

#[test]
fn azure_devops_legacy_https_preserves_web_location() {
    let remote = RemoteInfo {
        hostname: "organization.visualstudio.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Https,
        repository: "DefaultCollection/project/_git/repository".to_string(),
    };

    let resolved = resolve_remote(&remote, automatic_target).unwrap();

    assert_eq!(
        resolved.target.base_url(),
        "https://organization.visualstudio.com/"
    );
    assert_eq!(resolved.repository, remote.repository);
}

#[test]
fn azure_devops_rejects_malformed_ssh_repository_path() {
    let remote = RemoteInfo {
        hostname: "ssh.dev.azure.com".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "organization/project/repository".to_string(),
    };

    let error = resolve_remote(&remote, automatic_target).err().unwrap();

    assert!(matches!(error, Error::InvalidRemoteUrl(_)));
}

#[test]
fn azure_devops_target_normalizes_ssh_alias_repository_path() {
    let remote = RemoteInfo {
        hostname: "ado-work".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "v3/organization/project/repository".to_string(),
    };
    let target = ForgeTarget::new("https://dev.azure.com", Forge::AzureDevOps).unwrap();

    let resolved = resolve_remote(&remote, |_| Some(target)).unwrap();

    assert_eq!(resolved.repository, "organization/project/_git/repository");
}

#[test]
fn azure_devops_legacy_target_normalizes_ssh_alias_repository_path() {
    let remote = RemoteInfo {
        hostname: "ado-work".to_string(),
        port: None,
        scheme: gix::url::Scheme::Ssh,
        repository: "v3/organization/project/repository".to_string(),
    };
    let target =
        ForgeTarget::new("https://organization.visualstudio.com", Forge::AzureDevOps).unwrap();

    let resolved = resolve_remote(&remote, |_| Some(target)).unwrap();

    assert_eq!(resolved.repository, "project/_git/repository");
}

#[test]
fn project_link_azure_devops_ssh_end_to_end() {
    let dir = init_repo("git@ssh.dev.azure.com:v3/organization/project/repository");

    let url = project_link(dir.path(), "origin", automatic_target).unwrap();

    assert_eq!(
        url,
        "https://dev.azure.com/organization/project/_git/repository"
    );
}

#[test]
fn project_link_azure_devops_ssh_decodes_percent_encoded_names() {
    let dir = init_repo("git@ssh.dev.azure.com:v3/organization/My%20Project/My%20Repository");

    let url = project_link(dir.path(), "origin", automatic_target).unwrap();

    assert_eq!(
        url,
        "https://dev.azure.com/organization/My%20Project/_git/My%20Repository"
    );
}

#[test]
fn project_link_azure_devops_custom_collection_ssh_alias_end_to_end() {
    let dir = init_repo("git@ado.company.tld:v3/DefaultCollection/project/repository");
    let target = ForgeTarget::new(
        "https://company.tld/tfs/DefaultCollection",
        Forge::AzureDevOps,
    )
    .unwrap();

    let url = project_link(dir.path(), "origin", |_| Some(target)).unwrap();

    assert_eq!(
        url,
        "https://company.tld/tfs/DefaultCollection/project/_git/repository"
    );
}

#[test]
fn project_link_azure_devops_legacy_ssh_end_to_end() {
    let dir = init_repo("organization@vs-ssh.visualstudio.com:v3/organization/project/repository");

    let url = project_link(dir.path(), "origin", automatic_target).unwrap();

    assert_eq!(
        url,
        "https://organization.visualstudio.com/project/_git/repository"
    );
}

#[test]
fn build_link_azure_devops_ssh_end_to_end() {
    let dir = init_repo("git@ssh.dev.azure.com:v3/organization/project/repository");
    commit_empty(dir.path());

    let url = build_link(
        dir.path(),
        "origin",
        "src/main.rs",
        None,
        RefSpec::Commit,
        automatic_target,
    )
    .unwrap();

    assert!(
        url.starts_with(concat!(
            "https://dev.azure.com/organization/project/_git/repository",
            "?path=%2Fsrc%2Fmain.rs&version=GC"
        )),
        "got {url}"
    );
    assert!(url.ends_with("&_a=contents"), "got {url}");
}

#[test]
fn lines_range_accepts_ascending() {
    let lines = Lines::range(NonZero::new(10).unwrap(), NonZero::new(20).unwrap()).unwrap();
    assert_eq!(lines.start().get(), 10);
    assert_eq!(lines.end().get(), 20);
}

#[test]
fn lines_range_allows_equal() {
    let n = NonZero::new(5).unwrap();
    assert!(Lines::range(n, n).is_ok());
}

#[test]
fn lines_range_rejects_backwards() {
    let err = Lines::range(NonZero::new(20).unwrap(), NonZero::new(10).unwrap());
    assert!(matches!(err, Err(Error::InvalidLineRange { .. })));
}
