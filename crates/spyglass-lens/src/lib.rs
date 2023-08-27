use std::path::PathBuf;

use blake2::{Blake2s256, Digest};
use serde::{Deserialize, Serialize};

pub mod pipeline;
pub mod types;
mod utils;
use types::{LensFilters, LensRule, LensSource};

pub use crate::pipeline::PipelineConfiguration;
use utils::{regex_for_domain, regex_for_prefix};

/// Contexts are a set of domains/URLs/etc. that restricts a search space to
/// improve results.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LensConfig {
    #[serde(default = "LensConfig::default_author")]
    pub author: String,
    /// Unique identifier for the lens
    pub name: String,
    /// Human readable title for the lens
    #[serde(default)]
    pub label: String,
    /// Optional description of the lens and what it contains.
    pub description: Option<String>,
    /// Whole domains we want to be part of the index.
    pub domains: Vec<String>,
    /// Specific URLs or URL prefixes that will be crawled
    pub urls: Vec<String>,
    /// Semantic version of this lens (will be used to check for updates in the future).
    pub version: String,
    /// Rules to skip/constrain what URLs are indexed
    #[serde(default)]
    pub rules: Vec<LensRule>,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub pipeline: Option<String>,
    #[serde(default)]
    pub lens_source: LensSource,
    /// Category(ies) this lens is in.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Tags to automatically apply to any URLs indexed by this lens
    #[serde(default)]
    pub tags: Vec<(String, String)>,
    // Fields that are used internally & should not be serialized/deserialized
    #[serde(skip)]
    pub file_path: PathBuf,
    #[serde(skip)]
    pub hash: String,
    #[serde(skip, default = "LensConfig::default_is_enabled")]
    pub is_enabled: bool,
}

impl LensConfig {
    fn default_author() -> String {
        "Unknown".to_string()
    }

    fn default_is_enabled() -> bool {
        true
    }

    pub fn label(&self) -> String {
        if self.label.is_empty() {
            self.name.clone()
        } else {
            self.label.clone()
        }
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
                LensRule::SanitizeUrls(_, _) => {}
            }
        }

        LensFilters { allowed, skipped }
    }

    pub fn from_string(contents: &str) -> anyhow::Result<Self> {
        let mut hasher = Blake2s256::new();
        hasher.update(contents);
        let hash_hex = hex::encode(hasher.finalize());

        match ron::from_str::<LensConfig>(contents) {
            Ok(mut lens) => {
                lens.hash = hash_hex;
                Ok(lens)
            }
            Err(e) => Err(anyhow::Error::msg(e.to_string())),
        }
    }

    pub fn from_path(path: PathBuf) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path.clone())?;
        match Self::from_string(&contents) {
            Ok(mut lens) => {
                lens.file_path = path;
                Ok(lens)
            }
            Err(e) => Err(anyhow::Error::msg(e.to_string())),
        }
    }

    pub fn all_tags(&self) -> Vec<(String, String)> {
        let mut tags = Vec::new();

        tags.push(("lens".into(), self.name.clone()));
        for cat in self.categories.iter() {
            tags.push(("category".into(), cat.clone()));
        }
        tags.extend(self.tags.clone());

        tags
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

        assert!(regexes
            .allowed
            .contains(&"^(http://|https://)paulgraham\\.com.*".to_string()));
        assert!(regexes
            .allowed
            .contains(&"^https://oldschool.runescape.wiki/w/.*".to_string()));
    }

    #[test]
    fn test_load_from_file() {
        let lens_str = include_str!("../../../fixtures/lens/extra_fields.ron");
        let config = ron::de::from_str::<LensConfig>(lens_str);
        assert!(config.is_ok());

        let config = config.expect("is err");
        assert_eq!(config.name, "extra_fields");
    }

    #[test]
    fn test_all_tags() {
        let config = LensConfig {
            name: "lens_name".into(),
            categories: vec!["category_one".into(), "category_two".into()],
            ..Default::default()
        };

        let tags = config.all_tags();
        assert_eq!(tags.len(), 3);
    }
}
