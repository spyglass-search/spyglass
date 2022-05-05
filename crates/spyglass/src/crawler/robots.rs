use regex::RegexSet;
use entities::models::resource_rule;
/// Parse robots.txt blobs
/// See the following for more details about robots.txt files:
/// - https://developers.google.com/search/docs/advanced/robots/intro
/// - https://www.robotstxt.org/robotstxt.html
use std::convert::From;

#[derive(Clone, Debug)]
pub struct ParsedRule {
    pub domain: String,
    pub regex: String,
    pub allow_crawl: bool,
}

impl From<resource_rule::Model> for ParsedRule {
    fn from(model: resource_rule::Model) -> Self {
        ParsedRule {
            domain: model.domain,
            regex: model.rule,
            allow_crawl: model.allow_crawl,
        }
    }
}

const BOT_AGENT_NAME: &str = "carto";

/// Convert a robots.txt rule into a proper regex string
fn rule_to_regex(rule: &str) -> Option<String> {
    if rule.is_empty() {
        return None;
    }

    let mut regex = String::new();
    let mut has_end = false;
    for ch in rule.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '^' => {
                regex.push('^');
                has_end = true;
            }
            _ => regex.push_str(&regex::escape(&ch.to_string())),
        }
    }

    if !has_end {
        regex.push_str(".*");
    }

    Some(regex)
}

/// Convert a set of rules into a regex set for matching
pub fn filter_set(rules: &[ParsedRule], allow: bool) -> RegexSet {
    let rules: Vec<String> = rules
        .iter()
        .filter(|x| x.allow_crawl == allow)
        .map(|x| x.regex.clone())
        .collect();

    RegexSet::new(&rules).unwrap()
}

/// Parse a robots.txt file and return a vector of parsed rules
pub fn parse(domain: &str, txt: &str) -> Vec<ParsedRule> {
    let mut rules = Vec::new();

    let mut user_agent: Option<String> = None;
    for line in txt.split('\n') {
        let line = line.trim().to_string();
        let split = line.split_once(':');

        if let Some((start, end)) = split {
            if start.to_lowercase().starts_with("user-agent") {
                user_agent = Some(end.trim().to_string());
            }
        }

        // A User-Agent will proceded any rules for that domain
        if let Some(user_agent) = &user_agent {
            if user_agent == "*" || user_agent == BOT_AGENT_NAME {
                if let Some((prefix, end)) = split {
                    let prefix = prefix.to_lowercase();

                    if prefix.starts_with("sitemap") {
                        continue;
                    }

                    if prefix.starts_with("disallow") || prefix.starts_with("allow") {
                        let regex = rule_to_regex(end.trim());
                        if let Some(regex) = regex {
                            rules.push(ParsedRule {
                                domain: domain.to_string(),
                                regex,
                                allow_crawl: prefix.starts_with("allow"),
                            });
                        // Empty disallow is an allow all
                        } else if regex.is_none() && prefix.starts_with("disallow") {
                            rules.push(ParsedRule {
                                domain: domain.to_string(),
                                regex: rule_to_regex("/").unwrap(),
                                allow_crawl: true,
                            });
                        }
                    }
                }
            }
        }
    }

    rules
}

#[cfg(test)]
mod test {
    use crate::crawler::robots::{filter_set, parse, rule_to_regex, ParsedRule};
    use regex::Regex;

    #[test]
    fn test_parse() {
        let robots_txt = include_str!("../../../../fixtures/robots/oldschool_runescape_wiki.txt");
        let matches = parse("oldschool.runescape.wiki", robots_txt);

        assert_eq!(matches.len(), 59);
    }

    #[test]
    fn test_parse_large() {
        let robots_txt = include_str!("../../../../fixtures/robots/reddit_com.txt");
        let matches = parse("www.reddit.com", robots_txt);

        assert_eq!(matches.len(), 37);
    }

    #[test]
    fn test_parse_blanks() {
        let robots_txt = include_str!("../../../../fixtures/robots/crates_io.txt");
        let matches = parse("crates.io", robots_txt);

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_rule_to_regex() {
        let regex = rule_to_regex("/*?title=Property:").unwrap();
        assert_eq!(regex, "/.*\\?title=Property:.*");

        let re = Regex::new(&regex).unwrap();
        assert!(re.is_match("/blah?title=Property:test"));
    }

    #[test]
    fn test_filter_set() {
        let robots_txt = include_str!("../../../../fixtures/robots/oldschool_runescape_wiki.txt");
        let matches = parse("oldschool.runescape.wiki", robots_txt);
        let disallow = filter_set(&matches, false);
        assert!(disallow.is_match("/api.php"));
        assert!(disallow.is_match("/blah?title=Property:test"));
    }

    #[test]
    fn test_filter_set_google() {
        let robots_txt = include_str!("../../../../fixtures/robots/www_google_com.txt");
        let matches = parse("www.google.com", robots_txt);

        let only_search: Vec<ParsedRule> = matches
            .iter()
            .filter(|x| x.regex.starts_with("/search"))
            .cloned()
            .collect();

        let allow = filter_set(&only_search, true);
        let disallow = filter_set(&only_search, false);

        assert!(!allow.is_match("/search?kgmid=/m/0dsbpg6"));
        assert!(disallow.is_match("/search?kgmid=/m/0dsbpg6"));
    }

    #[test]
    fn test_filter_set_factorio() {
        let robots_txt = include_str!("../../../../fixtures/robots/wiki_factorio_com.txt");
        let matches = parse("wiki.factorio.com", robots_txt);

        let allow = filter_set(&matches, true);
        let disallow = filter_set(&matches, false);

        assert_eq!(allow.is_match("/Belt_transport_system"), true);
        assert_eq!(disallow.is_match("/Belt_transport_system"), false);
    }
}
