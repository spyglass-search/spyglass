use entities::models::{crawl_queue, indexed_document};
use entities::sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QueryOrder, Set};

use entities::schema::{DocFields, SearchDocument};
use libspyglass::crawler::Crawler;
use libspyglass::search::Searcher;
use libspyglass::state::AppState;
use shared::config::{Config, Lens};
use url::Url;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config = Config::new();
    // Load app configuration
    let state = AppState::new(&config).await;
    let crawler = Crawler::new();

    let fields = DocFields::as_fields();

    // Load all indexed documents from db
    let mut pages = indexed_document::Entity::find()
        .order_by_asc(indexed_document::Column::Id)
        .paginate(&state.db, 10);

    // Loop through each doc and re-run parsing on doc
    while let Some(docs) = pages.fetch_and_next().await? {
        for doc in docs.into_iter() {
            let indexed_doc = {
                let index_reader = &state.index.reader;
                Searcher::get_by_id(index_reader, &doc.doc_id)
            };

            if let Some(indexed_doc) = indexed_doc {
                let url = indexed_doc
                    .get_first(fields.url)
                    .unwrap()
                    .as_text()
                    .unwrap();
                let raw_body = indexed_doc
                    .get_first(fields.raw)
                    .unwrap()
                    .as_text()
                    .unwrap();

                // Scrape page
                let url = Url::parse(url).unwrap();
                let scrape = crawler.scrape_page(&url, raw_body).await;

                // Update document in index
                {
                    // Delete old document
                    let mut index_writer = state.index.writer.lock().unwrap();
                    Searcher::delete(&mut index_writer, &doc.doc_id).unwrap();
                }

                // Update document in DB
                let doc_id = {
                    let mut index_writer = state.index.writer.lock().unwrap();
                    Searcher::add_document(
                        &mut index_writer,
                        &scrape.title.unwrap_or_default(),
                        &scrape.description.unwrap_or_default(),
                        url.host_str().unwrap(),
                        url.as_str(),
                        &scrape.content.unwrap(),
                        &scrape.raw.unwrap(),
                    )
                    .unwrap()
                };

                let mut update: indexed_document::ActiveModel = doc.into();
                update.doc_id = Set(doc_id);
                update.save(&state.db).await.unwrap();

                // Update parsed links
                let to_add: Vec<String> = scrape.links.into_iter().collect();
                let lenses: Vec<Lens> = state
                    .lenses
                    .iter()
                    .map(|entry| entry.value().clone())
                    .collect();
                crawl_queue::enqueue_all(
                    &state.db,
                    &to_add,
                    &lenses,
                    &state.user_settings,
                    &Default::default(),
                )
                .await?;
            }
        }
    }

    Ok(())
}
