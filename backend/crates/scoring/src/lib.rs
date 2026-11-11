//! Computed SEO metrics. Pure functions with documented formulas, so every number shown in
//! the product can be explained. Unknown inputs produce `None`, never a made-up value.

pub mod intent;

pub use intent::{Intent, classify_intent};

/// Canonical keyword form: lowercase, single spaces, no control characters.
pub fn normalize_keyword(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Host of an http(s) URL, lowercase, without port or credentials.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host_port
        .split(':')
        .next()?
        .trim_end_matches('.')
        .to_lowercase();
    (!host.is_empty() && host.contains('.')).then_some(host)
}

/// Registrable domain per the Public Suffix List: `news.bbc.co.uk` → `bbc.co.uk`.
pub fn registrable_domain(host: &str) -> Option<String> {
    let host = host.trim().trim_end_matches('.').to_lowercase();
    psl::domain_str(&host).map(str::to_string)
}

/// SeoSeo Rank (SR), 0–100, from a domain's position in the Common Crawl domain graph
/// ordered by harmonic centrality.
///
/// `SR = 100 × (1 − log10(rank) / log10(total))`: rank 1 scores 100, the last domain 0, and
/// each 10× drop in rank costs the same number of points (like other authority scores).
pub fn sr_from_rank(rank: i64, total: i64) -> Option<i32> {
    if rank < 1 || total < 2 {
        return None;
    }
    let rank = rank.min(total) as f64;
    let sr = 100.0 * (1.0 - rank.log10() / (total as f64).log10());
    Some(sr.round().clamp(0.0, 100.0) as i32)
}

/// SR assumed for a ranking domain that is outside the imported top list: it is known to be
/// weaker than every imported domain, so we use half the weakest imported score.
pub fn sr_floor(weakest_imported: i32) -> i32 {
    (weakest_imported / 2).max(1)
}

/// Share of clicks by organic position 1–10 (rounded industry curve); used as weights.
const POSITION_WEIGHT: [f64; 10] = [0.30, 0.16, 0.11, 0.08, 0.07, 0.06, 0.05, 0.04, 0.035, 0.03];

/// Keyword difficulty, 0–100: the click-weighted average SR of the domains ranking in the
/// top 10. Strong sites in the top positions make a keyword hard.
///
/// `top` holds the SR of each result in rank order (`None` = outside the imported list,
/// scored as `floor`). Returns `None` with fewer than 5 results: too little evidence.
pub fn keyword_difficulty(top: &[Option<i32>], floor: i32) -> Option<i32> {
    let top = &top[..top.len().min(10)];
    if top.len() < 5 {
        return None;
    }
    let (mut sum, mut weight) = (0.0, 0.0);
    for (sr, w) in top.iter().zip(POSITION_WEIGHT) {
        sum += f64::from(sr.unwrap_or(floor)) * w;
        weight += w;
    }
    Some((sum / weight).round().clamp(0.0, 100.0) as i32)
}

/// How an idea was discovered in autocomplete.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SuggestSignal {
    /// Best (lowest) 0-based position in any suggestion list.
    pub best_rank: usize,
    /// Kind of the query that produced the best position: `seed`, `alphabet`, `question`,
    /// `preposition` or `related`.
    pub best_kind: String,
    /// Number of distinct engines that suggested it.
    pub engines: usize,
    /// Number of distinct expansion queries that returned it.
    pub hits: usize,
}

/// Relative popularity, 0–100, **within one research run**. Not a search volume.
///
/// Suggestions for the bare seed are the most popular completions and score highest;
/// ideas that only appear after adding a letter or modifier score lower. Agreement between
/// engines and repeated appearances add a little.
pub fn popularity(s: &SuggestSignal) -> i32 {
    let rank = s.best_rank.min(15) as i32;
    let base = match s.best_kind.as_str() {
        "seed" => 96 - rank * 4,
        "alphabet" => 60 - rank * 3,
        "question" | "preposition" => 56 - rank * 3,
        _ => 45,
    };
    let agree = if s.engines >= 2 { 8 } else { 0 };
    let repeat = (s.hits.saturating_sub(1).min(3) as i32) * 3;
    (base + agree + repeat).clamp(1, 100)
}

/// Display bucket for an estimated monthly volume. Buckets avoid fake precision.
pub fn volume_bucket(volume: i64) -> &'static str {
    match volume {
        i64::MIN..=9 => "0-10",
        10..=99 => "10-100",
        100..=999 => "100-1K",
        1_000..=9_999 => "1K-10K",
        10_000..=99_999 => "10K-100K",
        100_000..=999_999 => "100K-1M",
        _ => "1M+",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_keywords_and_hosts() {
        assert_eq!(normalize_keyword("  Best\tCRM \n Tools "), "best crm tools");
        assert_eq!(
            host_of("https://user:pw@News.BBC.co.uk:443/a?b#c").as_deref(),
            Some("news.bbc.co.uk")
        );
        assert_eq!(host_of("ftp://x.com"), None);
        assert_eq!(host_of("https://localhost/"), None);
        assert_eq!(
            registrable_domain("news.bbc.co.uk").as_deref(),
            Some("bbc.co.uk")
        );
        assert_eq!(
            registrable_domain("WWW.Example.com.").as_deref(),
            Some("example.com")
        );
        assert_eq!(registrable_domain("co.uk"), None);
    }

    #[test]
    fn sr_is_log_scaled() {
        let total = 134_979_706;
        assert_eq!(sr_from_rank(1, total), Some(100));
        assert_eq!(sr_from_rank(total, total), Some(0));
        let (a, b, c) = (
            sr_from_rank(100, total).unwrap(),
            sr_from_rank(1_000, total).unwrap(),
            sr_from_rank(10_000, total).unwrap(),
        );
        assert!(a > b && b > c);
        assert!(((a - b) - (b - c)).abs() <= 1, "equal steps per decade");
        assert_eq!(sr_from_rank(0, total), None);
        assert_eq!(sr_floor(26), 13);
    }

    #[test]
    fn kd_weights_top_positions() {
        assert_eq!(keyword_difficulty(&[Some(90); 4], 10), None);
        assert_eq!(keyword_difficulty(&[Some(80); 10], 10), Some(80));
        let strong_top = [Some(95), Some(90), Some(20), Some(20), Some(20), Some(20)];
        let strong_bottom = [Some(20), Some(20), Some(20), Some(20), Some(90), Some(95)];
        assert!(
            keyword_difficulty(&strong_top, 10).unwrap()
                > keyword_difficulty(&strong_bottom, 10).unwrap()
        );
        assert_eq!(keyword_difficulty(&[None; 10], 13), Some(13));
    }

    #[test]
    fn popularity_orders_signals() {
        let sig = |rank, kind: &str, engines, hits| SuggestSignal {
            best_rank: rank,
            best_kind: kind.into(),
            engines,
            hits,
        };
        let top = popularity(&sig(0, "seed", 2, 3));
        let mid = popularity(&sig(4, "seed", 1, 1));
        let low = popularity(&sig(2, "alphabet", 1, 1));
        assert_eq!(top, 100);
        assert!(top > mid && mid > low);
        assert!((1..=100).contains(&popularity(&sig(99, "question", 1, 1))));
    }

    #[test]
    fn buckets() {
        assert_eq!(volume_bucket(0), "0-10");
        assert_eq!(volume_bucket(99), "10-100");
        assert_eq!(volume_bucket(1_500), "1K-10K");
        assert_eq!(volume_bucket(5_000_000), "1M+");
    }
}
