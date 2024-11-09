use entities::{
    models::{embedding_queue, vec_documents},
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
        Ok(Some(job)) => {
            match job.content {
                Some(content) => {
                    let embedding = if let Some(api) = state.embedding_api.load_full().as_ref() {
                        api.embed(&content, EmbeddingContentType::Document)
                    } else {
                        Err(anyhow::format_err!(
                            "Embedding Model is not properly configured"
                        ))
                    };
                    match embedding {
                        Ok(embedding) => {
                            match vec_documents::insert_embedding(
                                &state.db,
                                job.indexed_document_id,
                                &embedding,
                            )
                            .await
                            {
                                Ok(_) => {
                                    let _ = embedding_queue::mark_done(&state.db, job_id).await;
                                }
                                Err(insert_error) => {
                                    // The virtual table does not support on conflict so we try to
                                    // insert first then update.
                                    match vec_documents::update_embedding(
                                        &state.db,
                                        job.indexed_document_id,
                                        &embedding,
                                    )
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
                                                    "Error storing embedding for {}. Error {:?} and {:?}",
                                                    job.document_id, insert_error, error
                                                )),
                                            )
                                            .await;
                                        }
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
            }
        }
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
