use iri_string::percent_encode::PercentEncodedForUri;
use percent_encoding::percent_decode_str;

use crate::{Error, ForgeTarget, GitRef, Lines, LinkRequest, RemoteInfo, Result};

/// Looks for an exact match against a known forge host.
///
/// This does not match subdomains.
fn detect_by_known_host(host: &str) -> Option<Forge> {
    [
        ("git.sr.ht", Forge::SourceHut),
        ("codeberg.org", Forge::Codeberg),
        ("forge.fedoraproject.org", Forge::Codeberg),
        ("tangled.org", Forge::Tangled),
    ]
    .into_iter()
    .find_map(|(known, forge)| host.eq_ignore_ascii_case(known).then_some(forge))
}

fn is_domain_or_subdomain(host: &str, domain: &str) -> bool {
    let mut host_labels = host.rsplit('.');
    domain.rsplit('.').all(|domain_label| {
        host_labels
            .next()
            .is_some_and(|host_label| host_label.eq_ignore_ascii_case(domain_label))
    })
}

/// Looks for a forge domain at a complete DNS suffix.
fn detect_by_domain(host: &str) -> Option<Forge> {
    [
        ("dev.azure.com", Forge::AzureDevOps),
        ("visualstudio.com", Forge::AzureDevOps),
    ]
    .into_iter()
    .find_map(|(domain, forge)| is_domain_or_subdomain(host, domain).then_some(forge))
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
    detect_by_domain(host).or_else(|| detect_by_forge_label(host))
}

pub(crate) struct WebLocation {
    pub(crate) base_url: String,
    pub(crate) repository: String,
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
    Tangled,
    AzureDevOps,
}

/// An error returned when parsing an unsupported forge name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unsupported forge '{0}'")]
pub struct ParseForgeError(String);

impl std::str::FromStr for Forge {
    type Err = ParseForgeError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "github" => Ok(Forge::GitHub),
            "gitlab" => Ok(Forge::GitLab),
            "sourcehut" => Ok(Forge::SourceHut),
            "bitbucket" => Ok(Forge::Bitbucket),
            "codeberg" => Ok(Forge::Codeberg),
            "tangled" => Ok(Forge::Tangled),
            "azure-devops" => Ok(Forge::AzureDevOps),
            _ => Err(ParseForgeError(value.to_owned())),
        }
    }
}

impl Forge {
    pub(crate) fn web_location(
        self,
        remote: &RemoteInfo,
        target: Option<&ForgeTarget>,
    ) -> Result<WebLocation> {
        match self {
            Forge::AzureDevOps => azure_devops_web_location(remote, target),
            _ => Ok(default_web_location(remote)),
        }
    }

    pub(crate) fn file_url(self, target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
        match self {
            Forge::GitHub => github(target, req),
            Forge::GitLab => gitlab(target, req),
            Forge::SourceHut => sourcehut(target, req),
            Forge::Bitbucket => bitbucket(target, req),
            Forge::Codeberg => codeberg(target, req),
            Forge::Tangled => tangled(target, req),
            Forge::AzureDevOps => azure_devops(target, req),
        }
    }
}

fn default_web_location(remote: &RemoteInfo) -> WebLocation {
    WebLocation {
        base_url: format!("https://{}", remote.web_authority()),
        repository: remote.repository().to_string(),
    }
}

fn git_ref_str(git_ref: &GitRef) -> &str {
    match git_ref {
        GitRef::Branch(branch) => branch,
        GitRef::Commit(commit) => commit,
    }
}

fn finish(target: &ForgeTarget, path: &str, fragment: Option<String>) -> Result<String> {
    let mut url = target.with_path(path)?;
    url.set_fragment(fragment.as_deref());
    Ok(url.into())
}

fn github(target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
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
    finish(target, &path, fragment)
}

fn gitlab(target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
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
    finish(target, &path, fragment)
}

fn sourcehut(target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
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
    finish(target, &path, fragment)
}

fn bitbucket(target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
    let path = format!("{}/src/{}/{}", req.dir, git_ref_str(&req.git_ref), req.file);
    let basename = req.file.rsplit('/').next().unwrap_or(&req.file);
    let basename = PercentEncodedForUri::unreserve(basename);
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("{basename}-{line}"),
        Lines::Range(start, end) => format!("{basename}-{start}:{end}"),
    });
    finish(target, &path, fragment)
}

fn codeberg(target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
    let path = match &req.git_ref {
        GitRef::Branch(branch) => format!("{}/src/{branch}/{}", req.dir, req.file),
        GitRef::Commit(commit) => format!("{}/src/commit/{commit}/{}", req.dir, req.file),
    };
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("L{line}"),
        Lines::Range(start, end) => format!("L{start}-L{end}"),
    });
    finish(target, &path, fragment)
}

fn tangled(target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
    let path = format!(
        "{}/blob/{}/{}",
        req.dir,
        git_ref_str(&req.git_ref),
        req.file
    );
    let fragment = req.lines.as_ref().map(|lines| match lines {
        Lines::Single(line) => format!("L{line}"),
        Lines::Range(start, end) => format!("L{start}-{end}"),
    });

    finish(target, &path, fragment)
}

fn invalid_azure_devops_ssh_path() -> Error {
    Error::InvalidRemoteUrl("invalid Azure DevOps SSH repository path".to_string())
}

fn decode_azure_devops_path_segment(segment: &str) -> Result<String> {
    percent_decode_str(segment)
        .decode_utf8()
        .map(|segment| segment.into_owned())
        .map_err(|_| invalid_azure_devops_ssh_path())
}

fn azure_devops_target_contains_organization(
    target: &ForgeTarget,
    organization: &str,
) -> Result<bool> {
    let legacy_host = target.url().host_str().is_some_and(|host| {
        !host.eq_ignore_ascii_case("visualstudio.com")
            && is_domain_or_subdomain(host, "visualstudio.com")
    });
    if legacy_host {
        return Ok(true);
    }

    let Some(final_segment) = target
        .url()
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
    else {
        return Ok(false);
    };
    let final_segment = decode_azure_devops_path_segment(final_segment)?;
    Ok(final_segment.eq_ignore_ascii_case(organization))
}

fn azure_devops_web_location(
    remote: &RemoteInfo,
    target: Option<&ForgeTarget>,
) -> Result<WebLocation> {
    let mut location = default_web_location(remote);
    let modern_ssh = remote.hostname().eq_ignore_ascii_case("ssh.dev.azure.com");
    let legacy_ssh = remote
        .hostname()
        .eq_ignore_ascii_case("vs-ssh.visualstudio.com");
    let ssh_path = remote.scheme() == "ssh"
        && remote
            .repository()
            .split('/')
            .next()
            .is_some_and(|segment| segment.eq_ignore_ascii_case("v3"));

    if modern_ssh || legacy_ssh || ssh_path {
        let segments: Vec<_> = remote.repository().split('/').collect();
        let [version, organization, project, repository] = segments.as_slice() else {
            return Err(invalid_azure_devops_ssh_path());
        };
        if !version.eq_ignore_ascii_case("v3") {
            return Err(invalid_azure_devops_ssh_path());
        }

        let organization = decode_azure_devops_path_segment(organization)?;
        let project = decode_azure_devops_path_segment(project)?;
        let repository = decode_azure_devops_path_segment(repository)?;

        if modern_ssh {
            location.base_url = "https://dev.azure.com".to_string();
        } else if legacy_ssh {
            location.base_url = format!("https://{organization}.visualstudio.com");
        }

        let target_contains_organization = match target {
            Some(target) => azure_devops_target_contains_organization(target, &organization)?,
            None => legacy_ssh,
        };
        location.repository = if target_contains_organization {
            format!("{project}/_git/{repository}")
        } else {
            format!("{organization}/{project}/_git/{repository}")
        };
    }
    Ok(location)
}

fn azure_devops(target: &ForgeTarget, req: &LinkRequest) -> Result<String> {
    let mut url = target.with_path(&req.dir)?;
    let version = match &req.git_ref {
        GitRef::Branch(branch) => format!("GB{branch}"),
        GitRef::Commit(commit) => format!("GC{commit}"),
    };

    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("path", &format!("/{}", req.file))
            .append_pair("version", &version);

        if let Some(lines) = &req.lines {
            query
                .append_pair("line", &lines.start().to_string())
                .append_pair("lineEnd", &(u64::from(lines.end().get()) + 1).to_string())
                .append_pair("lineStartColumn", "1")
                .append_pair("lineEndColumn", "1")
                .append_pair("lineStyle", "plain");
        }

        query.append_pair("_a", "contents");
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests;
