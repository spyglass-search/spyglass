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

pub struct LensFilters {
    pub allowed: Vec<String>,
    pub skipped: Vec<String>,
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

    pub fn into_regexes(&self) -> LensFilters {
        let mut allowed = Vec::new();
        let mut skipped = Vec::new();

        for domain in &self.domains {
            allowed.push(regex_for_domain(domain));
        }

        for prefix in &self.urls {
            allowed.push(regex_for_prefix(prefix));
        }

        for rule in &self.rules {
            match rule {
                LensRule::LimitURLDepth { .. } => allowed.push(rule.to_regex()),
                LensRule::SkipURL(_) => skipped.push(rule.to_regex()),
            }
        }

        LensFilters { allowed, skipped }
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
            urls: vec!["https://oldschool.runescape.wiki/w/".to_string()],
            ..Default::default()
        };

        let regexes = config.into_regexes();
        assert_eq!(regexes.allowed.len(), 2);
        assert_eq!(regexes.skipped.len(), 0);

        dbg!(&regexes.allowed);

        assert!(regexes
            .allowed
            .contains(&"^(http://|https://)paulgraham\\.com.*".to_string()));
        assert!(regexes
            .allowed
            .contains(&"^https://oldschool.runescape.wiki/w/.*".to_string()));
    }
}
