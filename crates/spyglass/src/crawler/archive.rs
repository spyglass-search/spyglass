use libnetrunner::parser::ParseResult;
use std::path::Path;
use warc::{WarcHeader, WarcReader};

// Warc Record object
pub struct ArchiveRecord {
    pub status: u16,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub content: String,
}

/// Reads a WARC file from the provided path and provides a streaming record
/// iterator
pub fn read_warc(path: &Path) -> anyhow::Result<impl Iterator<Item = Option<ArchiveRecord>>> {
    let reader = WarcReader::from_path(path)?;
    let record_itr = reader.iter_records().map(move |record_rslt| {
        if let Ok(record) = record_rslt {
            let url = record
                .header(WarcHeader::TargetURI)
                .expect("TargetURI not set")
                .to_string();

            if let Ok(body) = String::from_utf8(record.body().into()) {
                let (headers, content) = parse_body(&body);
                return Option::Some(ArchiveRecord {
                    status: 200u16,
                    url,
                    headers,
                    content,
                });
            }
        }
        Option::None
    });

    return Ok(record_itr);
}

// Reads the parsed cache file and provides the contents as an iterator
pub fn read_parsed(path: &Path) -> anyhow::Result<impl Iterator<Item = ParseResult>> {
    ParseResult::iter_from_gz(path)
}

// Helper used to parse the body string into headers and content
fn parse_body(body: &str) -> (Vec<(String, String)>, String) {
    let mut headers = Vec::new();
    let mut content = String::new();

    let mut headers_finished = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            headers_finished = true;
        } else {
            match headers_finished {
                true => content.push_str(trimmed),
                false => {
                    if let Some((key, value)) = trimmed.split_once(':') {
                        headers.push((key.trim().to_string(), value.trim().to_string()));
                    }
                }
            }
        }
    }

    (headers, content)
}
