use anyhow::anyhow;
use bytes::Bytes;
use docx::{
    document::ParagraphContent,
    document::Run,
    document::RunContent,
    document::{BodyContent, TableCellContent},
    DocxFile,
};
use std::{borrow::Borrow, io::Cursor, path::Path};

fn process_file(docx: &DocxFile) -> anyhow::Result<String> {
    let result = match docx.parse() {
        Ok(res) => res,
        Err(err) => return Err(anyhow!(format!("Error parsing file: {err:?}"))),
    };

    let mut text: Vec<String> = Vec::new();
    result
        .document
        .body
        .content
        .iter()
        .for_each(|body_content| match body_content {
            BodyContent::Paragraph(paragraph) => {
                let mut str = String::from("");
                paragraph.content.iter().for_each(|paragraph_content| {
                    str.push_str(process_paragraph_content(paragraph_content).as_str())
                });
                text.push(str);
            }
            BodyContent::Table(table) => {
                table
                    .rows
                    .iter()
                    .flat_map(|row| row.cells.iter())
                    .flat_map(|cell| cell.content.iter())
                    .for_each(|table_cell| {
                        let mut str = String::from("");
                        match table_cell {
                            TableCellContent::Paragraph(p) => {
                                p.content.iter().for_each(|pc| {
                                    str.push_str(process_paragraph_content(pc).as_str())
                                });
                            }
                        }

                        text.push(str);
                    });
            }
        });

    let output = text.join(" ");
    log::trace!("Document: {:?}", output);
    Ok(output)
}

pub fn parse_bytes(b: Bytes) -> anyhow::Result<String> {
    let c = Cursor::new(b);
    match DocxFile::from_reader(c) {
        Ok(docx) => process_file(&docx),
        Err(err) => {
            log::error!("Error processing docx file: {err:?}");
            Err(anyhow!(format!("Error reading Docx file {err:?}")))
        }
    }
}
/*
 * Reads the provided file as a DOCX, pulls out all paragraph text
 * and returns it as a String
 */
pub fn parse(file_path: &Path) -> anyhow::Result<String> {
    match DocxFile::from_file(file_path) {
        Ok(docx) => process_file(&docx),
        Err(error) => {
            log::error!(
                "Error processing docx file {:?}, Error: {:?} ",
                file_path,
                error
            );
            Err(anyhow!(format!(
                "Error reading Docx file {file_path:?} - {error:?}"
            )))
        }
    }
}

/*
 * Helper method used to process paragraph content.
 */
fn process_paragraph_content(pc: &ParagraphContent) -> String {
    let mut str = String::from("");
    match pc {
        ParagraphContent::Run(r) => {
            str.push_str(process_run(r).as_str());
        }
        ParagraphContent::Link(link) => {
            str.push_str(process_run(&link.content).as_str());
        }
        _ => {}
    }
    str
}

/**
 * Helper method used to process a run.
 */
fn process_run(run: &Run) -> String {
    let mut str = String::from("");
    run.content.iter().for_each(|rc| {
        if let RunContent::Text(t) = rc {
            str.push_str(t.text.borrow());
            str.push(' ');
        }
    });
    str
}
