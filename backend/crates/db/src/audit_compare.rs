use crate::{audits::Audit, internal};
use common::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct ComparisonPage {
    pub id: Uuid,
    pub url: String,
    pub comparable: bool,
    /// Field fingerprints keep large HTML metadata out of the comparison response.
    pub values: serde_json::Value,
    pub issues: Vec<String>,
}

pub struct ComparisonEvidence {
    pub baseline: Audit,
    pub current: Audit,
    pub before: Vec<ComparisonPage>,
    pub after: Vec<ComparisonPage>,
}

pub async fn evidence(
    pool: &PgPool,
    org: Uuid,
    pid: Uuid,
    baseline: Uuid,
    current: Uuid,
) -> AppResult<ComparisonEvidence> {
    let mut tx = pool.begin().await.map_err(internal)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let mut audits = Vec::new();
    let mut pages = Vec::new();
    for id in [baseline, current] {
        let audit: Audit = sqlx::query_as("SELECT * FROM audits WHERE id=$1 AND project_id=$2 AND EXISTS(SELECT 1 FROM projects WHERE id=$2 AND organization_id=$3)")
            .bind(id).bind(pid).bind(org).fetch_optional(&mut *tx).await.map_err(internal)?.ok_or(AppError::NotFound("audit"))?;
        if audit.status != "completed" {
            return Err(AppError::Conflict(
                "both audits must be completed before comparison".into(),
            ));
        }
        let values: Vec<ComparisonPage> = sqlx::query_as(
            "SELECT p.id,p.url,(p.status_code=200 AND p.fetch_class='html' AND jsonb_array_length(p.redirect_chain)=0) IS TRUE AS comparable,
             jsonb_build_object('status_code',p.status_code,'fetch_class',p.fetch_class,'redirected',jsonb_array_length(p.redirect_chain)>0,
                'title',md5(p.title),'meta_description',md5(p.meta_description),'h1',md5(p.h1),'canonical_url',md5(p.canonical_url),
                'header_canonical_url',md5(p.header_canonical_url),'is_indexable',p.is_indexable,'content_hash',p.content_hash,
                'word_count',p.word_count,'depth',p.depth,'inlinks',p.inlinks) AS values,
             COALESCE((SELECT array_agg(DISTINCT i.issue_type ORDER BY i.issue_type) FROM audit_issues i WHERE i.audit_id=$1 AND i.page_id=p.id),'{}'::text[]) AS issues
             FROM audit_pages p WHERE p.audit_id=$1 ORDER BY p.url LIMIT 10001")
            .bind(id).fetch_all(&mut *tx).await.map_err(internal)?;
        if values.len() > 10000 {
            return Err(AppError::Conflict(
                "comparison supports at most 10000 stored pages per audit".into(),
            ));
        }
        audits.push(audit);
        pages.push(values);
    }
    tx.commit().await.map_err(internal)?;
    let current = audits.pop().unwrap();
    let baseline = audits.pop().unwrap();
    let after = pages.pop().unwrap();
    let before = pages.pop().unwrap();
    Ok(ComparisonEvidence {
        baseline,
        current,
        before,
        after,
    })
}
