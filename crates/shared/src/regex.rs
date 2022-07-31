#[derive(PartialEq)]
pub enum WildcardType {
    Database,
    Regex,
}

pub fn regex_for_domain(domain: &str) -> String {
    let mut regex = String::new();
    for ch in domain.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            _ => regex.push_str(&regex::escape(&ch.to_string())),
        }
    }

    format!("(http://|https://){}.*", regex)
}

pub fn regex_for_prefix(prefix: &str) -> String {
    if prefix.ends_with('$') {
        return prefix.to_string();
    }

    return format!("{}.*", prefix);
}

/// Convert a robots.txt rule into a proper regex string
pub fn regex_for_robots(rule: &str, wildcard_type: WildcardType) -> Option<String> {
    if rule.is_empty() {
        return None;
    }

    let wildcard = match wildcard_type {
        WildcardType::Database => "%",
        WildcardType::Regex => ".*",
    };

    let mut regex = String::new();
    let mut has_end = false;
    for ch in rule.chars() {
        match ch {
            '*' => regex.push_str(wildcard),
            '^' => {
                // Ignore carets when converting for database
                if wildcard_type == WildcardType::Regex {
                    regex.push('^');
                    has_end = true;
                }
            }
            _ => {
                if wildcard_type == WildcardType::Regex {
                    regex.push_str(&regex::escape(&ch.to_string()));
                } else {
                    regex.push(ch);
                }
            }
        }
    }

    if !has_end && !regex.ends_with(wildcard) {
        regex.push_str(wildcard);
    }

    Some(regex)
}

#[cfg(test)]
mod test {
    use super::{regex_for_domain, regex_for_prefix};
    use regex::Regex;

    #[test]
    fn test_regex_for_domain() {
        // Baseline check
        let regex = Regex::new(&regex_for_domain("en.wikipedia.org")).unwrap();
        assert!(regex.is_match("https://en.wikipedia.org/wiki/Rust"));

        // Should match http OR https
        let regex = Regex::new(&regex_for_domain("en.wikipedia.org")).unwrap();
        assert!(regex.is_match("http://en.wikipedia.org/wiki/Rust"));

        // Wildcard should match anything
        let regex = Regex::new(&regex_for_domain("*.wikipedia.org")).unwrap();
        for test in [
            "https://en.wikipedia.org/wiki/Rust",
            "http://sub.sub.wikipedia.org/wiki/blah",
        ] {
            assert!(regex.is_match(test));
        }
    }

    #[test]
    fn test_regex_for_prefix() {
        let prefix = "https://roll20.net/compendium/dnd5e";
        let regex = Regex::new(&regex_for_prefix(prefix)).unwrap();
        // Successes
        for test in [
            "https://roll20.net/compendium/dnd5e",
            "https://roll20.net/compendium/dnd5e/monsters",
            "https://roll20.net/compendium/dnd5e.html",
        ] {
            assert!(regex.is_match(test));
        }

        // Failures
        for test in [
            "https://sub.roll20.net",
            "https://en.wikipedia.org",
            "https://localhost",
        ] {
            assert!(!regex.is_match(test));
        }
    }

    #[test]
    fn test_regex_for_singular_url() {
        let prefix = "https://roll20.net/compendium/dnd5e$";
        let regex = Regex::new(&regex_for_prefix(prefix)).unwrap();
        // Successes
        for test in ["https://roll20.net/compendium/dnd5e"] {
            assert!(regex.is_match(test));
        }

        // Failures
        for test in [
            "https://roll20.net/compendium/dnd5e/monsters",
            "https://roll20.net/compendium/dnd5e.html",
        ] {
            assert!(!regex.is_match(test));
        }
    }
}
