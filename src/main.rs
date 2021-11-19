use rusqlite::{params, Connection, OpenFlags, Result};

const HISTORY_DB: &str = "_data/places.sqlite";

#[derive(Debug)]
struct Place {
    id: i32,
    url: String
}


fn main() -> Result<()> {
    let conn = Connection::open_with_flags(&HISTORY_DB, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    println!("Connected to db...");

    let mut stmt = conn.prepare("SELECT id, url FROM moz_places LIMIT 10")?;
    let place_iter = stmt.query_map(params![], |row| {
        Ok(Place {
            id: row.get(0)?,
            url: row.get(1)?
        })
    })?;

    for place in place_iter {
        println!("Found place {:?}", place.unwrap());
    }

    Ok(())
}
