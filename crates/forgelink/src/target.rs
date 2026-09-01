use iri_string::percent_encode::PercentEncodedForUri;
use url::{PathSegmentsMut, Url};

use crate::{Error, Forge, LinkRequest, Result};

fn path_segments_mut(url: &mut Url) -> Result<PathSegmentsMut<'_>> {
    url.path_segments_mut()
        .map_err(|()| Error::InvalidBaseUrl("cannot be used as a base".to_string()))
}

/// A validated web destination and URL format for a forge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeTarget {
    base_url: Url,
    forge: Forge,
}

impl ForgeTarget {
    /// Creates a forge target from an HTTP(S) base URL.
    ///
    /// The URL may contain a port and path prefix, but not credentials, a query,
    /// or a fragment.
    ///
    /// # Errors
    ///
    /// Fails if `base_url` is not a supported absolute URL.
    pub fn new(base_url: &str, forge: Forge) -> Result<Self> {
        let mut base_url =
            Url::parse(base_url).map_err(|error| Error::InvalidBaseUrl(error.to_string()))?;

        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(Error::InvalidBaseUrl(
                "scheme must be http or https".to_string(),
            ));
        }
        if base_url.host().is_none() {
            return Err(Error::InvalidBaseUrl("missing host".to_string()));
        }
        if !base_url.username().is_empty() || base_url.password().is_some() {
            return Err(Error::InvalidBaseUrl(
                "credentials are not supported".to_string(),
            ));
        }
        if base_url.query().is_some() {
            return Err(Error::InvalidBaseUrl(
                "query strings are not supported".to_string(),
            ));
        }
        if base_url.fragment().is_some() {
            return Err(Error::InvalidBaseUrl(
                "fragments are not supported".to_string(),
            ));
        }

        path_segments_mut(&mut base_url)?.pop_if_empty();

        Ok(Self { base_url, forge })
    }

    /// Returns the normalized base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    /// Returns the forge URL format.
    #[must_use]
    pub fn forge(&self) -> Forge {
        self.forge
    }

    /// Builds a URL for a file and optional line range.
    ///
    /// # Errors
    ///
    /// Returns an error if the target URL does not support hierarchical path segments.
    pub fn file_url(&self, req: &LinkRequest) -> Result<String> {
        self.forge.file_url(self, req)
    }

    /// Builds a URL for a repository project page.
    ///
    /// # Errors
    ///
    /// Returns an error if the target URL does not support hierarchical path segments.
    pub fn project_url(&self, dir: &str) -> Result<String> {
        self.with_path(dir).map(Into::into)
    }

    pub(crate) fn with_path(&self, path: &str) -> Result<Url> {
        let mut url = self.base_url.clone();
        path_segments_mut(&mut url)?.pop_if_empty();

        let path = format!(
            "{}/{}",
            url.path().trim_end_matches('/'),
            PercentEncodedForUri::from_path(path)
        );
        url.set_path(&path);

        Ok(url)
    }
}

#[cfg(test)]
mod tests;
