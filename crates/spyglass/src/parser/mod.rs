use anyhow::anyhow;
use std::{ffi::OsStr, path::Path};

pub mod docx_parser;
pub mod pdf_parser;
pub mod xlsx_parser;

/*
 * Parses the specified file
 */
pub fn parse_file(extension: &OsStr, file_path: &Path) -> anyhow::Result<String> {
    if extension.eq_ignore_ascii_case("docx") {
        return docx_parser::parse(file_path);
    } else if extension.eq_ignore_ascii_case("xlsx")
        || extension.eq_ignore_ascii_case("xls")
        || extension.eq_ignore_ascii_case("ods")
    {
        return xlsx_parser::parse(file_path);
    } else if extension.eq_ignore_ascii_case("pdf") {
        return pdf_parser::parse(file_path);
    }

    Err(anyhow!(format!("Extension {extension:?} not supported")))
}
