use serde::{Deserialize, Serialize};
use std::path::PathBuf;
mod utils;
use utils::{regex_for_domain, regex_for_prefix, regex_for_robots};

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
}

impl LensRule {
    pub fn to_regex(&self) -> String {
        match &self {
            LensRule::LimitURLDepth(prefix, max_depth) => {
                let prefix = prefix.trim_end_matches('/');
                let regex = format!("^{}/?(/[^/]+/?){{0, {}}}$", prefix, max_depth);
                regex
            }
            LensRule::SkipURL(rule_str) => {
                regex_for_robots(rule_str).expect("Invalid SkipURL regex")
            }
        }
    }
}

/// Contexts are a set of domains/URLs/etc. that restricts a search space to
/// improve results.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LensConfig {
    #[serde(default = "LensConfig::default_author")]
    pub author: String,
    pub name: String,
    pub description: Option<String>,
    pub domains: Vec<String>,
    pub urls: Vec<String>,
    pub version: String,
    #[serde(default = "LensConfig::default_is_enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub rules: Vec<LensRule>,
    #[serde(default)]
    pub trigger: String,
}

impl LensConfig {
    fn default_author() -> String {
        "Unknown".to_string()
    }

    fn default_is_enabled() -> bool {
        true
    }

    pub fn into_regexes(&self) -> Vec<String> {
        let mut filters: Vec<String> = Vec::new();
        for domain in &self.domains {
            filters.push(regex_for_domain(domain));
        }

        for prefix in &self.urls {
            filters.push(regex_for_prefix(prefix));
        }

        for rule in &self.rules {
            filters.push(rule.to_regex());
        }

        filters
    }

    pub fn from_path(path: PathBuf) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        match ron::from_str::<LensConfig>(&contents) {
            Ok(lens) => Ok(lens),
            Err(e) => Err(anyhow::Error::msg(e.to_string())),
        }
    }
}

#[cfg(test)]
mod test {
    use super::LensConfig;

    #[test]
    fn test_into_regexes() {
        let config = LensConfig {
            domains: vec!["paulgraham.com".to_string()],
            urls: vec!["https://oldschool.runescape.wiki/wiki/".to_string()],
            ..Default::default()
        };

        let regexes = config.into_regexes();
        dbg!(&regexes);
        assert_eq!(regexes.len(), 2);
        // Should contain domain regex
        assert!(regexes.contains(&"^(http://|https://)paulgraham\\.com/.*".to_string()));
        // Should contain url prefix regex
        assert!(regexes.contains(&"^https://oldschool.runescape.wiki/wiki/.*".to_string()));
    }
}
