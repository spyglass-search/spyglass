use bytes::Bytes;
use pdf::file::FileOptions;
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::Path,
    process,
};

#[cfg(all(target_os = "windows", not(debug_assertions)))]
const EXE_NAME: &str = "./pdftotext.exe";

#[cfg(all(not(target_os = "windows"), not(debug_assertions)))]
const EXE_NAME: &str = "./pdftotext";

#[cfg(all(target_os = "windows", debug_assertions))]
const EXE_NAME: &str = "../../utils/win/pdftotext.exe";

#[cfg(all(target_os = "macos", debug_assertions))]
const EXE_NAME: &str = "../../utils/mac/pdftotext";

#[cfg(all(target_os = "linux", debug_assertions))]
const EXE_NAME: &str = "../../utils/linux/pdftotext";

// Uses utility pdftotxt. The utility documentation is as follows
//
// pdftotext version 4.04 [www.xpdfreader.com]
// Copyright 1996-2022 Glyph & Cog, LLC
// Usage: pdftotext [options] <PDF-file> [<text-file>]
//   -f <int>               : first page to convert
//   -l <int>               : last page to convert
//   -layout                : maintain original physical layout
//   -simple                : simple one-column page layout
//   -simple2               : simple one-column page layout, version 2
//   -table                 : similar to -layout, but optimized for tables
//   -lineprinter           : use strict fixed-pitch/height layout
//   -raw                   : keep strings in content stream order
//   -fixed <number>        : assume fixed-pitch (or tabular) text
//   -linespacing <number>  : fixed line spacing for LinePrinter mode
//   -clip                  : separate clipped text
//   -nodiag                : discard diagonal text
//   -enc <string>          : output text encoding name
//   -eol <string>          : output end-of-line convention (unix, dos, or mac)
//   -nopgbrk               : don't insert a page break at the end of each page
//   -bom                   : insert a Unicode BOM at the start of the text file
//   -marginl <number>      : left page margin
//   -marginr <number>      : right page margin
//   -margint <number>      : top page margin
//   -marginb <number>      : bottom page margin
//   -opw <string>          : owner password (for encrypted files)
//   -upw <string>          : user password (for encrypted files)
//   -verbose               : print per-page status information
//   -q                     : don't print any messages or errors
//   -cfg <string>          : configuration file to use in place of .xpdfrc
//   -listencodings         : list all available output text encodings
//   -v                     : print copyright and version info
//   -h                     : print usage information
//   -help                  : print usage information
//   --help                 : print usage information
//   -?                     : print usage information
pub fn parse(path: &Path) -> anyhow::Result<Pdf> {
    let uuid = uuid::Uuid::new_v4().as_hyphenated().to_string();

    let current_dir = match env::current_exe() {
        Ok(current_exe) => current_exe.parent().map(|path| path.to_owned()),
        Err(err) => {
            log::error!("Unable to access current exe {:?}", err);
            None
        }
    };

    let temp_dir = env::temp_dir();
    let txt_path = temp_dir.join(format!("{uuid}-spyglass-processing.txt"));

    let exe_path = if let Some(path) = &current_dir {
        path.join(EXE_NAME)
    } else {
        Path::new(EXE_NAME).to_path_buf()
    };

    log::debug!("Executable Path {:?}", exe_path);

    let mut cmd = process::Command::new(exe_path.to_str().unwrap());
    cmd.arg("-layout")
        .arg("-q")
        .arg("-nopgbrk")
        .arg(path.to_str().unwrap())
        .arg(txt_path.to_str().unwrap());

    if let Some(path) = current_dir {
        cmd.current_dir(path);
    }

    log::debug!("Full Command {:?}", cmd);
    let cmd = cmd.spawn();

    match cmd {
        Ok(mut child) => {
            if let Err(err) = child.wait() {
                let _ = fs::remove_file(txt_path);
                return Err(anyhow::format_err!(err));
            }
        }
        Err(err) => {
            let _ = fs::remove_file(txt_path);
            return Err(anyhow::format_err!(err));
        }
    }

    let content = match std::fs::read(txt_path.as_path()) {
        Ok(content) => {
            let pdf_contents = String::from_utf8_lossy(&content);
            let _ = fs::remove_file(txt_path);
            String::from(pdf_contents.as_ref())
        }
        Err(err) => {
            let _ = fs::remove_file(txt_path);
            return Err(anyhow::format_err!(err));
        }
    };

    let metadata = PdfMetadata::parse(path);

    Ok(Pdf { content, metadata })
}

pub fn parse_bytes(b: Bytes) -> anyhow::Result<Pdf> {
    let uuid = uuid::Uuid::new_v4().as_hyphenated().to_string();
    let temp_dir = env::temp_dir();
    let temp_doc = temp_dir.join(format!("{uuid}.pdf"));

    {
        let mut file = File::create(temp_doc.as_path())?;
        file.write_all(&b)?;
    }

    let result = parse(temp_doc.as_path());
    let _ = fs::remove_file(temp_doc);
    result
}

pub struct Pdf {
    pub content: String,
    pub metadata: PdfMetadata,
}

#[derive(Default)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
}

impl PdfMetadata {
    pub fn parse(path: &Path) -> Self {
        let pdf = match FileOptions::cached().open(path) {
            Ok(pdf) => pdf,
            Err(_) => return Default::default(),
        };
        let pdf_info = match &pdf.trailer.info_dict {
            Some(dict) => dict,
            None => return Default::default(),
        };
        Self {
            title: pdf_info.get("Title").and_then(|v| v.to_string().ok()),
            author: pdf_info.get("Author").and_then(|v| v.to_string().ok()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn test_pdf_metadata_extraction_from_pdf_with_metadata() {
        let path_with_metadata = Path::new("../../fixtures/pdf/pdf_with_metadata.pdf");
        let metadata = super::PdfMetadata::parse(&path_with_metadata);
        assert_eq!(metadata.title, Some("PDF title".to_string()));
        assert_eq!(metadata.author, Some("PDF author".to_string()));
    }

    #[test]
    fn test_pdf_metadata_extraction_from_pdf_with_missing_metadata() {
        let path_with_metadata = Path::new("../../fixtures/pdf/pdf_without_metadata.pdf");
        let metadata = super::PdfMetadata::parse(&path_with_metadata);
        assert_eq!(metadata.title, None);
        assert_eq!(metadata.author, None);
    }
}
