use anyhow::anyhow;
use clap::{Parser, Subcommand};
use entities::models::{self, indexed_document::DocumentIdentifier};
use libspyglass::state::AppState;
use ron::ser::PrettyConfig;
use shared::config::Config;
use spyglass_plugin::DocumentQuery;
use std::{path::PathBuf, process::ExitCode};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use libspyglass::pipeline::cache_pipeline::process_update;
use spyglass_searcher::{document_to_struct, IndexPath, QueryStats, Searcher};

#[cfg(debug_assertions)]
const LOG_LEVEL: &str = "spyglassdebug=DEBUG";
#[cfg(debug_assertions)]
const LIBSPYGLASS_LEVEL: &str = "libspyglass=DEBUG";

#[cfg(not(debug_assertions))]
const LOG_LEVEL: &str = "spyglassdebug=INFO";
#[cfg(not(debug_assertions))]
const LIBSPYGLASS_LEVEL: &str = "libspyglass=INFO";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct CdxCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Outputs crawl details for a crawl ID
    CrawlDetails {
        crawl_task_id: i64,
    },
    /// Outputs document metadata & content for a document ID
    GetDocumentDetails {
        id_or_url: String,
    },
    GetDocumentQueryExplanation {
        id_or_url: String,
        query: String,
    },
    /// Load a local lens archive into the index
    LoadArchive {
        name: String,
        archive_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive(LOG_LEVEL.parse().expect("Invalid log filter"))
                .add_directive("tantivy=WARN".parse().expect("Invalid EnvFilter"))
                .add_directive(LIBSPYGLASS_LEVEL.parse().expect("invalid log filter")),
        )
        .with(fmt::Layer::new().with_writer(std::io::stdout));
    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    let _ = LogTracer::init();

    let cli = CdxCli::parse();
    let config = Config::new();

    match cli.command {
        Command::CrawlDetails { crawl_task_id } => {
            let db = models::create_connection(&config, false).await?;
            let num_progress = models::crawl_queue::num_tasks_in_progress(&db)
                .await
                .unwrap_or_default();
            let task_details = models::crawl_queue::get_task_details(crawl_task_id, &db).await;

            println!("## Task Details ##");
            println!("Task Processing: {}", num_progress);
            match task_details {
                Ok(Some((task, tags))) => {
                    println!(
                        "Crawl Task: {}",
                        ron::ser::to_string_pretty(&task, PrettyConfig::new()).unwrap_or_default()
                    );
                    println!(
                        "Tags: {}",
                        ron::ser::to_string_pretty(&tags, PrettyConfig::new()).unwrap_or_default()
                    );
                }
                Ok(None) => {
                    println!("No task found for id {}", crawl_task_id);
                }
                Err(err) => {
                    println!("Error accessing task details {:?}", err);
                }
            }
        }
        Command::GetDocumentDetails { id_or_url } => {
            let db = models::create_connection(&config, false).await?;

            let identifier = if id_or_url.contains("://") {
                DocumentIdentifier::Url(&id_or_url)
            } else {
                DocumentIdentifier::DocId(&id_or_url)
            };

            let doc_details =
                models::indexed_document::get_document_details(&db, identifier).await?;

            println!("## Document Details ##");
            match doc_details {
                Some((doc, tags)) => {
                    println!(
                        "Document: {}",
                        ron::ser::to_string_pretty(&doc, PrettyConfig::new()).unwrap_or_default()
                    );
                    println!(
                        "Tags: {}",
                        ron::ser::to_string_pretty(&tags, PrettyConfig::new()).unwrap_or_default()
                    );
                    let index =
                        Searcher::with_index(&IndexPath::LocalPath(config.index_dir()), true)
                            .expect("Unable to open index.");

                    let docs = index
                        .search_by_query(&db, Some(vec![doc.url.clone()]), None, None, None)
                        .await;
                    println!("### Indexed Document ###");
                    if docs.is_empty() {
                        println!("No indexed document for url {:?}", &doc.url);
                    } else {
                        for (_score, doc_addr) in docs {
                            if let Ok(Ok(doc)) = index
                                .reader
                                .searcher()
                                .doc(doc_addr)
                                .map(|doc| document_to_struct(&doc))
                            {
                                println!(
                                    "Indexed Document: {}",
                                    ron::ser::to_string_pretty(&doc, PrettyConfig::new())
                                        .unwrap_or_default()
                                );
                            } else {
                                println!("Error accessing Doc at address {:?}", doc_addr);
                            }
                        }
                    }
                }
                None => println!("No document found for identifier: {}", id_or_url),
            }
        }
        Command::GetDocumentQueryExplanation { id_or_url, query } => {
            let db = models::create_connection(&config, false).await?;

            let doc_query = if id_or_url.contains("://") {
                DocumentQuery {
                    urls: Some(vec![id_or_url.clone()]),
                    ..Default::default()
                }
            } else {
                DocumentQuery {
                    ids: Some(vec![id_or_url.clone()]),
                    ..Default::default()
                }
            };

            let index = Searcher::with_index(&IndexPath::LocalPath(config.index_dir()), true)
                .expect("Unable to open index.");

            let docs = index
                .search_by_query(&db, doc_query.urls, doc_query.ids, None, None)
                .await;

            if docs.is_empty() {
                println!("No indexed document for url {:?}", id_or_url);
            } else {
                for (_score, doc_addr) in docs {
                    let mut stats = QueryStats::default();
                    let explain = Searcher::explain_search_with_lens(
                        &db,
                        doc_addr,
                        &vec![],
                        &index,
                        query.as_str(),
                        &mut stats,
                    )
                    .await;
                    match explain {
                        Some(explanation) => {
                            println!(
                                "Query \"{:?}\" for document {:?} \n {:?}",
                                query, id_or_url, explanation
                            );
                        }
                        None => {
                            println!("Could not get score for document");
                        }
                    }
                }
            }
        }
        Command::LoadArchive { name, archive_path } => {
            if !archive_path.exists() {
                eprintln!("{} does not exist!", archive_path.display());
                return Err(anyhow!("ARCHIVE_PATH does not exist"));
            }

            let config = Config::new();
            let state = AppState::new(&config, false).await;

            let lens = shared::config::LensConfig {
                author: "spyglass-search".into(),
                name: name.clone(),
                label: name,
                ..Default::default()
            };

            process_update(state.clone(), &lens, archive_path, true).await;
            let _ = state.index.save().await;
        }
    }

    Ok(ExitCode::SUCCESS)
}
