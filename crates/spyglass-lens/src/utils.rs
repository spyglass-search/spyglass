/// Convert a base domain string, e.g. "example.com" into a regex
/// that can be used to match against URLs, e.g. "^(http://|https://)example.com.*"
pub fn regex_for_domain(domain: &str) -> String {
    let mut regex = String::new();
    for ch in domain.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            _ => regex.push_str(&regex::escape(&ch.to_string())),
        }
    }

    format!("^(http://|https://){}.*", regex)
}

pub fn regex_for_prefix(prefix: &str) -> String {
    if prefix.ends_with('$') {
        return format!("^{}", prefix);
    }

    format!("^{}.*", prefix)
}

/// Convert a robots.txt rule into a proper regex string
pub fn regex_for_robots(rule: &str) -> Option<String> {
    if rule.is_empty() {
        return None;
    }

    let wildcard = ".*";
    let mut regex = String::new();
    let mut has_end = false;
    for ch in rule.chars() {
        match ch {
            '*' => regex.push_str(wildcard),
            '^' => {
                // Ignore carets when converting for database
                regex.push('^');
                has_end = true;
            }
            other_ch => {
                regex.push_str(&regex::escape(&other_ch.to_string()));
            }
        }
    }

    if !has_end && !regex.ends_with(wildcard) {
        regex.push_str(wildcard);
    }

    Some(regex)
}
