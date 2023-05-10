use anyhow::anyhow;
use bytes::Bytes;
use std::{ffi::OsStr, path::Path};

use crate::utils;

pub mod docx_parser;
pub mod pdf_parser;
pub mod xlsx_parser;

/*
 * Parses the specified file
 */
pub fn parse_file(extension: &OsStr, file_path: &Path) -> anyhow::Result<ParsedDocument> {
    if extension.eq_ignore_ascii_case("docx") {
        Ok(ParsedDocument {
            content: docx_parser::parse(file_path)?,
            ..Default::default()
        })
    } else if extension.eq_ignore_ascii_case("xlsx")
        || extension.eq_ignore_ascii_case("xls")
        || extension.eq_ignore_ascii_case("ods")
    {
        Ok(ParsedDocument {
            content: xlsx_parser::parse(file_path)?,
            ..Default::default()
        })
    } else if extension.eq_ignore_ascii_case("pdf") {
        Ok(pdf_parser::parse(file_path)?.into())
    } else {
        Err(anyhow!(format!("Extension {extension:?} not supported")))
    }
}

pub fn parse_content(mime_type_str: &str, content: &Bytes) -> anyhow::Result<ParsedDocument> {
    let supported_mime = utils::mime::SupportedMime::from_mime(mime_type_str);

    match supported_mime {
        utils::mime::SupportedMime::Audio(_mime) => Err(anyhow!(format!(
            "Audio Mimetype {mime_type_str:?} not supported"
        ))),
        utils::mime::SupportedMime::Code(_mime) => Ok(ParsedDocument {
            content: std::str::from_utf8(content)?.to_string(),
            ..Default::default()
        }),
        utils::mime::SupportedMime::Document(mime) => {
            // match mime_type.
            match mime.to_string().as_str() {
                utils::mime::DOCX => Ok(ParsedDocument {
                    content: docx_parser::parse_bytes(content.clone())?,
                    ..Default::default()
                }),
                utils::mime::XLSX | utils::mime::XLS | utils::mime::ODS | utils::mime::GSHEET => {
                    Ok(ParsedDocument {
                        content: xlsx_parser::parse_bytes(content.clone())?,
                        ..Default::default()
                    })
                }
                _ => Err(anyhow!(format!(
                    "Document Mimetype {mime_type_str:?} not supported"
                ))),
            }
        }
        utils::mime::SupportedMime::Text(_mime) => Ok(ParsedDocument {
            content: std::str::from_utf8(content)?.to_string(),
            ..Default::default()
        }),
        utils::mime::SupportedMime::NotSupported => {
            Err(anyhow!(format!("Mimetype {mime_type_str:?} not supported")))
        }
    }
}

#[derive(Default)]
pub struct ParsedDocument {
    pub title: Option<String>,
    pub author: Option<String>,
    pub content: String,
}

impl From<pdf_parser::Pdf> for ParsedDocument {
    fn from(value: pdf_parser::Pdf) -> Self {
        Self {
            title: value.metadata.title,
            author: value.metadata.author,
            content: value.content,
        }
    }
}
