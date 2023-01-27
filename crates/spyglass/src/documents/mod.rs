use std::collections::HashMap;

use entities::models::{crawl_queue, indexed_document};
use entities::sea_orm::entity::prelude::*;

use crate::{search::Searcher, state::AppState};

/// Helper method to delete indexed documents, crawl queue items and search
/// documents by url
pub async fn delete_documents_by_uri(state: &AppState, uri: Vec<String>) {
    log::debug!("Deleting {:?} documents", uri.len());

    // Delete from crawl queue

    if let Err(error) = crawl_queue::delete_many_by_url(&state.db, &uri).await {
        log::error!("Error delete items from crawl queue {:?}", error);
    }

    // find all documents that already exist with that url
    let existing: Vec<indexed_document::Model> = indexed_document::Entity::find()
        .filter(indexed_document::Column::Url.is_in(uri.clone()))
        .all(&state.db)
        .await
        .unwrap_or_default();

    // build a hash map of Url to the doc id
    let mut id_map = HashMap::new();
    for model in &existing {
        let _ = id_map.insert(model.url.to_string(), model.doc_id.clone());
    }

    // build a list of doc ids to delete from the index
    let doc_id_list = id_map
        .values()
        .into_iter()
        .map(|x| x.to_owned())
        .collect::<Vec<String>>();

    let _ = Searcher::delete_many_by_id(state, &doc_id_list, false).await;
    let _ = Searcher::save(state).await;

    // now that the documents are deleted delete from the queue
    if let Err(error) = indexed_document::delete_many_by_url(&state.db, uri).await {
        log::error!("Error deleting for indexed document store {:?}", error);
    }
}
