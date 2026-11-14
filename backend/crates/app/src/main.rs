//! `seoseo` — single binary for the API server, background worker, migrations and admin tasks.

use clap::{Parser, Subcommand};
use common::Config;
use tokio::sync::watch;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "seoseo", version, about = "Free, open-source SEO platform")]
struct Cli {
    /// Log format: `pretty` for development, `json` for production.
    #[arg(long, env = "LOG_FORMAT", default_value = "json", global = true)]
    log_format: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP API.
    Serve {
        #[command(flatten)]
        cfg: Config,
        /// Also run migrations on startup.
        #[arg(long, env = "MIGRATE_ON_START", default_value_t = false)]
        migrate: bool,
    },
    /// Run the background job worker.
    Worker {
        #[command(flatten)]
        cfg: Config,
        #[arg(long, env = "WORKER_CONCURRENCY", default_value_t = 8)]
        concurrency: usize,
    },
    /// Apply database migrations and exit.
    Migrate {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
    /// Print the OpenAPI document and exit.
    Openapi,
    /// Operator tasks.
    Admin {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[command(subcommand)]
        task: AdminTask,
    },
}

#[derive(Subcommand)]
enum AdminTask {
    /// Set a user's password (e.g. when the admin forgot it).
    ResetPassword {
        #[arg(long)]
        email: String,
        #[arg(long, env = "NEW_PASSWORD")]
        password: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    init_tracing(&cli.log_format);

    match cli.command {
        Command::Serve { cfg, migrate } => {
            let pool = db::connect(&cfg.database_url, cfg.db_max_connections).await?;
            if migrate {
                db::migrate(&pool).await?;
            }
            let valkey = connect_valkey(&cfg.valkey_url).await;
            let metrics = metrics_exporter_prometheus::PrometheusBuilder::new()
                .install_recorder()
                .map_err(|e| tracing::warn!(error = %e, "metrics recorder not installed"))
                .ok();
            let pctx = jobs::ProviderCtx::from_config(&cfg)?;
            let state = api::AppState::new(pool, valkey, cfg, metrics, pctx);
            api::serve(state, shutdown_signal()).await?;
        }
        Command::Worker { cfg, concurrency } => {
            let pool = db::connect(&cfg.database_url, cfg.db_max_connections).await?;
            let (tx, rx) = watch::channel(false);
            tokio::spawn(async move {
                shutdown_signal().await;
                let _ = tx.send(true);
            });
            let pctx = jobs::ProviderCtx::from_config(&cfg)?;
            let registry = jobs::site_audit::register(
                jobs::default_registry(),
                jobs::site_audit::CrawlSettings {
                    user_agent: cfg.crawler_user_agent.clone(),
                    allow_private: cfg.crawler_allow_private,
                    renderer_url: cfg.renderer_url.clone().filter(|s| !s.is_empty()),
                },
            );
            let registry = jobs::keyword_research::register(registry, pctx.clone());
            let registry = jobs::saved_refresh::register(registry, pctx.clone());
            let registry = jobs::rank_check::register(registry, pctx.clone());
            let registry = jobs::backlink_discovery::register(registry, cfg.clone(), pctx.clone());
            let registry = jobs::cc_ranks::register(registry, pctx);
            let registry = jobs::ai_visibility::register(registry, cfg.clone());
            let sched_cfg = cfg.clone();
            let sched_pool = pool.clone();
            let mut sched_rx = rx.clone();
            let scheduler = tokio::spawn(async move {
                loop {
                    if *sched_rx.borrow() {
                        break;
                    }
                    if let Err(error) =
                        jobs::ai_visibility::schedule_due(&sched_pool, &sched_cfg).await
                    {
                        tracing::error!(%error,"AI scheduler failed");
                    }
                    if let Err(error) =
                        jobs::audit_schedule::schedule_due(&sched_pool, &sched_cfg).await
                    {
                        tracing::error!(%error,"audit scheduler failed");
                    }
                    tokio::select! {_=tokio::time::sleep(std::time::Duration::from_secs(60))=>{},_=sched_rx.changed()=>break,}
                }
            });
            let registry = domain::local_scans::register(registry, cfg.clone());
            let registry = domain::dashboard::register(registry, cfg);
            let result = jobs::run_worker(pool, registry, concurrency, rx).await;
            scheduler.abort();
            result?;
        }
        Command::Migrate { database_url } => {
            let pool = db::connect(&database_url, 2).await?;
            db::migrate(&pool).await?;
            tracing::info!("migrations applied");
        }
        Command::Openapi => println!("{}", api::openapi_json()),
        Command::Admin { database_url, task } => {
            let pool = db::connect(&database_url, 2).await?;
            match task {
                AdminTask::ResetPassword { email, password } => {
                    domain::auth::admin_set_password(&pool, &email, &password)
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    println!("password updated for {email}");
                }
            }
        }
    }
    Ok(())
}

fn init_tracing(format: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    if format == "pretty" {
        builder.init();
    } else {
        builder.json().init();
    }
}

async fn connect_valkey(url: &str) -> Option<redis::aio::ConnectionManager> {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "invalid VALKEY_URL; continuing without cache");
            return None;
        }
    };
    match client.get_connection_manager().await {
        Ok(c) => Some(c),
        Err(e) => {
            tracing::warn!(error = %e, "valkey unavailable; continuing without cache");
            None
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
