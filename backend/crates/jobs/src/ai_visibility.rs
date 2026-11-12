use crate::JobCtx;
use common::{AppError, AppResult, Config};
use db::ai_visibility::{self as repo, NewAiAnswer};
use providers::ai_engines::{EngineGateway, NativeEngine};
use providers::ai_visibility::{ModelGateway, mentioned, sources};
use serde_json::json;
use std::collections::BTreeSet;

pub fn gateway(cfg: &Config) -> AppResult<EngineGateway> {
    let mut models: Vec<String> = cfg
        .ai_visibility_models
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if models.is_empty() {
        models = cfg
            .llm_model
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    let compatible = match cfg
        .openai_compat_base_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        Some(base) if !models.is_empty() => Some(
            ModelGateway::new(base, cfg.openai_compat_api_key.clone(), models)
                .map_err(crate::provider_ctx::provider_err)?,
        ),
        _ => None,
    };
    let mut native = Vec::new();
    for model in cfg
        .ai_ollama_models
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        native.push(
            NativeEngine::new("ollama", model, &cfg.ai_ollama_url, None)
                .map_err(crate::provider_ctx::provider_err)?,
        );
    }
    if let Some(model) = cfg
        .ai_anthropic_model
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        native.push(
            NativeEngine::new(
                "anthropic",
                model,
                &cfg.ai_anthropic_url,
                cfg.ai_anthropic_api_key.clone(),
            )
            .map_err(crate::provider_ctx::provider_err)?,
        );
    }
    if let Some(model) = cfg
        .ai_gemini_model
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        native.push(
            NativeEngine::new(
                "gemini",
                model,
                &cfg.ai_gemini_url,
                cfg.ai_gemini_api_key.clone(),
            )
            .map_err(crate::provider_ctx::provider_err)?,
        );
    }
    EngineGateway::new(compatible, native).map_err(crate::provider_ctx::provider_err)
}
pub fn gateway_id(gateway: &EngineGateway) -> String {
    common::token::sha256_hex(gateway.identity())
}

pub fn register(registry: crate::Registry, cfg: Config) -> crate::Registry {
    registry.register("ai_visibility", move |ctx: JobCtx| {
        let cfg = cfg.clone();
        async move { run(ctx, &cfg).await }
    })
}
pub async fn schedule_due(pool: &sqlx::PgPool, cfg: &Config) -> AppResult<usize> {
    let due = repo::due(pool).await?;
    let mut started = 0;
    for tracker in due {
        let Some(expected) = tracker.next_run_at else {
            continue;
        };
        let gateway = match gateway(cfg) {
            Ok(g) => g,
            Err(error) => {
                repo::pause_schedule(pool, tracker.id, expected, &error.to_string()).await?;
                continue;
            }
        };
        if tracker
            .config
            .models
            .iter()
            .any(|m| !gateway.models().contains(m))
        {
            repo::pause_schedule(pool,tracker.id,expected,"Selected engine/model is no longer approved; create a tracker with current engines").await?;
            continue;
        }
        let Some(next) = crate::rank_check::next_run(
            &tracker.schedule,
            tracker.hour_utc as u32,
            tracker.weekday as u32,
            chrono::Utc::now(),
        ) else {
            continue;
        };
        let org = repo::project_org(pool, tracker.project_id).await?;
        match repo::start_scheduled(
            pool,
            org,
            tracker.project_id,
            tracker.id,
            &tracker.domain,
            &gateway_id(&gateway),
            repo::ScheduleClaim { expected, next },
        )
        .await
        {
            Ok(Some(_)) => started += 1,
            Ok(None) => {}
            Err(error) => {
                repo::pause_schedule(pool, tracker.id, expected, &error.to_string()).await?;
            }
        }
    }
    Ok(started)
}
async fn run(ctx: JobCtx, cfg: &Config) -> anyhow::Result<()> {
    let id = ctx.job.payload["run_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing run id"))?
        .parse()?;
    let run = repo::run_any(&ctx.pool, id).await?;
    if matches!(run.status.as_str(), "completed" | "failed") {
        return Ok(());
    }
    let result = collect(&ctx, cfg, &run).await;
    match result {
        Ok(()) => repo::status(&ctx.pool, id, "completed", None).await?,
        Err(AppError::Internal(error)) => return Err(error),
        Err(error) => repo::status(&ctx.pool, id, "failed", Some(&error.to_string())).await?,
    }
    Ok(())
}
async fn collect(ctx: &JobCtx, cfg: &Config, run: &repo::AiVisibilityRun) -> AppResult<()> {
    let org = ctx
        .job
        .organization_id
        .ok_or_else(|| AppError::Conflict("AI job has no workspace".into()))?;
    let pid = ctx
        .job
        .project_id
        .ok_or_else(|| AppError::Conflict("AI job has no project".into()))?;
    let gateway = gateway(cfg)?;
    if gateway_id(&gateway) != run.gateway_id {
        return Err(AppError::Conflict(
            "Model gateway changed after this run was queued; start a new run".into(),
        ));
    }
    for model in &run.config.models {
        if !gateway.models().contains(model) {
            return Err(AppError::Conflict(
                "A selected model is no longer approved by the operator".into(),
            ));
        }
    }
    repo::current(&ctx.pool, org, pid, run).await?;
    let existing = repo::answers(&ctx.pool, run.id).await?;
    if existing.iter().any(|a| a.error.is_some()) {
        return Err(AppError::Unavailable(
            "A previous model request failed; saved answers are preserved".into(),
        ));
    }
    let done: BTreeSet<_> = existing
        .iter()
        .map(|r| (r.prompt_index, r.model.clone()))
        .collect();
    repo::status(&ctx.pool, run.id, "running", None).await?;
    let total = run.config.prompts.len() * run.config.models.len();
    let mut completed = done.len();
    for (index, prompt) in run.config.prompts.iter().enumerate() {
        for model in &run.config.models {
            if done.contains(&(index as i32, model.clone())) {
                continue;
            }
            repo::current(&ctx.pool, org, pid, run).await?;
            ctx.progress(json!({"completed":completed,"total":total}))
                .await?;
            match gateway
                .answer(model, prompt, &run.config.country, &run.config.language)
                .await
            {
                Ok(answer) => {
                    repo::current(&ctx.pool, org, pid, run).await?;
                    let matches: Vec<_> = run
                        .config
                        .brands
                        .iter()
                        .filter(|brand| mentioned(&answer.text, brand))
                        .collect();
                    repo::save_answer(
                        &ctx.pool,
                        NewAiAnswer {
                            run_id: run.id,
                            prompt_index: index as i32,
                            model,
                            answer: Some(&answer.text),
                            mentions: Some(json!(matches)),
                            sources: Some(json!(sources(&answer.text))),
                            engine: answer.engine,
                            grounding: answer.grounding,
                            citations: json!(answer.citations),
                            error: None,
                        },
                    )
                    .await?;
                }
                Err(error) => {
                    let message = error.to_string();
                    repo::save_answer(
                        &ctx.pool,
                        NewAiAnswer {
                            run_id: run.id,
                            prompt_index: index as i32,
                            model,
                            answer: None,
                            mentions: None,
                            sources: None,
                            error: Some(&message),
                            engine: gateway.engine(model),
                            grounding: "none",
                            citations: json!([]),
                        },
                    )
                    .await?;
                    return Err(crate::provider_ctx::provider_err(error));
                }
            }
            completed += 1;
        }
    }
    ctx.progress(json!({"completed":completed,"total":total}))
        .await?;
    Ok(())
}
