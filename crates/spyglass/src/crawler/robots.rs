/// Parse robots.txt blobs
/// See the following for more details about robots.txt files:
/// - https://developers.google.com/search/docs/advanced/robots/intro
/// - https://www.robotstxt.org/robotstxt.html
use regex::RegexSet;
use reqwest::StatusCode;
use std::convert::From;
use url::Url;

use entities::models::resource_rule;
use entities::sea_orm::prelude::*;
use entities::sea_orm::{DatabaseConnection, Set};
use shared::regex::{regex_for_robots, WildcardType};

use super::client::HTTPClient;

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

const BOT_AGENT_NAME: &str = "spyglass";

/// Convert a set of rules into a regex set for matching
pub fn filter_set(rules: &[ParsedRule], allow: bool) -> RegexSet {
    let rules: Vec<String> = rules
        .iter()
        .filter(|x| x.allow_crawl == allow)
        .map(|x| x.regex.clone())
        .collect();

    RegexSet::new(rules).expect("Invalid regex rules")
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
                        let regex = regex_for_robots(end.trim(), WildcardType::Regex);
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
                                regex: regex_for_robots("/", WildcardType::Regex)
                                    .expect("Invalid robots regex"),
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

// Checks whether we're allow to crawl this url
pub async fn check_resource_rules(db: &DatabaseConnection, client: &HTTPClient, url: &Url) -> bool {
    let domain = url.host_str().unwrap_or_default();
    let path = url[url::Position::BeforePath..].to_string();

    let rules = resource_rule::Entity::find()
        .filter(resource_rule::Column::Domain.eq(domain))
        .all(db)
        .await
        .expect("Unable to add resource rules");

    if rules.is_empty() && domain != "localhost" {
        log::info!("No rules found for <{}>, fetching robot.txt", domain);
        let mut robots_url = url.clone();
        robots_url.set_path("/robots.txt");

        let res = client.get(&robots_url).await;
        match res {
            Err(err) => log::warn!("Unable to check robots.txt {}", err.to_string()),
            Ok(res) => {
                match res.status() {
                    StatusCode::OK => {
                        if let Ok(body) = res.text().await {
                            let parsed_rules = parse(domain, &body);
                            // No rules? Treat as an allow all
                            if parsed_rules.is_empty() {
                                let new_rule = resource_rule::ActiveModel {
                                    domain: Set(domain.to_owned()),
                                    rule: Set("/".to_owned()),
                                    no_index: Set(false),
                                    allow_crawl: Set(true),
                                    ..Default::default()
                                };
                                let _ = new_rule.insert(db).await;
                            } else {
                                for rule in parsed_rules.iter() {
                                    let new_rule = resource_rule::ActiveModel {
                                        domain: Set(rule.domain.to_owned()),
                                        rule: Set(rule.regex.to_owned()),
                                        no_index: Set(false),
                                        allow_crawl: Set(rule.allow_crawl),
                                        ..Default::default()
                                    };
                                    let _ = new_rule.insert(db).await;
                                }
                            }
                        }
                    }
                    // No robots.txt? Treat as an allow all
                    StatusCode::NOT_FOUND => {
                        let new_rule = resource_rule::ActiveModel {
                            domain: Set(domain.to_owned()),
                            rule: Set("/".to_owned()),
                            no_index: Set(false),
                            allow_crawl: Set(true),
                            ..Default::default()
                        };
                        let _ = new_rule.insert(db).await;
                    }
                    _ => {}
                }
            }
        }
    }

    // Check path against rules, if we find any matches that disallow, skip it
    let rules_into: Vec<ParsedRule> = rules.iter().map(|x| x.to_owned().into()).collect();

    let allow_filter = filter_set(&rules_into, true);
    let disallow_filter = filter_set(&rules_into, false);

    if (allow_filter.is_empty() || !allow_filter.is_match(&path)) && disallow_filter.is_match(&path)
    {
        log::info!("Unable to crawl `{}` due to rule", url.as_str());
        return false;
    }

    // Check the content-type of the URL, only crawl HTML pages for now
    match client.head(url).await {
        Err(err) => {
            log::info!("Unable to check content-type: {}", err.to_string());
            return false;
        }
        Ok(res) => {
            let headers = res.headers();
            if !headers.contains_key(http::header::CONTENT_TYPE) {
                return false;
            } else {
                let value = headers
                    .get(http::header::CONTENT_TYPE)
                    .and_then(|header| header.to_str().ok());

                if let Some(value) = value {
                    if !value.to_string().contains("text/html") {
                        log::info!("Unable to crawl: content-type =/= text/html");
                        return false;
                    }
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod test {
    use super::{check_resource_rules, filter_set, parse, ParsedRule};
    use crate::crawler::Crawler;

    use entities::models::resource_rule;
    use entities::sea_orm::{ActiveModelTrait, Set};
    use entities::test::setup_test_db;
    use regex::Regex;
    use shared::regex::{regex_for_robots, WildcardType};

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
        let regex = regex_for_robots("/*?title=Property:", WildcardType::Regex).unwrap();
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
        assert!(disallow.is_match("/blah?veaction=edit"));
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

    #[tokio::test]
    async fn test_check_resource_rules() {
        let crawler = Crawler::new();
        let db = setup_test_db().await;

        let url = url::Url::parse("https://oldschool.runescape.wiki/").unwrap();
        let domain = url.host_str().unwrap();

        // Add some fake rules
        let allow = resource_rule::ActiveModel {
            domain: Set(domain.to_owned()),
            rule: Set("/".to_string()),
            no_index: Set(false),
            allow_crawl: Set(true),
            ..Default::default()
        };
        allow
            .insert(&db)
            .await
            .expect("Unable to insert allow rule");

        let res = check_resource_rules(&db, &crawler.client, &url).await;

        assert_eq!(res, true);
    }
}
