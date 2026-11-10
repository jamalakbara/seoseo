//! Background worker: claims jobs from the Postgres queue, runs handlers, keeps leases alive,
//! and reaps jobs whose worker died. Job kinds are registered in [`Registry`].

use common::{AppResult, new_id};
use db::models::Job;
use sqlx::PgPool;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use uuid::Uuid;

pub const LEASE_SECS: i64 = 120;
const HEARTBEAT_EVERY: Duration = Duration::from_secs(30);
const IDLE_POLL: Duration = Duration::from_secs(2);
const REAP_EVERY: Duration = Duration::from_secs(60);

/// Context handed to a running job.
#[derive(Clone)]
pub struct JobCtx {
    pub pool: PgPool,
    pub job: Job,
    worker_id: Arc<str>,
}

impl JobCtx {
    /// Persist progress (shown in the UI) and extend the lease.
    pub async fn progress(&self, value: serde_json::Value) -> AppResult<()> {
        db::jobs::heartbeat(
            &self.pool,
            self.job.id,
            &self.worker_id,
            LEASE_SECS,
            Some(value),
        )
        .await
    }
}

type HandlerFut = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;
type Handler = Arc<dyn Fn(JobCtx) -> HandlerFut + Send + Sync>;

#[derive(Default, Clone)]
pub struct Registry {
    handlers: HashMap<&'static str, Handler>,
}

impl Registry {
    pub fn register<F, Fut>(mut self, kind: &'static str, f: F) -> Self
    where
        F: Fn(JobCtx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.handlers
            .insert(kind, Arc::new(move |ctx| Box::pin(f(ctx))));
        self
    }

    pub fn kinds(&self) -> Vec<&'static str> {
        self.handlers.keys().copied().collect()
    }
}

/// Built-in job kinds available in every build.
pub fn default_registry() -> Registry {
    Registry::default().register("noop", |ctx: JobCtx| async move {
        ctx.progress(serde_json::json!({ "message": "done" }))
            .await?;
        Ok(())
    })
}

/// Enqueue a job for an organization (and optionally a project).
pub async fn enqueue(
    pool: &PgPool,
    kind: &str,
    org_id: Option<Uuid>,
    project_id: Option<Uuid>,
    payload: serde_json::Value,
) -> AppResult<Job> {
    db::jobs::enqueue(pool, new_id(), kind, org_id, project_id, payload, 3).await
}

/// Run the worker loop until `shutdown` flips to true.
pub async fn run_worker(
    pool: PgPool,
    registry: Registry,
    concurrency: usize,
    mut shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let worker_id: Arc<str> = format!("worker-{}", new_id()).into();
    let permits = Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));
    let registry = Arc::new(registry);
    tracing::info!(worker = %worker_id, concurrency, kinds = ?registry.kinds(), "worker started");

    let reaper = {
        let pool = pool.clone();
        let mut shutdown = shutdown.clone();
        tokio::spawn(async move {
            loop {
                match db::jobs::reap_expired(&pool).await {
                    Ok(n) if n > 0 => tracing::warn!(count = n, "reaped jobs with expired leases"),
                    Err(e) => tracing::error!(error = %e, "reaper failed"),
                    _ => {}
                }
                if let Err(e) = db::sessions::delete_expired(&pool).await {
                    tracing::error!(error = %e, "session cleanup failed");
                }
                tokio::select! {
                    _ = tokio::time::sleep(REAP_EVERY) => {}
                    _ = shutdown.changed() => break,
                }
            }
        })
    };

    loop {
        if *shutdown.borrow() {
            break;
        }
        let permit = permits.clone().acquire_owned().await?;
        let claimed = match db::jobs::claim(&pool, &worker_id, LEASE_SECS).await {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(error = %e, "claim failed");
                None
            }
        };
        let Some(job) = claimed else {
            drop(permit);
            tokio::select! {
                _ = tokio::time::sleep(IDLE_POLL) => continue,
                _ = shutdown.changed() => break,
            }
        };
        let ctx = JobCtx {
            pool: pool.clone(),
            job,
            worker_id: worker_id.clone(),
        };
        let registry = registry.clone();
        tokio::spawn(async move {
            run_one(ctx, &registry).await;
            drop(permit);
        });
    }

    tracing::info!("worker draining");
    let _ = permits.acquire_many(concurrency.max(1) as u32).await;
    reaper.abort();
    Ok(())
}

async fn run_one(ctx: JobCtx, registry: &Registry) {
    use tracing::Instrument;
    let span = tracing::info_span!("job", id = %ctx.job.id, kind = %ctx.job.kind, attempt = ctx.job.attempts);
    run_one_inner(ctx, registry).instrument(span).await
}

async fn run_one_inner(ctx: JobCtx, registry: &Registry) {
    let job_id = ctx.job.id;
    let kind = ctx.job.kind.clone();

    let Some(handler) = registry.handlers.get(kind.as_str()).cloned() else {
        let _ = db::jobs::fail(&ctx.pool, job_id, &format!("unknown job kind `{kind}`")).await;
        return;
    };

    // Heartbeat keeps the lease alive while the handler runs.
    let hb = {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(HEARTBEAT_EVERY).await;
                let _ =
                    db::jobs::heartbeat(&ctx.pool, ctx.job.id, &ctx.worker_id, LEASE_SECS, None)
                        .await;
            }
        })
    };
    let result = handler(ctx.clone()).await;
    hb.abort();

    let outcome = match result {
        Ok(()) => db::jobs::complete(&ctx.pool, job_id).await,
        Err(e) => {
            tracing::warn!(error = %e, "job failed");
            db::jobs::fail(&ctx.pool, job_id, &format!("{e:#}")).await
        }
    };
    if let Err(e) = outcome {
        tracing::error!(error = %e, "could not record job outcome");
    }
}
