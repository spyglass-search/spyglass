use directories::ProjectDirs;
use dirs::home_dir;
use rusqlite::{params, Connection, OpenFlags, Result};
use std::{env, fs, io, path::PathBuf};

mod carto;
use crate::carto::models::Place;

/// Get the default profile path for Firefox
fn default_profile_path() -> Result<PathBuf, &'static str> {
    let home = home_dir().expect("No home directory detected");
    match env::consts::OS {
        // "linux" => {},
        "macos" => Ok(home.join("Library/Application Support/Firefox/Profiles")),
        // "windows" => {},
        _ => Err("Platform not supported"),
    }
}

fn detect_profiles() -> Vec<PathBuf> {
    let mut path_results = Vec::new();
    if let Ok(path) = default_profile_path() {
        for path in fs::read_dir(path).unwrap() {
            if let Ok(path) = path {
                if path.path().is_dir() {
                    let db_path = path.path().join("places.sqlite");
                    if db_path.exists() {
                        path_results.push(db_path);
                    }
                }
            }
        }
    }

    path_results
}

fn check_and_copy_history(path: &PathBuf) -> Result<PathBuf, io::Error> {
    let proj_dirs = ProjectDirs::from("com", "athlabs", "carto").unwrap();
    let data_dir = proj_dirs.data_dir();

    // Create directory to store copy of data
    fs::create_dir_all(data_dir)?;

    // Copy data if we don't already have it.
    let data_path = data_dir.join("firefox.sqlite");
    // TODO: Check when the file was last updated and copy if newer.
    if !data_path.exists() {
        fs::copy(path, &data_path)?;
    }

    Ok(data_path)
}

fn main() -> Result<()> {
    // Detect profiles
    let profiles = detect_profiles();
    let db_path = profiles.first().expect("No Firefox history detected");

    if let Ok(db_path) = check_and_copy_history(&db_path) {
        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        println!("Connected to db...");

        let mut stmt = conn.prepare("SELECT id, url FROM moz_places where hidden = 0 LIMIT 10")?;
        let place_iter = stmt.query_map(params![], |row| {
            Ok(Place {
                id: row.get(0)?,
                url: row.get(1)?,
            })
        })?;

        for place in place_iter {
            println!("Found place {:?}", place.unwrap());
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::detect_profiles;

    #[test]
    fn test_detect_profiles() {
        let profiles = detect_profiles();
        assert!(profiles.len() > 0);
    }
}
