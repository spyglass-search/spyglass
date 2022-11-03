use calamine::{open_workbook_auto, DataType, Reader};
use std::{
    io,
    io::{Error, ErrorKind},
    path::Path,
};

/**
 * Uses calamine to parse spreadsheet files. Takes all cell contents and combines
 * them together into a single string to send for indexing.
 */
pub fn parse(file_path: &Path) -> io::Result<String> {
    let workbook_r = open_workbook_auto(file_path);
    match workbook_r {
        Ok(mut workbook) => {
            let sheets = workbook.sheet_names().to_owned();
            let mut str = String::new();
            for s in sheets {
                if let Some(Ok(r)) = workbook.worksheet_range(&s) {
                    for row in r.rows() {
                        for col in row {
                            match col {
                                DataType::Int(val) => str.push_str(val.to_string().as_str()),
                                DataType::Float(val) => str.push_str(val.to_string().as_str()),
                                DataType::String(val) => str.push_str(val.as_str()),
                                DataType::Bool(val) => str.push_str(val.to_string().as_str()),
                                DataType::DateTime(val) => str.push_str(val.to_string().as_str()),
                                DataType::Error(error) => log::debug!("Cell Error {:?}", error),
                                DataType::Empty => {}
                            }
                            str.push(' ');
                        }
                    }
                }
            }
            log::debug!("Document {:?} text: {:?}", file_path, str);
            Ok(str)
        }
        Err(error) => {
            log::error!("Error opening file {:?}. Error: {:?}", file_path, error);
            Err(Error::new(ErrorKind::InvalidData, format!("{:?}", error)))
        }
    }
}
