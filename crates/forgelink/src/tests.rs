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

fn commit_empty(dir: &Path) {
    let repo = gix::open(dir).unwrap();
    let tree = repo
        .write_object(gix::objs::Tree::default())
        .unwrap()
        .detach();
    repo.commit("HEAD", "init", tree, gix::commit::NO_PARENT_IDS)
        .unwrap();
}

fn automatic_target(_: &str) -> Option<ForgeTarget> {
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
fn build_link_accepts_target_for_ssh_alias() {
    let dir = init_repo("git@gh-work:user/repo.git");
    commit_empty(dir.path());
    let expected_host = "gh-work".to_string();

    let url = build_link(
        dir.path(),
        "origin",
        "src/main.rs",
        None,
        RefSpec::Commit,
        move |host| {
            assert_eq!(host, expected_host);
            drop(expected_host);
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

    let url = project_link(dir.path(), "origin", |host| {
        assert_eq!(host, "git.company.tld");
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
