use scraper::{Html, Selector};

fn html_to_text(doc: &str) {
    let parsed = Html::parse_document(doc);
    let selector = Selector::parse("body").unwrap();

    let root = parsed.select(&selector).next().unwrap();

    for node in root.text() {
        let stripped = node.trim();
        if stripped.is_empty() {
            continue;
        }

        println!("{}", stripped);
    }
}

#[cfg(test)]
mod test {
    use crate::scraper::html_to_text;

    #[test]
    fn test_html_to_text() {
        let html = include_str!("../../fixtures/raw.html");
        html_to_text(html);
    }
}