/// Parse robots.txt blobs
/// See the following for more details about robots.txt files:
/// - https://developers.google.com/search/docs/advanced/robots/intro
/// - https://www.robotstxt.org/robotstxt.html
use regex::RegexSet;

pub struct ParsedRule {
    pub domain: String,
    pub regex: String,
    pub no_index: bool,
    pub allow_crawl: bool,
}

const BOT_AGENT_NAME: &str = "carto";

/// Convert a robots.txt rule into a proper regex string
fn rule_to_regex(rule: &str) -> String {
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

    regex
}

/// Convert a set of rules into a regex set for matching
pub fn filter_set(rules: &[ParsedRule]) -> RegexSet {
    let disallow: Vec<String> = rules
        .iter()
        .filter(|x| !x.allow_crawl)
        .map(|x| x.regex.clone())
        .collect();

    RegexSet::new(&disallow).unwrap()
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
                        rules.push(ParsedRule {
                            domain: domain.to_string(),
                            regex: rule_to_regex(end.trim()),
                            no_index: false,
                            allow_crawl: prefix.starts_with("allow"),
                        });
                    }
                }
            }
        }
    }

    rules
}

#[cfg(test)]
mod test {
    use crate::crawler::robots::{filter_set, parse, rule_to_regex};
    use regex::Regex;

    #[test]
    fn test_parse() {
        let robots_txt = include_str!("../../../../fixtures/robots.txt");
        let matches = parse("oldschool.runescape.wiki", robots_txt);

        assert_eq!(matches.len(), 59);
    }

    #[test]
    fn test_parse_large() {
        let robots_txt = include_str!("../../../../fixtures/robots_2.txt");
        let matches = parse("www.reddit.com", robots_txt);

        assert_eq!(matches.len(), 37);
    }

    #[test]
    fn test_parse_blanks() {
        let robots_txt = include_str!("../../../../fixtures/robots_crates_io.txt");
        let matches = parse("crates.io", robots_txt);

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_rule_to_regex() {
        let regex = rule_to_regex("/*?title=Property:");
        assert_eq!(regex, "/.*\\?title=Property:.*");

        let re = Regex::new(&regex).unwrap();
        assert!(re.is_match("/blah?title=Property:test"));
    }

    #[test]
    fn test_filter_set() {
        let robots_txt = include_str!("../../../../fixtures/robots.txt");
        let matches = parse("oldschool.runescape.wiki", robots_txt);
        let filter_set = filter_set(&matches);
        assert!(filter_set.is_match("/api.php"));
        assert!(filter_set.is_match("/blah?title=Property:test"));
    }
}
