use std::{ffi::OsStr, path::Path};

use anyhow::anyhow;

pub mod docx_parser;
pub mod xlsx_parser;

/*
 * Processes the file extension to identify if there is a special
 * parser available
 */
pub fn supports_filetype(extension: &OsStr) -> bool {
    log::debug!("Extension {:?}", extension);
    if extension.eq_ignore_ascii_case("docx")
        || extension.eq_ignore_ascii_case("xlsx")
        || extension.eq_ignore_ascii_case("xls")
        || extension.eq_ignore_ascii_case("ods")
    {
        return true;
    }
    false
}

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
    }
    Err(anyhow!(format!("Extension {extension:?} not supported")))
}
