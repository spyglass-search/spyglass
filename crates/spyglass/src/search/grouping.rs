use std::collections::HashMap;

pub fn group_urls_by_scheme(urls: Vec<&str>) -> HashMap<&str, Vec<&str>> {
    let mut grouping: HashMap<&str, Vec<&str>> = HashMap::new();
    urls.iter().for_each(|url| {
        let part = url.split(':').next();
        if let Some(scheme) = part {
            grouping
                .entry(scheme)
                .and_modify(|list| list.push(url))
                .or_insert_with(|| Vec::from([url.to_owned()]));
        }
    });
    grouping
}
