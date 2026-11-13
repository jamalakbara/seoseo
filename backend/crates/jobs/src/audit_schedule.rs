use common::{AppResult, Config};
use sqlx::PgPool;
use url::Url;

pub async fn schedule_due(pool: &PgPool, cfg: &Config) -> AppResult<usize> {
    let mut started = 0;
    for schedule in db::audit_schedule::due(pool).await? {
        let Some(next) = crate::rank_check::next_run(
            &schedule.schedule,
            schedule.hour_utc as u32,
            schedule.weekday as u32,
            chrono::Utc::now(),
        ) else {
            continue;
        };
        let policy = crawler::UrlPolicy {
            allow_private: cfg.crawler_allow_private,
        };
        let mut error = Url::parse(&schedule.start_url)
            .map_err(|e| e.to_string())
            .and_then(|u| policy.check_url(&u).map_err(|e| e.to_string()))
            .err();
        let config =
            serde_json::from_value::<crate::site_audit::AuditConfig>(schedule.config.clone());
        match config {
            Ok(c)
                if (c.render_js || c.lighthouse)
                    && cfg
                        .renderer_url
                        .as_deref()
                        .is_none_or(|s| s.trim().is_empty()) =>
            {
                error =
                    Some("Renderer unavailable; review the schedule after configuring it".into())
            }
            Err(_) => {
                error =
                    Some("Stored crawl settings are invalid; save a new baseline schedule".into())
            }
            _ => {}
        }
        if db::audit_schedule::enqueue_due(pool, &schedule, next, error)
            .await?
            .is_some()
        {
            started += 1;
        }
    }
    Ok(started)
}
