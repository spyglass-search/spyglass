use docx::{
    document::ParagraphContent,
    document::Run,
    document::RunContent,
    document::{BodyContent, TableCellContent},
    DocxError, DocxFile,
};
use std::{
    borrow::Borrow,
    io,
    io::{Error, ErrorKind},
    path::Path,
};

/*
 * Reads the provided file as a DOCX, pulls out all paragraph text
 * and returns it as a String
 */
pub fn parse(file_path: &Path) -> io::Result<String> {
    let docx = DocxFile::from_file(file_path);
    match docx {
        Ok(docx) => {
            let result = docx.parse();
            match result {
                Ok(docx) => {
                    let mut text: Vec<String> = Vec::new();
                    docx.document
                        .body
                        .content
                        .iter()
                        .for_each(|body_content| match body_content {
                            BodyContent::Paragraph(paragraph) => {
                                let mut str = String::from("");
                                paragraph.content.iter().for_each(|paragraph_content| {
                                    str.push_str(
                                        process_paragraph_content(paragraph_content).as_str(),
                                    )
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
                                                    str.push_str(
                                                        process_paragraph_content(pc).as_str(),
                                                    )
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
                Err(DocxError::Xml(xml_err)) => {
                    log::warn!("Error reading Docx file {:?}", xml_err);
                    Err(Error::new(ErrorKind::InvalidData, xml_err))
                }
                Err(doc_err) => {
                    log::warn!("Error reading Docx file {:?}", doc_err);
                    Err(Error::new(
                        ErrorKind::InvalidData,
                        format!("Error reading Docx file {doc_err:?}"),
                    ))
                }
            }
        }
        Err(error) => {
            log::error!(
                "Error processing docx file {:?}, Error: {:?} ",
                file_path,
                error
            );
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Error reading docx file {error:?}"),
            ))
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
