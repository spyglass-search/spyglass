use anyhow::anyhow;
use bytes::Bytes;
use calamine::{open_workbook_auto, open_workbook_auto_from_rs, DataType, Reader, Sheets};
use std::{
    io::{Cursor, Read, Seek},
    path::Path,
};

fn process_file<R>(workbook: &mut Sheets<R>) -> anyhow::Result<String>
where
    R: Read + Seek,
{
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
    Ok(str)
}

pub fn parse_bytes(b: Bytes) -> anyhow::Result<String> {
    let c = Cursor::new(b);
    match open_workbook_auto_from_rs(c) {
        Ok(mut workbook) => process_file(&mut workbook),
        Err(err) => {
            log::error!("Error opening file. Error: {err:?}");
            Err(anyhow!(err.to_string()))
        }
    }
}
/**
 * Uses calamine to parse spreadsheet files. Takes all cell contents and combines
 * them together into a single string to send for indexing.
 */
pub fn parse(file_path: &Path) -> anyhow::Result<String> {
    match open_workbook_auto(file_path) {
        Ok(mut workbook) => process_file(&mut workbook),
        Err(err) => {
            log::error!("Error opening file {:?}. Error: {:?}", file_path, err);
            Err(anyhow!(err.to_string()))
        }
    }
}
