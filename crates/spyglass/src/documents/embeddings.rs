use entities::{
    models::{embedding_queue, vec_documents, vec_to_indexed},
    sea_orm::EntityTrait,
};
use spyglass_model_interface::embedding_api::EmbeddingContentType;

use crate::state::AppState;

pub async fn trigger_processing_embedding(state: &AppState, job_id: i64) {
    let _ = tokio::spawn(processing_embedding(state.clone(), job_id)).await;
}

pub async fn processing_embedding(state: AppState, job_id: i64) {
    match embedding_queue::Entity::find_by_id(job_id)
        .one(&state.db)
        .await
    {
        Ok(Some(job)) => match job.content {
            Some(content) => {
                let embeddings = if let Some(api) = state.embedding_api.load_full().as_ref() {
                    api.embed(&content, EmbeddingContentType::Document)
                } else {
                    Err(anyhow::format_err!(
                        "Embedding Model is not properly configured"
                    ))
                };
                match embeddings {
                    Ok(embeddings) => {
                        if let Err(error) = vec_to_indexed::delete_all_for_document(
                            &state.db,
                            job.indexed_document_id,
                        )
                        .await
                        {
                            log::error!("Error deleting document vectors {:?}", error);
                        }

                        for embedding in embeddings {
                            match vec_to_indexed::insert_embedding_mapping(
                                &state.db,
                                job.indexed_document_id,
                            )
                            .await
                            {
                                Ok(insert_result) => {
                                    let id: i64 = insert_result.last_insert_id;
                                    match vec_documents::insert_embedding(&state.db, id, &embedding)
                                        .await
                                    {
                                        Ok(_) => {
                                            let _ =
                                                embedding_queue::mark_done(&state.db, job_id).await;
                                        }
                                        Err(error) => {
                                            let _ = embedding_queue::mark_failed(
                                                &state.db,
                                                job_id,
                                                Some(format!(
                                                    "Error storing embedding for {}. Error {:?}",
                                                    job.document_id, error
                                                )),
                                            )
                                            .await;
                                        }
                                    }
                                }
                                Err(error) => {
                                    log::error!("Error inserting mapping {:?}", error);
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let _ = embedding_queue::mark_failed(
                            &state.db,
                            job_id,
                            Some(format!(
                                "Error generating embedding for {}. Error {:?}",
                                job.document_id, error
                            )),
                        )
                        .await;
                    }
                }
            }
            None => {
                let _ = embedding_queue::mark_failed(
                    &state.db,
                    job_id,
                    Some(format!("No content found for document {}", job.document_id)),
                )
                .await;
            }
        },
        Ok(None) => {
            let _ = embedding_queue::mark_failed(
                &state.db,
                job_id,
                Some(format!("Job {} not found", job_id)),
            )
            .await;
        }
        Err(error) => {
            let _ = embedding_queue::mark_failed(
                &state.db,
                job_id,
                Some(format!(
                    "Unable to access job {}. Error {:?}",
                    job_id, error
                )),
            )
            .await;
        }
    }
}
