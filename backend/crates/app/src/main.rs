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
            let state = api::AppState::new(pool, valkey, cfg, metrics);
            api::serve(state, shutdown_signal()).await?;
        }
        Command::Worker { cfg, concurrency } => {
            let pool = db::connect(&cfg.database_url, cfg.db_max_connections).await?;
            let (tx, rx) = watch::channel(false);
            tokio::spawn(async move {
                shutdown_signal().await;
                let _ = tx.send(true);
            });
            jobs::run_worker(pool, jobs::default_registry(), concurrency, rx).await?;
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
