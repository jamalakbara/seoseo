//! Autocomplete suggestions (keyword ideas). Public JSON endpoints, no key needed.

use crate::http::{bad_json, check, client, net_err, pace, text, url};
use crate::{Market, ProviderError};
use async_trait::async_trait;
use std::time::Duration;

const MAX_BODY: usize = 256 * 1024;

#[async_trait]
pub trait SuggestProvider: Send + Sync {
    fn id(&self) -> &'static str;
    /// Suggestions for `query` in rank order (most prominent first).
    async fn suggest(&self, query: &str, market: &Market) -> Result<Vec<String>, ProviderError>;
}

/// OpenSearch suggestion format shared by Google and Bing: `["query", ["s1", "s2", …], …]`.
pub fn parse_opensearch(body: &str) -> Result<Vec<String>, ProviderError> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(bad_json)?;
    let list = v
        .get(1)
        .and_then(|l| l.as_array())
        .ok_or_else(|| ProviderError::BadResponse("missing suggestion list".into()))?;
    Ok(list
        .iter()
        .filter_map(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

pub struct GoogleSuggest {
    base: String,
}

impl Default for GoogleSuggest {
    fn default() -> Self {
        Self {
            base: "https://suggestqueries.google.com/complete/search".into(),
        }
    }
}

#[async_trait]
impl SuggestProvider for GoogleSuggest {
    fn id(&self) -> &'static str {
        "suggest_google"
    }

    async fn suggest(&self, query: &str, market: &Market) -> Result<Vec<String>, ProviderError> {
        let u = url(
            &self.base,
            &[
                ("client", "firefox"),
                ("q", query),
                ("hl", market.lang()),
                ("gl", &market.country),
                ("ie", "utf-8"),
                ("oe", "utf-8"),
            ],
        )?;
        pace(self.id(), Duration::from_millis(200)).await;
        let res = check(client().get(u).send().await.map_err(net_err)?)?;
        parse_opensearch(&text(res, MAX_BODY).await?)
    }
}

pub struct BingSuggest {
    base: String,
}

impl Default for BingSuggest {
    fn default() -> Self {
        Self {
            base: "https://api.bing.com/osjson.aspx".into(),
        }
    }
}

#[async_trait]
impl SuggestProvider for BingSuggest {
    fn id(&self) -> &'static str {
        "suggest_bing"
    }

    async fn suggest(&self, query: &str, market: &Market) -> Result<Vec<String>, ProviderError> {
        let u = url(&self.base, &[("query", query), ("mkt", &market.locale())])?;
        pace(self.id(), Duration::from_millis(200)).await;
        let res = check(client().get(u).send().await.map_err(net_err)?)?;
        parse_opensearch(&text(res, MAX_BODY).await?)
    }
}

/// One autocomplete query derived from the seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub query: String,
    /// `seed` | `alphabet` | `question` | `preposition`
    pub kind: &'static str,
}

struct Modifiers {
    questions: &'static [&'static str],
    prepositions: &'static [&'static str],
}

fn modifiers(lang: &str) -> Modifiers {
    match lang {
        "es" => Modifiers {
            questions: &[
                "como", "que", "por que", "cuando", "donde", "cual", "quien", "cuanto",
            ],
            prepositions: &["para", "con", "sin", "vs", "cerca de", "en", "de"],
        },
        "de" => Modifiers {
            questions: &["wie", "was", "warum", "wann", "wo", "welche", "wer"],
            prepositions: &["für", "mit", "ohne", "vs", "in der nähe", "bei"],
        },
        "fr" => Modifiers {
            questions: &["comment", "quoi", "pourquoi", "quand", "où", "quel", "qui"],
            prepositions: &["pour", "avec", "sans", "vs", "près de", "en"],
        },
        "pt" => Modifiers {
            questions: &["como", "o que", "por que", "quando", "onde", "qual", "quem"],
            prepositions: &["para", "com", "sem", "vs", "perto de", "em"],
        },
        "id" | "ms" => Modifiers {
            questions: &[
                "cara",
                "apa",
                "kenapa",
                "kapan",
                "dimana",
                "berapa",
                "siapa",
                "bagaimana",
            ],
            prepositions: &["untuk", "dengan", "tanpa", "vs", "terdekat", "di"],
        },
        "en" => Modifiers {
            questions: &[
                "how", "what", "why", "when", "where", "which", "who", "can", "is", "are",
            ],
            prepositions: &["for", "with", "without", "vs", "near", "to", "like", "or"],
        },
        _ => Modifiers {
            questions: &[],
            prepositions: &["vs"],
        },
    }
}

/// Question words of a language (used to tag question-style ideas).
pub fn question_words(lang: &str) -> &'static [&'static str] {
    modifiers(lang).questions
}

/// The autocomplete queries to run for a seed: the seed itself, `seed a`…`seed z`,
/// question words before it and prepositions after it (per language).
pub fn expansions(seed: &str, lang: &str) -> Vec<Expansion> {
    let seed = seed.trim();
    let m = modifiers(lang);
    let mut out = vec![
        Expansion {
            query: seed.to_string(),
            kind: "seed",
        },
        Expansion {
            query: format!("{seed} "),
            kind: "seed",
        },
    ];
    out.extend(('a'..='z').map(|c| Expansion {
        query: format!("{seed} {c}"),
        kind: "alphabet",
    }));
    out.extend(m.questions.iter().map(|q| Expansion {
        query: format!("{q} {seed}"),
        kind: "question",
    }));
    out.extend(m.prepositions.iter().map(|p| Expansion {
        query: format!("{seed} {p} "),
        kind: "preposition",
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_and_bing_payloads() {
        let google = r#"["standing desk ",["standing desk converter","standing desk legs"],[],{"google:suggestsubtypes":[[512],[512]]}]"#;
        assert_eq!(
            parse_opensearch(google).unwrap(),
            vec!["standing desk converter", "standing desk legs"]
        );
        let bing = r#"["standing desk",["standing desk"," standing desk riser ",""]]"#;
        assert_eq!(
            parse_opensearch(bing).unwrap(),
            vec!["standing desk", "standing desk riser"]
        );
        assert!(parse_opensearch("<html>blocked</html>").is_err());
        assert!(parse_opensearch(r#"{"error":1}"#).is_err());
    }

    #[test]
    fn expands_seed() {
        let e = expansions(" crm ", "en");
        assert_eq!(e[0].query, "crm");
        assert!(e.iter().any(|x| x.query == "crm z" && x.kind == "alphabet"));
        assert!(
            e.iter()
                .any(|x| x.query == "how crm" && x.kind == "question")
        );
        assert!(
            e.iter()
                .any(|x| x.query == "crm for " && x.kind == "preposition")
        );
        // Unknown languages still get the alphabet expansion.
        assert!(expansions("crm", "ja").len() >= 28);
    }
}
