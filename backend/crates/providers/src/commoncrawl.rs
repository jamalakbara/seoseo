//! Common Crawl web graph: domain-level ranks (harmonic centrality + PageRank), published
//! roughly quarterly as a public dataset. The rank file is sorted by harmonic centrality, so
//! reading the first N lines of the stream yields the top N domains without downloading
//! the whole multi-gigabyte file.

use crate::ProviderError;
use crate::http::{bad_json, check, client, net_err, text};
use flate2::write::MultiGzDecoder;
use futures::StreamExt;
use serde_json::Value;
use std::io::Write;

pub const ID: &str = "authority_commoncrawl";
pub const GRAPHINFO_URL: &str = "https://index.commoncrawl.org/graphinfo.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub id: String,
    /// Number of domains in the graph (the worst possible rank).
    pub total_domains: i64,
}

impl Release {
    pub fn ranks_url(&self) -> String {
        format!(
            "https://data.commoncrawl.org/projects/hyperlinkgraph/{id}/domain/{id}-domain-ranks.txt.gz",
            id = self.id
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankRow {
    pub domain: String,
    pub harmonic_rank: i64,
    pub pagerank_rank: i64,
}

pub fn parse_graphinfo(body: &str) -> Result<Release, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(bad_json)?;
    let first = v
        .get(0)
        .ok_or_else(|| ProviderError::BadResponse("no web graph releases listed".into()))?;
    let id = first
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| {
            !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .ok_or_else(|| ProviderError::BadResponse("release has no valid id".into()))?;
    let total = first
        .pointer("/stats/domain/nodes")
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .ok_or_else(|| ProviderError::BadResponse("release has no domain count".into()))?;
    Ok(Release {
        id: id.to_string(),
        total_domains: total,
    })
}

pub async fn latest_release(graphinfo_url: Option<&str>) -> Result<Release, ProviderError> {
    let res = client()
        .get(graphinfo_url.unwrap_or(GRAPHINFO_URL))
        .send()
        .await
        .map_err(net_err)?;
    parse_graphinfo(&text(check(res)?, 2 * 1024 * 1024).await?)
}

/// `com.example.shop` → `shop.example.com`
fn unreverse(host_rev: &str) -> String {
    host_rev.rsplit('.').collect::<Vec<_>>().join(".")
}

/// Parse one line: `harmonic_pos \t harmonic_val \t pr_pos \t pr_val \t host_rev \t n_hosts`.
pub fn parse_line(line: &str) -> Option<RankRow> {
    if line.starts_with('#') {
        return None;
    }
    let mut f = line.split('\t');
    let harmonic_rank = f.next()?.trim().parse().ok()?;
    f.next()?;
    let pagerank_rank = f.next()?.trim().parse().ok()?;
    f.next()?;
    let rev = f.next()?.trim();
    if rev.is_empty() || rev.len() > 253 {
        return None;
    }
    Some(RankRow {
        domain: unreverse(rev).to_ascii_lowercase(),
        harmonic_rank,
        pagerank_rank,
    })
}

/// Incremental reader over the gzip'd rank file.
pub struct RankStream {
    body: futures::stream::BoxStream<'static, reqwest::Result<Vec<u8>>>,
    gz: MultiGzDecoder<Vec<u8>>,
    tail: Vec<u8>,
    remaining: usize,
    done: bool,
}

impl RankStream {
    /// Start streaming and stop after `limit` domains.
    pub async fn open(url: &str, limit: usize) -> Result<Self, ProviderError> {
        let res = client()
            .get(url)
            // The body is already gzip; ask for it as-is.
            .header("Accept-Encoding", "identity")
            .timeout(std::time::Duration::from_secs(60 * 60))
            .send()
            .await
            .map_err(net_err)?;
        let res = check(res)?;
        Ok(Self {
            body: res.bytes_stream().map(|r| r.map(|b| b.to_vec())).boxed(),
            gz: MultiGzDecoder::new(Vec::new()),
            tail: Vec::new(),
            remaining: limit,
            done: false,
        })
    }

    /// Next batch of rows, or `None` when the limit or the end of the file is reached.
    pub async fn next_batch(&mut self) -> Result<Option<Vec<RankRow>>, ProviderError> {
        while !self.done && self.remaining > 0 {
            let Some(chunk) = self.body.next().await else {
                self.done = true;
                break;
            };
            let chunk = chunk.map_err(net_err)?;
            self.gz
                .write_all(&chunk)
                .map_err(|e| ProviderError::BadResponse(format!("corrupt gzip stream ({e})")))?;
            let mut data = std::mem::take(&mut self.tail);
            data.append(self.gz.get_mut());
            let rows = self.drain_lines(&mut data);
            self.tail = data;
            if !rows.is_empty() {
                return Ok(Some(rows));
            }
        }
        Ok(None)
    }

    fn drain_lines(&mut self, data: &mut Vec<u8>) -> Vec<RankRow> {
        let Some(last_nl) = data.iter().rposition(|b| *b == b'\n') else {
            return Vec::new();
        };
        let rest = data.split_off(last_nl + 1);
        let complete = std::mem::replace(data, rest);
        let mut rows = Vec::new();
        for line in String::from_utf8_lossy(&complete).lines() {
            if self.remaining == 0 {
                break;
            }
            if let Some(row) = parse_line(line) {
                rows.push(row);
                self.remaining -= 1;
            }
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_and_lines() {
        let info = r#"[{"id":"cc-main-2026-aug-sep-oct","stats":{"host":{"nodes":9},"domain":{"nodes":134979706,"arcs":1}}},{"id":"older"}]"#;
        let r = parse_graphinfo(info).unwrap();
        assert_eq!(r.total_domains, 134_979_706);
        assert!(r.ranks_url().ends_with(
            "/cc-main-2026-aug-sep-oct/domain/cc-main-2026-aug-sep-oct-domain-ranks.txt.gz"
        ));
        assert!(parse_graphinfo(r#"[{"id":"../evil","stats":{"domain":{"nodes":1}}}]"#).is_err());

        assert!(parse_line("#harmonicc_pos\t#harmonicc_val\t#pr_pos").is_none());
        let row = parse_line("3\t2.9365982E7\t2\t0.0128\tcom.google\t18672").unwrap();
        assert_eq!(row.domain, "google.com");
        assert_eq!((row.harmonic_rank, row.pagerank_rank), (3, 2));
        assert_eq!(
            parse_line("9\t1.0\t7\t0.1\tuk.co.bbc\t1").unwrap().domain,
            "bbc.co.uk"
        );
        assert!(parse_line("garbage").is_none());
    }
}
