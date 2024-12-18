use anyhow::anyhow;
use clap::{Parser, Subcommand};
use entities::models::vec_documents::{self, DocDistance};
use entities::models::{self, indexed_document::DocumentIdentifier, tag::check_query_for_tags};
use libspyglass::documents::DocumentQuery;
use libspyglass::state::AppState;
use ron::ser::PrettyConfig;
use shared::config::Config;
use shared::llm::{ChatMessage, ChatRole, ChatStream, LlmSession};
use spyglass_llm::LlmClient;
use spyglass_model_interface::embedding_api::EmbeddingApi;
use std::collections::HashMap;
use std::{path::PathBuf, process::ExitCode};
use tokio::sync::mpsc;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

use libspyglass::pipeline::cache_pipeline::process_update;
use spyglass_searcher::schema::SearchDocument;
use spyglass_searcher::SearchTrait;
use spyglass_searcher::{client::Searcher, schema::DocFields, Boost, IndexBackend, QueryBoost};
use std::io::Write;

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
    AskDocument {
        id_or_url: String,
        question: String,
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

            let schema = DocFields::as_schema();
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
                    let index = Searcher::with_index(
                        &IndexBackend::LocalPath(config.index_dir()),
                        schema,
                        true,
                    )
                    .expect("Unable to open index.");

                    let docs = index
                        .search_by_query(Some(vec![doc.url.clone()]), None, &[], &[])
                        .await;
                    println!("### Indexed Document ###");
                    if docs.is_empty() {
                        println!("No indexed document for url {:?}", &doc.url);
                    } else {
                        for (_score, doc) in docs {
                            println!(
                                "Indexed Document: {}",
                                ron::ser::to_string_pretty(&doc, PrettyConfig::new())
                                    .unwrap_or_default()
                            );
                        }
                    }
                }
                None => println!("No document found for identifier: {}", id_or_url),
            }
        }
        Command::AskDocument {
            id_or_url,
            question,
        } => {
            let (tx, mut rx) = mpsc::channel(10);
            // Spawn a task to stream the chat resp
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    match msg {
                        ChatStream::LoadingPrompt => {
                            println!("loading prompt...");
                        }
                        ChatStream::ChatStart => {
                            println!("starting generation...");
                        }
                        ChatStream::Token(tok) => {
                            print!("{tok}");
                            std::io::stdout().flush().unwrap();
                        }
                        ChatStream::ChatDone => {
                            println!("🤖");
                            println!("DONE!");
                        }
                    }
                }
            });

            let identifier = if id_or_url.contains("://") {
                DocumentIdentifier::Url(&id_or_url)
            } else {
                DocumentIdentifier::DocId(&id_or_url)
            };

            let db = models::create_connection(&config, false).await?;

            let doc_details =
                models::indexed_document::get_document_details(&db, identifier).await?;

            if let Some(doc_details) = doc_details {
                let schema = DocFields::as_schema();
                let index = Searcher::with_index(
                    &IndexBackend::LocalPath(config.index_dir()),
                    schema,
                    true,
                )
                .expect("Unable to open index.");

                let embedding_api = EmbeddingApi::new(config.embedding_model_dir()).unwrap();
                if let Ok(embeddings) = embedding_api.embed(
                    &question,
                    spyglass_model_interface::embedding_api::EmbeddingContentType::Query,
                ) {
                    if let Some(embedding) = embeddings.first() {
                        if let Ok(mut segments) = vec_documents::get_context_for_doc(
                            &db,
                            doc_details.0.id,
                            &embedding.embedding,
                        )
                        .await
                        {
                            let _ = segments.split_off(2.min(segments.len()));
                            let context = concat_context(&segments, &index).await;
                            let prompt = LlmSession {
                                    messages: vec![
                                        ChatMessage {
                                            role: ChatRole::System,
                                            content: "You are a helpful AI assistant that reviews possible relevant document context and answers questions about the documents".into(),
                                        },
                                        ChatMessage {
                                            role: ChatRole::User,
                                            content: format!("Here is the documents semantically related to the question:\n {}",context),
                                        },
                                        ChatMessage {
                                            role: ChatRole::User,
                                            content: format!("Here is my question: {}", question),
                                        },
                                    ],
                                };

                            match LlmClient::new(
                                config
                                    .llm_model_dir()
                                    .join("llama3")
                                    .join("Llama-3.2-3B-Instruct.Q5_K_M.gguf"),
                            ) {
                                Ok(mut client) => {
                                    client.chat(&prompt, Some(tx)).await?;
                                }
                                Err(error) => {
                                    log::error!("Error loading model {error}");
                                }
                            }
                        }
                    }
                }
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

            let schema = DocFields::as_schema();
            let index =
                Searcher::with_index(&IndexBackend::LocalPath(config.index_dir()), schema, true)
                    .expect("Unable to open index.");

            let docs = index
                .search_by_query(doc_query.urls, doc_query.ids, &[], &[])
                .await;

            if docs.is_empty() {
                println!("No indexed document for url {:?}", id_or_url);
            } else {
                for (_score, doc) in docs {
                    let boosts = check_query_for_tags(&db, &query)
                        .await
                        .iter()
                        .map(|x| QueryBoost::new(Boost::Tag(*x)))
                        .collect::<Vec<_>>();

                    let explain = index
                        .explain_search_with_lens(doc.doc_id, query.as_str(), &boosts)
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

#[allow(dead_code)]
pub async fn concat_context(distances: &[DocDistance], searcher: &Searcher) -> String {
    let mut map = HashMap::<String, usize>::new();
    let mut sorted: Vec<Vec<&DocDistance>> = Vec::new();
    // documents are already ordered now we just want to group documents by
    // uuid incase there are multiple results per document
    for distance in distances {
        match map.get(&distance.doc_id) {
            Some(index) => {
                if let Some(vec) = sorted.get_mut(*index) {
                    vec.push(distance);
                }
            }
            None => {
                let index = sorted.len();
                sorted.push(vec![distance]);
                map.insert(distance.doc_id.clone(), index);
            }
        }
    }

    let mut context_text = "Context for all documents\n".to_string();
    for grouped_results in sorted {
        let first = grouped_results.first();
        if let Some(first) = first {
            context_text.push_str(
                "\n\n-----------------------------------------------------------------\n\n",
            );
            context_text.push_str(&format!(
                "Document UUID: {} URL: {} \n\n ",
                first.doc_id, first.url,
            ));
        }

        for (i, doc_distance) in grouped_results.iter().enumerate() {
            if let Some(context) = pull_context(doc_distance, searcher).await {
                context_text.push_str(&format!(
                    "Context Segment -- #{} -- score #{}\n\n Context Text: {} \n\n",
                    i, doc_distance.distance, context
                ));
            }
        }
    }
    context_text
}

#[allow(dead_code)]
async fn pull_context(distance: &DocDistance, searcher: &Searcher) -> Option<String> {
    if let Some(document) = searcher.get(&distance.doc_id).await {
        if distance.segment_start == 0
            && distance.segment_end == ((document.content.len() - 1) as i64)
        {
            Some(document.content)
        } else {
            let segment = document
                .content
                .trim()
                .char_indices()
                .filter_map(|(i, c)| {
                    let index = i as i64;
                    if index >= distance.segment_start && index < distance.segment_end {
                        Some(c)
                    } else {
                        None
                    }
                })
                .collect::<String>();

            Some(segment)
        }
    } else {
        None
    }
}
