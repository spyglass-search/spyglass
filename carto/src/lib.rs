use directories::ProjectDirs;
use reqwest::StatusCode;
use std::{fs, path::PathBuf};

pub mod models;
use models::Place;

pub struct Carto {
    data_dir: PathBuf,
}

impl Carto {
    pub fn init() -> Self {
        let proj_dirs = ProjectDirs::from("com", "athlabs", "carto").unwrap();
        let data_dir = proj_dirs.data_dir().join("crawls");

        fs::create_dir_all(&data_dir).expect("Unable to create crawl folder");

        Carto { data_dir }
    }

    // TODO: Load web indexing as a plugin?
    pub async fn fetch(&self, place: &Place) {
        // Make sure cache directory exists for this domain
        let url = &place.url;
        let domain_dir = self.data_dir.join(url.host_str().unwrap());
        if !domain_dir.exists() {
            fs::create_dir(&domain_dir).expect("Unable to create dir");
        }

        log::info!("Fetching page: {}", place.url.as_str());
        let res = reqwest::get(place.url.as_str()).await.unwrap();
        log::info!("Status: {}", res.status());
        if res.status() == StatusCode::OK {
            // TODO: Save headers
            log::info!("Headers:\n{:?}", res.headers());
            let body = res.text().await.unwrap();
            let file_path = domain_dir.join("raw.html");
            fs::write(file_path, body).expect("Unable to save html");
        }
    }
}
