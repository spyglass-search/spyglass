use crate::models::ResourceRule;
use regex::Regex;

pub fn parse(domain: &str, txt: &str) -> Vec<ResourceRule> {
    let mut rules = Vec::new();

    let mut user_agent: Option<String> = None;
    for line in txt.split('\n') {
        let line = line.to_lowercase().trim().to_string();
        if line.starts_with("user-agent:") {
            let ua = line.strip_prefix("user-agent:").unwrap().trim();
            user_agent = Some(ua.to_string());
        }

        if let Some(user_agent) = &user_agent {
            if user_agent == "*" || user_agent == "carto" {
                if line.starts_with("disallow:") {
                    let regex = line.strip_prefix("disallow:").unwrap().trim();
                    if let Ok(regex) = Regex::new(regex) {
                        rules.push(ResourceRule::new(domain, &regex, false, false));
                    }
                } else if line.starts_with("allow:") {
                    let regex = line.strip_prefix("allow:").unwrap().trim();
                    if let Ok(regex) = Regex::new(regex) {
                        rules.push(ResourceRule::new(domain, &regex, false, true));
                    }
                }
            }
        }
    }

    rules
}
