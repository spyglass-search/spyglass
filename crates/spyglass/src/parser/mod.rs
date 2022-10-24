use std::{
    ffi::OsStr,
    io,
    io::{Error, ErrorKind},
    path::Path,
};

mod docx_parser;

/*
 * Processes the file extension to identify if there is a special
 * parser available
 */
pub fn supports_filetype(extension: &OsStr) -> bool {
    if extension.eq_ignore_ascii_case("docx") {
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
    }
    Err(Error::new(
        ErrorKind::Unsupported,
        format!("Extension {:?} not supported", extension),
    ))
}
