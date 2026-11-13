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
    /// Optional free Chrome UX Report API key; never returned to clients.
    #[arg(long, env = "CRUX_API_KEY", hide_env_values = true)]
    pub crux_api_key: Option<String>,
    /// Operator-only override for integration fixtures or an approved proxy.
    #[arg(
        long,
        env = "CRUX_API_URL",
        default_value = "https://chromeuxreport.googleapis.com/v1/records:queryRecord"
    )]
    pub crux_api_url: String,
    /// Operator-only Bing Webmaster JSON API base (override for fixtures/proxies).
    #[arg(
        long,
        env = "BING_WEBMASTER_API_URL",
        default_value = "https://www.bing.com/webmaster/api.svc/json"
    )]
    pub bing_webmaster_api_url: String,
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

    /// Base64 of 32 random bytes. Encrypts provider API keys at rest; without it,
    /// bring-your-own-key providers cannot be configured. `openssl rand -base64 32`
    #[arg(long, env = "APP_ENCRYPTION_KEY", hide_env_values = true)]
    pub app_encryption_key: Option<String>,

    /// Base URL of a SearXNG instance with the JSON format enabled (compose profile `serp`).
    /// Operator-only on purpose: it usually points at an internal host.
    #[arg(long, env = "SEARXNG_URL")]
    pub searxng_url: Option<String>,

    /// Internal isolated browser service (compose profile renderer).
    #[arg(long, env = "RENDERER_URL")]
    pub renderer_url: Option<String>,

    /// Override the Common Crawl web-graph release list (mirrors, tests).
    #[arg(long, env = "CC_GRAPHINFO_URL")]
    pub cc_graphinfo_url: Option<String>,

    #[arg(long, env = "GOOGLE_CLIENT_ID")]
    pub google_client_id: Option<String>,
    #[arg(long, env = "GOOGLE_CLIENT_SECRET", hide_env_values = true)]
    pub google_client_secret: Option<String>,
    /// Operator-only endpoint overrides for local integration fixtures.
    #[arg(
        long,
        env = "GOOGLE_AUTH_URL",
        default_value = "https://accounts.google.com/o/oauth2/v2/auth"
    )]
    pub google_auth_url: String,
    #[arg(
        long,
        env = "GOOGLE_TOKEN_URL",
        default_value = "https://oauth2.googleapis.com/token"
    )]
    pub google_token_url: String,
    #[arg(
        long,
        env = "GSC_API_URL",
        default_value = "https://www.googleapis.com/webmasters/v3"
    )]
    pub gsc_api_url: String,
    #[arg(
        long,
        env = "GSC_INSPECTION_URL",
        default_value = "https://searchconsole.googleapis.com/v1/urlInspection/index:inspect"
    )]
    pub gsc_inspection_url: String,

    #[arg(
        long,
        env = "GA4_ADMIN_URL",
        default_value = "https://analyticsadmin.googleapis.com/v1beta"
    )]
    pub ga4_admin_url: String,
    #[arg(
        long,
        env = "GA4_DATA_URL",
        default_value = "https://analyticsdata.googleapis.com/v1beta"
    )]
    pub ga4_data_url: String,
    #[arg(
        long,
        env = "INDEXNOW_URL",
        default_value = "https://api.indexnow.org/indexnow"
    )]
    pub indexnow_url: String,
    /// Operator-controlled local/OpenAI-compatible model endpoint; never set from project input.
    #[arg(long, env = "OPENAI_COMPAT_BASE_URL")]
    pub openai_compat_base_url: Option<String>,
    #[arg(long, env = "OPENAI_COMPAT_API_KEY", hide_env_values = true)]
    pub openai_compat_api_key: Option<String>,
    #[arg(long, env = "LLM_MODEL")]
    pub llm_model: Option<String>,
    /// Operator-approved models for manual AI visibility comparisons; defaults to LLM_MODEL.
    #[arg(long, env = "AI_VISIBILITY_MODELS", value_delimiter = ',')]
    pub ai_visibility_models: Vec<String>,
    #[arg(long, env = "AI_OLLAMA_URL", default_value = "http://ollama:11434/v1")]
    pub ai_ollama_url: String,
    #[arg(long, env = "AI_OLLAMA_MODELS", value_delimiter = ',')]
    pub ai_ollama_models: Vec<String>,
    #[arg(long, env = "AI_ANTHROPIC_MODEL")]
    pub ai_anthropic_model: Option<String>,
    #[arg(long, env = "AI_ANTHROPIC_API_KEY", hide_env_values = true)]
    pub ai_anthropic_api_key: Option<String>,
    #[arg(
        long,
        env = "AI_ANTHROPIC_URL",
        default_value = "https://api.anthropic.com/v1"
    )]
    pub ai_anthropic_url: String,
    #[arg(long, env = "AI_GEMINI_MODEL")]
    pub ai_gemini_model: Option<String>,
    #[arg(long, env = "AI_GEMINI_API_KEY", hide_env_values = true)]
    pub ai_gemini_api_key: Option<String>,
    #[arg(
        long,
        env = "AI_GEMINI_URL",
        default_value = "https://generativelanguage.googleapis.com/v1beta"
    )]
    pub ai_gemini_url: String,
}
