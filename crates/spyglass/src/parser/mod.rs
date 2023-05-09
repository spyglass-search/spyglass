use anyhow::anyhow;
use std::{ffi::OsStr, path::Path};

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
