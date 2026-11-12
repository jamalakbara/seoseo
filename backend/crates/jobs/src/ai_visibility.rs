use crate::JobCtx;
use common::{AppError, AppResult, Config};
use db::ai_visibility::{self as repo, NewAiAnswer};
use providers::ai_visibility::{ModelGateway, mentioned, sources};
use serde_json::json;
use std::collections::BTreeSet;

pub fn gateway(cfg: &Config) -> AppResult<ModelGateway> {
    let base=cfg.openai_compat_base_url.as_deref().filter(|s|!s.trim().is_empty())
        .ok_or_else(||AppError::Conflict("Configure OPENAI_COMPAT_BASE_URL and AI_VISIBILITY_MODELS (or LLM_MODEL), for example a local Ollama /v1 endpoint".into()))?;
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
    ModelGateway::new(base, cfg.openai_compat_api_key.clone(), models)
        .map_err(crate::provider_ctx::provider_err)
}
pub fn gateway_id(gateway: &ModelGateway) -> String {
    common::token::sha256_hex(gateway.identity())
}

pub fn register(registry: crate::Registry, cfg: Config) -> crate::Registry {
    registry.register("ai_visibility", move |ctx: JobCtx| {
        let cfg = cfg.clone();
        async move { run(ctx, &cfg).await }
    })
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
                        .filter(|brand| mentioned(&answer, brand))
                        .collect();
                    repo::save_answer(
                        &ctx.pool,
                        NewAiAnswer {
                            run_id: run.id,
                            prompt_index: index as i32,
                            model,
                            answer: Some(&answer),
                            mentions: Some(json!(matches)),
                            sources: Some(json!(sources(&answer))),
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
