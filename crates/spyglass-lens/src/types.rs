use serde::{Deserialize, Serialize};
use std::fmt;

use crate::utils::regex_for_robots;

pub struct LensFilters {
    pub allowed: Vec<String>,
    pub skipped: Vec<String>,
}

/// Different rules that filter out the URLs that would be crawled for a lens
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LensRule {
    /// Limits the depth of a URL to a certain depth.
    /// For example:
    ///  - LimitURLDepth("https://example.com/", 1) will limit it to https://example.com/<path 1>
    ///  - LimitURLDepth("https://example.com/", 2) will limit it to https://example.com/<path 1>/<path 2>
    ///  - etc.
    LimitURLDepth(String, u8),
    /// Skips are applied when bootstrapping & crawling
    SkipURL(String),
    /// Modifies the url to walk, applied when bootstrapping & crawling   
    SanitizeUrls(String, UrlSanitizeConfig),
}

/// Defines Url Sanitization Configuration. This configuration allows urls to be modified to
/// produce the correct url for crawling.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UrlSanitizeConfig {
    // Removes query parameters from the url
    pub remove_query_parameter: bool,
}

impl fmt::Display for UrlSanitizeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UrlSanitizeConfig {{ remove_query_parameter: {} }}",
            self.remove_query_parameter
        )
    }
}

impl fmt::Display for LensRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitURLDepth(url, depth) => write!(f, "LimitURLDepth(\"{url}\", {depth})"),
            Self::SkipURL(url) => write!(f, "SkipURL(\"{url}\")",),
            Self::SanitizeUrls(url, config) => write!(f, "SanitizeUrls(\"{url}\", {config}"),
        }
    }
}

impl LensRule {
    pub fn to_regex(&self) -> String {
        match &self {
            LensRule::LimitURLDepth(prefix, max_depth) => {
                let prefix = prefix.trim_end_matches('/');
                let regex = format!("^{prefix}/?(/[^/]+/?){{0, {max_depth}}}$");
                regex
            }
            LensRule::SkipURL(rule_str) => {
                regex_for_robots(rule_str).expect("Invalid SkipURL regex")
            }
            LensRule::SanitizeUrls(rule_str, _) => {
                regex_for_robots(rule_str).expect("Invalid SanitizeUrls regex")
            }
        }
    }
}

/// The lens source is used to identify if the lens was provided by a remote
/// lens provider or if it is a locally created lens. Depending on the
/// provider different features might be available
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub enum LensSource {
    /**
     * Lens sourced locally
     */
    #[default]
    Local,
    /**
     * Lens download from a remote source
     */
    Remote(String),
}

#[cfg(test)]
mod test {
    use super::LensRule;
    use super::UrlSanitizeConfig;

    #[test]
    fn test_rules_display() {
        let rule = LensRule::SkipURL("http://example.com".to_string());
        assert_eq!(rule.to_string(), "SkipURL(\"http://example.com\")");

        let rule = LensRule::LimitURLDepth("http://example.com".to_string(), 2);
        assert_eq!(rule.to_string(), "LimitURLDepth(\"http://example.com\", 2)");

        let rule = LensRule::SanitizeUrls(
            "www.hello.com".to_string(),
            UrlSanitizeConfig {
                remove_query_parameter: true,
            },
        );
        assert_eq!(
            rule.to_string(),
            "SanitizeUrls(\"www.hello.com\", UrlSanitizeConfig { remove_query_parameter: true }"
        );
    }
}
