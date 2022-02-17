/// Parse robots.txt blobs
/// See the following for more details about robots.txt files:
/// - https://developers.google.com/search/docs/advanced/robots/intro
/// - https://www.robotstxt.org/robotstxt.html
///
/// TODO: Convert allow/disallow paths into proper regexes.
use regex::Regex;

pub struct ParsedRule {
    pub domain: String,
    pub regex: String,
    pub no_index: bool,
    pub allow_crawl: bool,
}

const BOT_AGENT_NAME: &str = "carto";

pub fn parse(domain: &str, txt: &str) -> Vec<ParsedRule> {
    let mut rules = Vec::new();

    let mut user_agent: Option<String> = None;
    for line in txt.split('\n') {
        let line = line.to_lowercase().trim().to_string();
        if line.starts_with("user-agent:") {
            let ua = line.strip_prefix("user-agent:").unwrap().trim();
            user_agent = Some(ua.to_string());
        }

        // A User-Agent will proceded any rules for that domain
        if let Some(user_agent) = &user_agent {
            if user_agent == "*" || user_agent == BOT_AGENT_NAME {
                if line.starts_with("disallow:") {
                    let regex = line.strip_prefix("disallow:").unwrap().trim();
                    if let Ok(regex) = Regex::new(regex) {
                        rules.push(ParsedRule {
                            domain: domain.to_string(),
                            regex: regex.to_string(),
                            no_index: false,
                            allow_crawl: false,
                        });
                    }
                } else if line.starts_with("allow:") {
                    let regex = line.strip_prefix("allow:").unwrap().trim();
                    if let Ok(regex) = Regex::new(regex) {
                        rules.push(ParsedRule {
                            domain: domain.to_string(),
                            regex: regex.to_string(),
                            no_index: false,
                            allow_crawl: true,
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
    use crate::crawler::robots::parse;

    #[test]
    fn test_parse() {
        let robots_txt = include_str!("../../fixtures/robots.txt");
        let matches = parse("oldschool.runescape.wiki", robots_txt);

        assert_eq!(matches.len(), 59);
    }

    #[test]
    fn test_parse_large() {
        let robots_txt = include_str!("../../fixtures/robots_2.txt");
        let matches = parse("www.reddit.com", robots_txt);

        assert_eq!(matches.len(), 37);
    }

    #[test]
    fn test_parse_blanks() {
        let robots_txt = include_str!("../../fixtures/robots_crates_io.txt");
        let matches = parse("crates.io", robots_txt);

        assert_eq!(matches.len(), 1);
    }
}
