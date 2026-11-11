use super::audits::csv_cell;
use crate::auth::Auth;
use crate::error::{ApiResult, Problem};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use db::keywords::{KeywordIdea, KeywordResearch, SavedKeyword};
use domain::audits::Paged;
use domain::keywords::{
    DeleteSavedInput, DeleteSavedResult, IdeaQuery, SaveKeywordsInput, SaveKeywordsResult,
    SavedQuery, SerpAnalysisInput, StartResearchInput, UpdateSavedInput,
};
use jobs::serp::SerpAnalysis;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_keyword_research, start_keyword_research))
        .routes(routes!(get_keyword_research, delete_keyword_research))
        .routes(routes!(list_keyword_ideas))
        .routes(routes!(export_keyword_ideas))
        .routes(routes!(list_saved_keywords, save_keywords))
        .routes(routes!(update_saved_keyword))
        .routes(routes!(delete_saved_keywords))
        .routes(routes!(list_saved_keyword_tags))
        .routes(routes!(export_saved_keywords))
        .routes(routes!(analyze_serp))
        .routes(routes!(refresh_saved_keywords))
}

#[utoipa::path(get, path = "/projects/{project_id}/keyword-research", tag = "keywords",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Vec<KeywordResearch>)),
    security(("session" = []), ("bearer" = [])))]
async fn list_keyword_research(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<KeywordResearch>>> {
    Ok(Json(
        domain::keywords::list(&state.pool, &actor, project_id).await?,
    ))
}

/// Expand a seed keyword into ideas with popularity, intent, and (for the top ideas)
/// difficulty and SERP features. Runs in the background; poll the research until its status
/// is `completed`. Metrics that cannot be determined are `null`, never 0.
#[utoipa::path(post, path = "/projects/{project_id}/keyword-research", tag = "keywords",
    params(("project_id" = Uuid, Path)), request_body = StartResearchInput,
    responses((status = ACCEPTED, body = KeywordResearch), (status = CONFLICT, body = Problem),
              (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn start_keyword_research(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<StartResearchInput>,
) -> ApiResult<(StatusCode, Json<KeywordResearch>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(domain::keywords::start(&state.pool, &actor, project_id, input).await?),
    ))
}

#[utoipa::path(get, path = "/projects/{project_id}/keyword-research/{research_id}", tag = "keywords",
    params(("project_id" = Uuid, Path), ("research_id" = Uuid, Path)),
    responses((status = OK, body = KeywordResearch), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn get_keyword_research(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, research_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<KeywordResearch>> {
    Ok(Json(
        domain::keywords::get(&state.pool, &actor, project_id, research_id).await?,
    ))
}

#[utoipa::path(delete, path = "/projects/{project_id}/keyword-research/{research_id}", tag = "keywords",
    params(("project_id" = Uuid, Path), ("research_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn delete_keyword_research(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, research_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    domain::keywords::delete(&state.pool, &actor, project_id, research_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/projects/{project_id}/keyword-research/{research_id}/ideas", tag = "keywords",
    params(("project_id" = Uuid, Path), ("research_id" = Uuid, Path), IdeaQuery),
    responses((status = OK, body = Paged<KeywordIdea>)), security(("session" = []), ("bearer" = [])))]
async fn list_keyword_ideas(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, research_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<IdeaQuery>,
) -> ApiResult<Json<Paged<KeywordIdea>>> {
    Ok(Json(
        domain::keywords::ideas(&state.pool, &actor, project_id, research_id, q).await?,
    ))
}

const CSV_HEAD: &str = "keyword,popularity,est_volume,volume_bucket,volume_source,volume_confidence,kd,intent,serp_features,sources_or_tags\n";

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn csv_response(name: String, csv: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        csv,
    )
        .into_response()
}

/// All ideas of a research as CSV. Empty cells mean "unknown".
#[utoipa::path(get, path = "/projects/{project_id}/keyword-research/{research_id}/ideas.csv", tag = "keywords",
    params(("project_id" = Uuid, Path), ("research_id" = Uuid, Path)),
    responses((status = OK, content_type = "text/csv", body = String)),
    security(("session" = []), ("bearer" = [])))]
async fn export_keyword_ideas(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, research_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Response> {
    let page = domain::keywords::ideas(
        &state.pool,
        &actor,
        project_id,
        research_id,
        IdeaQuery {
            limit: Some(1000),
            ..Default::default()
        },
    )
    .await?;
    let mut csv = String::from(CSV_HEAD);
    for i in &page.items {
        csv.push_str(
            &[
                csv_cell(&i.keyword),
                i.popularity.to_string(),
                opt(i.est_volume),
                csv_cell(i.volume_bucket.as_deref().unwrap_or("")),
                csv_cell(i.volume_source.as_deref().unwrap_or("")),
                csv_cell(i.volume_confidence.as_deref().unwrap_or("")),
                opt(i.kd),
                csv_cell(&i.intent),
                csv_cell(&i.serp_features.clone().unwrap_or_default().join(" ")),
                csv_cell(&i.sources.join(" ")),
            ]
            .join(","),
        );
        csv.push('\n');
    }
    Ok(csv_response(format!("keywords-{research_id}.csv"), csv))
}

#[utoipa::path(get, path = "/projects/{project_id}/saved-keywords", tag = "keywords",
    params(("project_id" = Uuid, Path), SavedQuery),
    responses((status = OK, body = Paged<SavedKeyword>)), security(("session" = []), ("bearer" = [])))]
async fn list_saved_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Query(q): Query<SavedQuery>,
) -> ApiResult<Json<Paged<SavedKeyword>>> {
    Ok(Json(
        domain::keywords::saved(&state.pool, &actor, project_id, q).await?,
    ))
}

/// Save ideas of a research (`research_id` + `idea_ids`) or keywords typed by hand.
#[utoipa::path(post, path = "/projects/{project_id}/saved-keywords", tag = "keywords",
    params(("project_id" = Uuid, Path)), request_body = SaveKeywordsInput,
    responses((status = OK, body = SaveKeywordsResult), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn save_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<SaveKeywordsInput>,
) -> ApiResult<Json<SaveKeywordsResult>> {
    Ok(Json(
        domain::keywords::save(&state.pool, &actor, project_id, input).await?,
    ))
}

#[utoipa::path(patch, path = "/projects/{project_id}/saved-keywords/{id}", tag = "keywords",
    params(("project_id" = Uuid, Path), ("id" = Uuid, Path)), request_body = UpdateSavedInput,
    responses((status = OK, body = SavedKeyword), (status = NOT_FOUND, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn update_saved_keyword(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateSavedInput>,
) -> ApiResult<Json<SavedKeyword>> {
    Ok(Json(
        domain::keywords::update_saved(&state.pool, &actor, project_id, id, input).await?,
    ))
}

#[utoipa::path(post, path = "/projects/{project_id}/saved-keywords/delete", tag = "keywords",
    params(("project_id" = Uuid, Path)), request_body = DeleteSavedInput,
    responses((status = OK, body = DeleteSavedResult)), security(("session" = []), ("bearer" = [])))]
async fn delete_saved_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<DeleteSavedInput>,
) -> ApiResult<Json<DeleteSavedResult>> {
    Ok(Json(
        domain::keywords::delete_saved(&state.pool, &actor, project_id, input).await?,
    ))
}

/// Tags in use on this project's saved keywords.
#[utoipa::path(get, path = "/projects/{project_id}/saved-keywords/tags", tag = "keywords",
    params(("project_id" = Uuid, Path)), responses((status = OK, body = Vec<String>)),
    security(("session" = []), ("bearer" = [])))]
async fn list_saved_keyword_tags(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(
        domain::keywords::saved_tags(&state.pool, &actor, project_id).await?,
    ))
}

/// Saved keywords as CSV. Empty cells mean "unknown".
#[utoipa::path(get, path = "/projects/{project_id}/saved-keywords.csv", tag = "keywords",
    params(("project_id" = Uuid, Path)),
    responses((status = OK, content_type = "text/csv", body = String)),
    security(("session" = []), ("bearer" = [])))]
async fn export_saved_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
) -> ApiResult<Response> {
    let mut csv = String::from(CSV_HEAD);
    let mut offset = 0;
    loop {
        let page = domain::keywords::saved(
            &state.pool,
            &actor,
            project_id,
            SavedQuery {
                limit: Some(1000),
                offset: Some(offset),
                ..Default::default()
            },
        )
        .await?;
        for k in &page.items {
            csv.push_str(
                &[
                    csv_cell(&k.keyword),
                    opt(k.popularity),
                    opt(k.est_volume),
                    csv_cell(k.volume_bucket.as_deref().unwrap_or("")),
                    csv_cell(k.volume_source.as_deref().unwrap_or("")),
                    csv_cell(k.volume_confidence.as_deref().unwrap_or("")),
                    opt(k.kd),
                    csv_cell(k.intent.as_deref().unwrap_or("")),
                    csv_cell(&k.serp_features.clone().unwrap_or_default().join(" ")),
                    csv_cell(&k.tags.join(" ")),
                ]
                .join(","),
            );
            csv.push('\n');
        }
        offset += page.items.len() as i64;
        if page.items.is_empty() || offset >= page.total {
            break;
        }
    }
    Ok(csv_response(
        format!("saved-keywords-{project_id}.csv"),
        csv,
    ))
}

/// Results page of a keyword with each domain's authority, SERP features, related searches,
/// difficulty and intent. Cached pages are reused for 7 days unless `refresh` is set.
#[utoipa::path(post, path = "/projects/{project_id}/serp-analysis", tag = "keywords",
    params(("project_id" = Uuid, Path)), request_body = SerpAnalysisInput,
    responses((status = OK, body = SerpAnalysis), (status = CONFLICT, body = Problem),
              (status = SERVICE_UNAVAILABLE, body = Problem), (status = UNPROCESSABLE_ENTITY, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn analyze_serp(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<SerpAnalysisInput>,
) -> ApiResult<Json<SerpAnalysis>> {
    Ok(Json(
        domain::keywords::serp_analysis(&state.pool, &actor, &state.providers, project_id, input)
            .await?,
    ))
}

#[utoipa::path(post, path = "/projects/{project_id}/saved-keywords/refresh", tag = "keywords",
    params(("project_id" = Uuid, Path)), request_body = domain::keywords::RefreshSavedInput,
    responses((status = ACCEPTED, body = db::models::Job), (status = CONFLICT, body = Problem)),
    security(("session" = []), ("bearer" = [])))]
async fn refresh_saved_keywords(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(project_id): Path<Uuid>,
    Json(input): Json<domain::keywords::RefreshSavedInput>,
) -> ApiResult<(StatusCode, Json<db::models::Job>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(
            domain::keywords::refresh_saved(
                &state.pool,
                &actor,
                &state.providers,
                project_id,
                input,
            )
            .await?,
        ),
    ))
}
