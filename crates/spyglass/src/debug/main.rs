use clap::{Parser, Subcommand};
use entities::models;
use libspyglass::search::{self, IndexPath, ReadonlySearcher};
use ron::ser::PrettyConfig;
use shared::config::Config;
use spyglass_plugin::DocumentQuery;
use std::process::ExitCode;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

const LOG_LEVEL: tracing::Level = tracing::Level::INFO;

#[cfg(debug_assertions)]
const LIB_LOG_LEVEL: &str = "spyglassdebug=DEBUG";

#[cfg(not(debug_assertions))]
const LIB_LOG_LEVEL: &str = "spyglassdebug=INFO";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct CdxCli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    CrawlDetails { crawl_task_id: i64 },
    GetFileDetails { url: String },
}

#[tokio::main]
async fn main() -> ExitCode {
    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                .add_directive(LOG_LEVEL.into())
                .add_directive(LIB_LOG_LEVEL.parse().expect("invalid log filter")),
        )
        .with(fmt::Layer::new().with_writer(std::io::stdout));
    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");
    let _ = LogTracer::init();

    let cli = CdxCli::parse();
    if let Some(command) = cli.command {
        match command {
            Command::CrawlDetails { crawl_task_id } => {
                let config = Config::new();
                match models::create_connection(&config, false).await {
                    Ok(db) => {
                        let num_progress = models::crawl_queue::num_tasks_in_progress(&db)
                            .await
                            .unwrap_or_default();
                        let task_details =
                            models::crawl_queue::get_task_details(crawl_task_id, &db).await;

                        println!("## Task Details ##");
                        println!("Task Processing: {}", num_progress);
                        match task_details {
                            Ok(Some((task, tags))) => {
                                println!(
                                    "Crawl Task: {}",
                                    ron::ser::to_string_pretty(&task, PrettyConfig::new())
                                        .unwrap_or_default()
                                );
                                println!(
                                    "Tags: {}",
                                    ron::ser::to_string_pretty(&tags, PrettyConfig::new())
                                        .unwrap_or_default()
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
                    Err(error) => println!("Error accessing database {:?}", error),
                }
            }
            Command::GetFileDetails { url } => {
                let config = Config::new();
                match models::create_connection(&config, false).await {
                    Ok(db) => {
                        let doc_details =
                            models::indexed_document::get_document_details(&db, &url).await;
                        println!("## Document Details ##");
                        match doc_details {
                            Ok(Some((doc, tags))) => {
                                println!(
                                    "Document: {}",
                                    ron::ser::to_string_pretty(&doc, PrettyConfig::new())
                                        .unwrap_or_default()
                                );
                                println!(
                                    "Tags: {}",
                                    ron::ser::to_string_pretty(&tags, PrettyConfig::new())
                                        .unwrap_or_default()
                                );
                                let index = ReadonlySearcher::with_index(&IndexPath::LocalPath(
                                    config.index_dir(),
                                ))
                                .expect("Unable to open index.");
                                let docs = ReadonlySearcher::search_by_query(
                                    &db,
                                    &index,
                                    &DocumentQuery {
                                        urls: Some(vec![url.clone()]),
                                        ..Default::default()
                                    },
                                )
                                .await;
                                println!("### Indexed Document ###");
                                if docs.is_empty() {
                                    println!("No indexed document for url {:?}", &url);
                                } else {
                                    for (_score, doc_addr) in docs {
                                        if let Ok(Ok(doc)) = index
                                            .reader
                                            .searcher()
                                            .doc(doc_addr)
                                            .map(|doc| search::document_to_struct(&doc))
                                        {
                                            println!(
                                                "Indexed Document: {}",
                                                ron::ser::to_string_pretty(
                                                    &doc,
                                                    PrettyConfig::new()
                                                )
                                                .unwrap_or_default()
                                            );
                                        } else {
                                            println!(
                                                "Error accessing Doc at address {:?}",
                                                doc_addr
                                            );
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                println!("No document found for url {}", url);
                            }
                            Err(err) => {
                                println!("Error accessing document details {:?}", err);
                            }
                        }
                    }
                    Err(error) => println!("Error accessing database {:?}", error),
                }
            }
        }
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
