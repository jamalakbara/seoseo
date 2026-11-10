//! robots.txt: fetch once per host, answer "may we crawl this URL", list sitemaps.

use crate::fetch::Fetcher;
use texting_robots::Robot;
use url::Url;

pub const MAX_ROBOTS_BYTES: usize = 500 * 1024;

pub struct Robots {
    robot: Option<Robot>,
    pub sitemaps: Vec<String>,
    /// Status of the robots.txt fetch (None = network error).
    pub status: Option<u16>,
    pub crawl_delay_secs: Option<f32>,
}

impl Robots {
    /// Missing (4xx) or unreachable robots.txt means "everything allowed", like search engines do.
    pub async fn fetch(fetcher: &Fetcher, origin: &Url, agent: &str) -> Self {
        let Ok(url) = origin.join("/robots.txt") else {
            return Self::allow_all(None);
        };
        match fetcher.get_bytes(&url, MAX_ROBOTS_BYTES).await {
            Ok((status, body)) if (200..300).contains(&status) => {
                Self::parse(agent, &body, Some(status))
            }
            Ok((status, _)) => Self::allow_all(Some(status)),
            Err(_) => Self::allow_all(None),
        }
    }

    pub fn parse(agent: &str, body: &[u8], status: Option<u16>) -> Self {
        match Robot::new(agent, body) {
            Ok(robot) => Self {
                sitemaps: robot.sitemaps.clone(),
                crawl_delay_secs: robot.delay,
                robot: Some(robot),
                status,
            },
            Err(_) => Self::allow_all(status),
        }
    }

    fn allow_all(status: Option<u16>) -> Self {
        Self {
            robot: None,
            sitemaps: Vec::new(),
            status,
            crawl_delay_secs: None,
        }
    }

    pub fn allowed(&self, url: &Url) -> bool {
        self.robot.as_ref().is_none_or(|r| r.allowed(url.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honours_agent_groups() {
        let txt = b"User-agent: *\nDisallow: /private\n\nUser-agent: SeoSeoBot\nDisallow: /no-bots\nCrawl-delay: 2\n\nSitemap: https://ex.com/sitemap.xml\n";
        let r = Robots::parse("SeoSeoBot", txt, Some(200));
        assert!(!r.allowed(&Url::parse("https://ex.com/no-bots/x").unwrap()));
        // A specific group replaces `*` entirely.
        assert!(r.allowed(&Url::parse("https://ex.com/private").unwrap()));
        assert_eq!(r.sitemaps, vec!["https://ex.com/sitemap.xml".to_string()]);
        assert_eq!(r.crawl_delay_secs, Some(2.0));
    }
}
