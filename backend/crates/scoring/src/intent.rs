//! Search intent from keyword modifiers, the project's brand and SERP features.
//! Rule-based and explainable; an LLM pass can refine it later.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Informational,
    Commercial,
    Transactional,
    Navigational,
}

impl Intent {
    pub fn as_str(self) -> &'static str {
        match self {
            Intent::Informational => "informational",
            Intent::Commercial => "commercial",
            Intent::Transactional => "transactional",
            Intent::Navigational => "navigational",
        }
    }
}

const NAVIGATIONAL: &[&str] = &[
    "login",
    "log in",
    "sign in",
    "signin",
    "website",
    "official site",
    "homepage",
    "customer service",
    "contact",
    "masuk",
    "daftar",
    "iniciar sesion",
    "anmelden",
    "connexion",
];
const TRANSACTIONAL: &[&str] = &[
    "buy",
    "price",
    "prices",
    "pricing",
    "cost",
    "cheap",
    "coupon",
    "discount",
    "deal",
    "deals",
    "order",
    "for sale",
    "sale",
    "shop",
    "store",
    "purchase",
    "download",
    "subscribe",
    "hire",
    "rent",
    "near me",
    "delivery",
    "free trial",
    "quote",
    // id / ms
    "beli",
    "harga",
    "murah",
    "diskon",
    "promo",
    "jual",
    "sewa",
    "terdekat",
    "pesan",
    // es / pt
    "comprar",
    "precio",
    "precios",
    "barato",
    "descuento",
    "oferta",
    "preco",
    "cerca de mi",
    // de / fr
    "kaufen",
    "preis",
    "günstig",
    "rabatt",
    "bestellen",
    "acheter",
    "prix",
    "pas cher",
];
const COMMERCIAL: &[&str] = &[
    "best",
    "top",
    "review",
    "reviews",
    "vs",
    "versus",
    "compare",
    "comparison",
    "alternative",
    "alternatives",
    "affordable",
    "recommended",
    "worth it",
    "pros and cons",
    "terbaik",
    "rekomendasi",
    "perbandingan",
    "ulasan",
    "mejor",
    "mejores",
    "opiniones",
    "comparativa",
    "melhor",
    "melhores",
    "beste",
    "besten",
    "vergleich",
    "meilleur",
    "meilleurs",
    "avis",
    "comparatif",
];
const INFORMATIONAL: &[&str] = &[
    "how",
    "what",
    "why",
    "when",
    "where",
    "who",
    "which",
    "guide",
    "tutorial",
    "tips",
    "ideas",
    "examples",
    "example",
    "meaning",
    "definition",
    "learn",
    "can",
    "does",
    "is",
    "are",
    "benefits",
    "history",
    "types",
    "cara",
    "apa",
    "kenapa",
    "mengapa",
    "kapan",
    "bagaimana",
    "contoh",
    "arti",
    "pengertian",
    "como",
    "que",
    "por que",
    "cuando",
    "donde",
    "ejemplos",
    "significado",
    "wie",
    "was",
    "warum",
    "wann",
    "anleitung",
    "beispiele",
    "bedeutung",
    "comment",
    "pourquoi",
    "quand",
    "exemples",
    "définition",
];

fn has(words: &[&str], padded: &str, list: &[&str]) -> bool {
    list.iter().any(|m| {
        if m.contains(' ') {
            padded.contains(&format!(" {m} "))
        } else {
            words.contains(m)
        }
    })
}

/// Classify a (normalized) keyword. Order matters: a brand or login query is navigational
/// even if it also contains "price"; buying words beat comparison words; question words win
/// last. With no modifier, SERP features decide; the fallback is informational.
pub fn classify_intent(keyword: &str, brand: Option<&str>, features: &[String]) -> Intent {
    let kw = keyword.to_lowercase();
    let words: Vec<&str> = kw.split_whitespace().collect();
    let padded = format!(" {} ", words.join(" "));

    let brand = brand
        .map(|b| b.trim().to_lowercase())
        .filter(|b| b.len() >= 2);
    if let Some(b) = &brand
        && padded.contains(&format!(" {b} "))
        && !has(&words, &padded, COMMERCIAL)
        && !has(&words, &padded, INFORMATIONAL)
    {
        return Intent::Navigational;
    }
    if has(&words, &padded, NAVIGATIONAL) {
        return Intent::Navigational;
    }
    if has(&words, &padded, TRANSACTIONAL) {
        return Intent::Transactional;
    }
    if has(&words, &padded, COMMERCIAL) {
        return Intent::Commercial;
    }
    if has(&words, &padded, INFORMATIONAL) {
        return Intent::Informational;
    }
    let feature = |f: &str| features.iter().any(|x| x == f);
    if feature("shopping") || feature("local_pack") {
        Intent::Transactional
    } else {
        Intent::Informational
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(k: &str) -> Intent {
        classify_intent(k, Some("Acme"), &[])
    }

    #[test]
    fn classifies_by_modifier() {
        assert_eq!(c("how to clean a standing desk"), Intent::Informational);
        assert_eq!(c("best standing desk"), Intent::Commercial);
        assert_eq!(c("standing desk vs sitting"), Intent::Commercial);
        assert_eq!(c("buy standing desk"), Intent::Transactional);
        assert_eq!(c("standing desk near me"), Intent::Transactional);
        assert_eq!(c("harga meja kerja"), Intent::Transactional);
        assert_eq!(c("ikea login"), Intent::Navigational);
        assert_eq!(c("standing desk"), Intent::Informational);
    }

    #[test]
    fn brand_and_priority() {
        assert_eq!(c("acme"), Intent::Navigational);
        assert_eq!(c("acme pricing"), Intent::Navigational);
        assert_eq!(c("acme reviews"), Intent::Commercial);
        assert_eq!(c("acmecorp desk"), Intent::Informational);
        // Buying words beat comparison words.
        assert_eq!(c("best price standing desk"), Intent::Transactional);
        // Whole words only: "shopify" is not "shop", "topper" is not "top".
        assert_eq!(c("shopify themes"), Intent::Informational);
        assert_eq!(c("standing desk topper"), Intent::Informational);
    }

    #[test]
    fn serp_features_break_ties() {
        let f = vec!["shopping".to_string()];
        assert_eq!(
            classify_intent("standing desk", None, &f),
            Intent::Transactional
        );
        assert_eq!(
            classify_intent("how standing desk", None, &f),
            Intent::Informational
        );
    }
}
