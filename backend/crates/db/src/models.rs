use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub password_hash: Option<String>,
    pub is_instance_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Membership {
    pub organization_id: Uuid,
    pub organization_name: String,
    pub role: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub user_id: Uuid,
    pub organization_id: Option<Uuid>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    /// First characters of the key, for recognising it in lists.
    pub prefix: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKeyAuthRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub domain: Option<String>,
    pub brand_name: Option<String>,
    /// ISO 3166-1 alpha-2 country of the project's default market.
    pub country_code: String,
    /// Language of the default market (ISO 639-1, optional region).
    pub language_code: String,
    /// Optional city/region for local markets.
    pub location_name: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub organization_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub progress: serde_json::Value,
    pub run_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ReportTemplate {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub instructions: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Report {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub summary: Option<String>,
    pub html: String,
    pub skill: Option<String>,
    pub template_id: Option<Uuid>,
    pub created_by: String,
    pub shared_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ReportListItem {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub summary: Option<String>,
    pub skill: Option<String>,
    pub template_id: Option<Uuid>,
    pub created_by: String,
    pub shared_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
