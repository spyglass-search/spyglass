use std::{
    ffi::OsStr,
    io,
    io::{Error, ErrorKind},
    path::Path,
};

mod docx_parser;
mod xlsx_parser;

/*
 * Processes the file extension to identify if there is a special
 * parser available
 */
pub fn supports_filetype(extension: &OsStr) -> bool {
    print!("Extension {:?}", extension);
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
pub fn parse_file(extension: &OsStr, file_path: &Path) -> io::Result<String> {
    if extension.eq_ignore_ascii_case("docx") {
        return docx_parser::parse(file_path);
    } else if extension.eq_ignore_ascii_case("xlsx")
        || extension.eq_ignore_ascii_case("xls")
        || extension.eq_ignore_ascii_case("ods")
    {
        return xlsx_parser::parse(file_path);
    }
    Err(Error::new(
        ErrorKind::Unsupported,
        format!("Extension {:?} not supported", extension),
    ))
}
