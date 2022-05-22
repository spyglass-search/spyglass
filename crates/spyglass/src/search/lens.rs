use std::fs;

use entities::models::lens;
use shared::config::{Config, Lens};

use crate::crawler::bootstrap;
use crate::state::AppState;

async fn create_default_lens(config: &Config) {
    // Create a default lens as an example.
    let lens = Lens {
        author: "Spyglass".to_string(),
        version: "1".to_string(),
        name: "wiki".to_string(),
        description: Some(
            "Search through official user-supported wikis for knowledge, games, and more."
                .to_string(),
        ),
        domains: vec!["blog.rust-lang.org".into(), "wiki.factorio.com".into()],
        urls: vec![
            "https://https://en.wikipedia.org/wiki/Portal:".into(),
            "https://doc.rust-lang.org/book/".into(),
            "https://oldschool.runescape.wiki/w/".into(),
        ],
        is_enabled: true,
    };

    fs::write(
        config.lenses_dir().join("wiki.ron"),
        ron::ser::to_string_pretty(&lens, Default::default()).unwrap(),
    )
    .expect("Unable to save default lens file.");
}

pub async fn read_lenses(state: &AppState, config: &Config) -> anyhow::Result<()> {
    let lense_dir = config.lenses_dir();
    let mut num_lenses = 0;

    for entry in (fs::read_dir(lense_dir)?).flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "ron" {
            if let Ok(file_contents) = fs::read_to_string(path) {
                match ron::from_str::<Lens>(&file_contents) {
                    Err(err) => log::error!("Unable to load lens {:?}: {}", entry.path(), err),
                    Ok(lens) => {
                        num_lenses += 1;
                        if lens.is_enabled {
                            log::info!("Loaded lens {}", lens.name);
                            state.lenses.insert(lens.name.clone(), lens);
                        } else {
                            state.lenses.remove(&lens.name.clone());
                        }
                    }
                }
            }
        }
    }

    if num_lenses == 0 {
        create_default_lens(config).await;
    }

    Ok(())
}

pub async fn load_lenses(state: AppState) {
    let mut new_lenses: Vec<Lens> = Vec::new();

    for entry in state.lenses.iter() {
        let lens = entry.value();

        // Have we added this lens to the database?
        match lens::add(
            &state.db,
            &lens.name,
            &lens.author,
            lens.description.as_ref(),
            &lens.version,
        )
        .await
        {
            Ok(true) => {
                log::info!("found new lens {}", lens.name);
                new_lenses.push(lens.clone());
            }
            Ok(false) => log::info!("lens ({}) already added", lens.name),
            Err(e) => log::error!("error loading lens {}", e),
        }
    }

    // Bootstrap new lenses
    log::info!("bootstraping new lenses");
    for lens in new_lenses {
        for domain in lens.domains.iter() {
            match bootstrap::bootstrap(
                &state.db,
                &state.user_settings,
                // Safe to assume domains always have HTTPS support?
                &format!("https://{}", domain),
            )
            .await
            {
                Err(e) => log::error!("{}", e),
                Ok(cnt) => log::info!("bootstrapped {} w/ {} urls", domain, cnt),
            }
        }

        for prefix in lens.urls.iter() {
            match bootstrap::bootstrap(&state.db, &state.user_settings, prefix).await {
                Err(e) => log::error!("{}", e),
                Ok(cnt) => log::info!("bootstrapped {} w/ {} urls", prefix, cnt),
            }
        }
    }

    log::info!("finished bootstrapping");
}
