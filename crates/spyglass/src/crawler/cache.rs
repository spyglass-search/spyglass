use bytes::{Buf, Bytes};
use chrono::{DateTime, ParseResult, TimeZone, Utc};
use entities::models::lens;
use entities::sea_orm::DatabaseConnection;
use flate2::bufread::GzDecoder;
use std::{io::Read, path::PathBuf};

use http::HeaderValue;
use reqwest::header::{HeaderMap, IF_MODIFIED_SINCE, LAST_MODIFIED};
use reqwest::Response;
use shared::config::Config;
use std::io::{Error, ErrorKind, Write};

use crate::state::AppState;

// Root URL for the spyglass lens stroage
const CACHE_ROOT: &str = "https://spyglass-lens-cache.s3.amazonaws.com/";
// The header date format
const HEADER_DATE_FMT: &str = "%a, %d %b %Y %H:%M:%S GMT";

/// Requests cache from spyglass cache storage and stores it on disk. If the
/// cache has not been updated since the last time the cache was processed
/// then a new cache is not downloaded. In the case that no new cache file
/// is found and an old cache file still exists one disk then the cache
/// file reference is returned.
pub async fn update_cache(
    app_state: &AppState,
    config: &Config,
    lens: &String,
) -> anyhow::Result<(Option<PathBuf>, Option<DateTime<Utc>>), Error> {
    let update_time = get_last_cached(app_state, lens).await;
    let client = reqwest::Client::new();

    let lens_cache_file = format!("{}/parsed.gz", lens);

    // Add modified since header if a last update time exists
    let mut headers = HeaderMap::new();
    if let Some(date) = update_time {
        let header_val =
            HeaderValue::from_str(format!("{}", date.format(HEADER_DATE_FMT)).as_str());
        if let Ok(header) = header_val {
            headers.insert(IF_MODIFIED_SINCE, header);
        }
    }

    let req = client
        .get(format!("{}{}", CACHE_ROOT, lens_cache_file))
        .headers(headers);
    log::debug!("Requesting cache file {:?}", req);
    let resp = req.send().await;
    log::debug!("Cache file response {:?}", resp);
    match resp {
        Ok(resp) => {
            let cache_dir = config.cache_dir();
            let cache_file = cache_dir.join(lens).join("parsed.ron.gz");
            if resp.status().is_success() {
                store_cache(lens, resp, &cache_file, &app_state.db).await?;
                return Ok((Option::Some(cache_file), update_time));
            } else {
                if cache_file.exists() {
                    return Ok((Option::Some(cache_file), update_time));
                }
                return Ok((Option::None, update_time));
            }
        }
        Err(err) => {
            log::error!("Error accessing cache - {:?}", err);
            return Ok((Option::None, update_time));
        }
    }
}

/// Deletes the cache
pub fn delete_cache(cache_path: &PathBuf) -> std::io::Result<()> {
    std::fs::remove_file(cache_path)
}

// Helper method used to return timestamp of the last time the lens cache was
// downloaded
async fn get_last_cached(state: &AppState, lens: &String) -> Option<DateTime<Utc>> {
    let item = lens::find_by_name(lens, &state.db).await;
    match item {
        Ok(Some(model)) => model.last_cache_update,
        _ => Option::None,
    }
}

// Helper method used to store the cache to disk. The cache is streamed from the
// http response and sent through a gz decoder and written to disk. The max amount
// of memory growth is the size of the gz file.
async fn store_cache(
    lens: &String,
    resp: Response,
    storage_file: &PathBuf,
    database_connection: &DatabaseConnection,
) -> anyhow::Result<PathBuf, Error> {
    if let Some(parent) = storage_file.parent() {
        let dir_rslt = std::fs::create_dir_all(parent);
        if let Err(err) = dir_rslt {
            return Err(Error::new(ErrorKind::Other, err));
        }
    }

    let file_rslt = std::fs::File::create(storage_file);
    if let Ok(mut file) = file_rslt {
        // Grab the last modified header and update the database
        let last_mod = resp.headers().get(LAST_MODIFIED);
        if let Some(last_mod_date) = last_mod {
            let date_str_result = last_mod_date.to_str();
            if let Ok(date) = date_str_result {
                let result: ParseResult<DateTime<Utc>> =
                    Utc.datetime_from_str(date, HEADER_DATE_FMT);
                if let Ok(date_obj) = result {
                    let _ = lens::update_cache_time(lens, date_obj, database_connection).await;
                }
            }
        }

        if let Ok(bytes) = resp.bytes().await {
            let _ = file.write_all(&bytes);
        }
    }

    Result::Ok(storage_file.clone())
}
