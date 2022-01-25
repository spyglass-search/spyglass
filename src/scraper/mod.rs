mod element;
mod html;

use crate::scraper::html::Html;

fn html_to_text(doc: &str) {
    let parsed = Html::parse(doc);
}

/// # fn main() {
/// # let document = "";
/// use html5ever::driver::{self, ParseOpts};
/// use scraper::Html;
/// use tendril::TendrilSink;
///
/// let parser = driver::parse_document(Html::new_document(), ParseOpts::default());
/// let html = parser.one(document);
/// # }
#[cfg(test)]
mod test {
    use crate::scraper::html_to_text;

    #[test]
    fn test_html_to_text() {
        let html = include_str!("../../fixtures/raw.html");
        html_to_text(html);
    }
}
