use clap::{Args, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AuthMode {
    /// Email + password accounts. The first visitor creates the admin account.
    Password,
    /// No sign-in: every request acts as a built-in admin. Only for trusted private networks.
    None,
}

/// Runtime configuration, read from CLI flags or environment variables.
#[derive(Debug, Clone, Args)]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "VALKEY_URL", default_value = "redis://127.0.0.1:6379")]
    pub valkey_url: String,

    #[arg(long, env = "BIND_ADDR", default_value = "0.0.0.0:8080")]
    pub bind_addr: String,

    /// Public URL of the web app (used for CORS and links).
    #[arg(long, env = "WEB_BASE_URL", default_value = "http://localhost:3000")]
    pub web_base_url: String,

    #[arg(long, env = "AUTH_MODE", value_enum, default_value = "password")]
    pub auth_mode: AuthMode,

    /// Set the `Secure` flag on session cookies (enable behind HTTPS).
    #[arg(long, env = "COOKIE_SECURE", default_value_t = false)]
    pub cookie_secure: bool,

    /// Session lifetime in days.
    #[arg(long, env = "SESSION_TTL_DAYS", default_value_t = 30)]
    pub session_ttl_days: i64,

    /// Allow sign-ups after the first (admin) account exists.
    #[arg(long, env = "ALLOW_SIGNUP", default_value_t = false)]
    pub allow_signup: bool,

    #[arg(long, env = "DB_MAX_CONNECTIONS", default_value_t = 20)]
    pub db_max_connections: u32,

    /// Allow audits of private/loopback hosts. Only for local testing.
    #[arg(long, env = "CRAWLER_ALLOW_PRIVATE", default_value_t = false)]
    pub crawler_allow_private: bool,

    /// Override the crawler User-Agent header.
    #[arg(long, env = "CRAWLER_USER_AGENT")]
    pub crawler_user_agent: Option<String>,
}
