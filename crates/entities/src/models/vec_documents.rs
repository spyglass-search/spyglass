use sea_orm::{ConnectionTrait, DbErr, ExecResult, FromQueryResult, Statement};

pub async fn insert_embedding<C>(db: &C, id: i64, embedding: &[f32]) -> Result<ExecResult, DbErr>
where
    C: ConnectionTrait,
{
    let embedding_string = serde_json::to_string(embedding)
        .map_err(|err| {
            log::error!("Error {:?}", err);
            err
        })
        .unwrap();
    insert_or_update_embedding_str(db, id, &embedding_string, false).await
}

pub async fn update_embedding<C>(db: &C, id: i64, embedding: &[f32]) -> Result<ExecResult, DbErr>
where
    C: ConnectionTrait,
{
    let embedding_string = serde_json::to_string(embedding)
        .map_err(|err| {
            log::error!("Error {:?}", err);
            err
        })
        .unwrap();
    insert_or_update_embedding_str(db, id, &embedding_string, true).await
}

pub async fn insert_or_update_embedding_str<C>(
    db: &C,
    id: i64,
    embedding: &str,
    is_update: bool,
) -> Result<ExecResult, DbErr>
where
    C: ConnectionTrait,
{
    let statement = if is_update {
        Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            update vec_documents set embedding = $2
                where rowid = $1
            "#,
            vec![id.into(), embedding.into()],
        )
    } else {
        Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            insert into vec_documents(rowid, embedding)
                VALUES($1, $2)
            "#,
            vec![id.into(), embedding.into()],
        )
    };

    db.execute(statement).await
}

pub async fn delete_embedding_by_id<C>(db: &C, id: i64) -> Result<ExecResult, DbErr>
where
    C: ConnectionTrait,
{
    let statement = Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
            delete from vec_documents where rowid = $1;
        "#,
        vec![id.into()],
    );

    db.execute(statement).await
}

pub async fn delete_embedding_by_ids<C>(db: &C, ids: &[i64]) -> Result<ExecResult, DbErr>
where
    C: ConnectionTrait,
{
    let st = format!(
        r#"
        delete from vec_documents where rowid in ({})
        "#,
        ids.iter()
            .map(|id| format!("{}", id))
            .collect::<Vec<String>>()
            .join(",")
    );
    let statement = Statement::from_string(db.get_database_backend(), st);

    db.execute(statement).await
}

pub async fn delete_embeddings_by_url<C>(db: &C, urls: &[String]) -> Result<ExecResult, DbErr>
where
    C: ConnectionTrait,
{
    let urls_list_str = urls
        .iter()
        .map(|url| format!("\"{}\"", url))
        .collect::<Vec<String>>()
        .join(",")
        .to_string();

    let statement = Statement::from_string(
        db.get_database_backend(),
        format!(
            r#"
            delete from vec_documents
            where rowid in (
                select id from indexed_document
                    where indexed_document.url in ({})
            );
        "#,
            urls_list_str
        ),
    );

    db.execute(statement).await
}

#[derive(Debug, FromQueryResult)]
pub struct DocDistance {
    pub id: i64,
    pub distance: f64,
    pub doc_id: String,
    pub url: String,
    pub segment_start: i64,
    pub segment_end: i64,
}

// Returns the topX documents based on the distance. (The smaller the number the
// better the result). A single document that has multiple segments in the
// topX will only be returned once and the segment provided will only be the
// top result. get_top_context can be used to get the highest ranked context
pub async fn get_document_distance<C>(
    db: &C,
    lens_ids: &[u64],
    embedding: &[f32],
    top_x: u32,
) -> Result<Vec<DocDistance>, DbErr>
where
    C: ConnectionTrait,
{
    let embedding_string = serde_json::to_string(embedding)
        .map_err(|err| {
            log::error!("Error {:?}", err);
            err
        })
        .unwrap();

    let k_size = 3 * top_x;
    let statement = if !lens_ids.is_empty() {
        let query = format!(
            r#"
                WITH RankedScores AS (
                    SELECT
                        indexed_document.id AS score_id,
                        vd.distance,
                        indexed_document.doc_id,
                        indexed_document.url,
                        vti.segment_start,
                        vti.segment_end,
                        ROW_NUMBER() OVER (PARTITION BY indexed_document.doc_id ORDER BY vd.distance ASC) AS rank
                    FROM
                        vec_documents vd
                    left JOIN
                        vec_to_indexed vti
                        ON vd.rowid = vti.id
                    left JOIN indexed_document
                        ON vti.indexed_id = indexed_document.id
                    left JOIN document_tag on document_tag.indexed_document_id = indexed_document.id
                    WHERE document_tag.id in ({}) AND vd.embedding MATCH $1 AND k = $2 ORDER BY vd.distance ASC
                )
                SELECT score_id as id, distance, doc_id, url, segment_start, segment_end FROM RankedScores WHERE rank = 1 ORDER BY distance ASC limit $3;
            "#,
            lens_ids
                .iter()
                .map(|val| val.to_string())
                .collect::<Vec<String>>()
                .join(",")
        );

        Statement::from_sql_and_values(
            db.get_database_backend(),
            query,
            vec![embedding_string.into(), k_size.into(), top_x.into()],
        )
    } else {
        Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
                WITH RankedScores AS (
                    SELECT
                        indexed_document.id AS score_id,
                        vd.distance,
                        indexed_document.doc_id,
                        indexed_document.url,
                        vti.segment_start,
                        vti.segment_end,
                        ROW_NUMBER() OVER (PARTITION BY indexed_document.doc_id ORDER BY vd.distance ASC) AS rank
                    FROM
                        vec_documents vd
                    left JOIN
                        vec_to_indexed vti
                        ON vd.rowid = vti.id
                    left JOIN indexed_document
                        ON vti.indexed_id = indexed_document.id
                    WHERE vd.embedding MATCH $1 AND k = $2 ORDER BY vd.distance ASC
                )
                SELECT score_id as id, distance, doc_id, url, segment_start, segment_end FROM RankedScores WHERE rank = 1 ORDER BY distance ASC limit $3;
            "#,
            vec![embedding_string.into(), k_size.into(), top_x.into()],
        )
    };

    DocDistance::find_by_statement(statement)
        .all(db)
        .await
        .map_err(|err| {
            log::error!("Error is {:?}", err);
            err
        })
}

pub async fn get_top_context<C>(
    db: &C,
    lens_ids: &[u64],
    embedding: &[f32],
    top_x: u32,
) -> Result<Vec<DocDistance>, DbErr>
where
    C: ConnectionTrait,
{
    let embedding_string = serde_json::to_string(embedding)
        .map_err(|err| {
            log::error!("Error {:?}", err);
            err
        })
        .unwrap();
    let statement = if !lens_ids.is_empty() {
        Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
                SELECT
                    indexed_document.id AS id,
                    vd.distance,
                    indexed_document.doc_id,
                    vti.segment_start,
                    vti.segment_end,
                    indexed_document.url
                FROM
                    vec_documents vd
                left JOIN
                    vec_to_indexed vti
                    ON vd.rowid = vti.id
                left JOIN indexed_document
                    ON vti.indexed_id = indexed_document.id
                left join document_tag on document_tag.indexed_document_id = indexed_document.id
                WHERE document_tag.id in $1 AND vd.embedding MATCH $2 AND k = $3 ORDER BY vd.distance ASC
            "#,
            vec![
                lens_ids.to_owned().into(),
                embedding_string.into(),
                top_x.into(),
            ],
        )
    } else {
        Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
                SELECT
                    indexed_document.id AS score_id,
                    vd.distance,
                    indexed_document.doc_id,
                    indexed_document.url,
                    vti.segment_start,
                    vti.segment_end,
                    ROW_NUMBER() OVER (PARTITION BY indexed_document.doc_id ORDER BY vd.distance ASC) AS rank
                FROM
                    vec_documents vd
                left JOIN
                    vec_to_indexed vti
                    ON vd.rowid = vti.id
                left JOIN indexed_document
                    ON vti.indexed_id = indexed_document.id
                WHERE vd.embedding MATCH $1 AND k = $2 ORDER BY vd.distance ASC
            "#,
            vec![embedding_string.into(), top_x.into()],
        )
    };

    DocDistance::find_by_statement(statement)
        .all(db)
        .await
        .map_err(|err| {
            log::error!("Error is {:?}", err);
            err
        })
}
