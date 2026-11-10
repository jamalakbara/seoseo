use common::{AppError, AppResult};

pub fn non_empty(field: &str, value: &str, max: usize) -> AppResult<String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    if v.chars().count() > max {
        return Err(AppError::Validation(format!(
            "{field} must be at most {max} characters"
        )));
    }
    Ok(v.to_string())
}

pub fn optional(field: &str, value: Option<&str>, max: usize) -> AppResult<Option<String>> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => non_empty(field, v, max).map(Some),
    }
}

pub fn email(value: &str) -> AppResult<String> {
    let v = value.trim().to_lowercase();
    let valid = v.len() <= 254
        && v.split_once('@').is_some_and(|(local, host)| {
            !local.is_empty() && host.contains('.') && !host.starts_with('.')
        })
        && !v.contains(char::is_whitespace);
    if !valid {
        return Err(AppError::Validation("enter a valid email address".into()));
    }
    Ok(v)
}

pub fn password(value: &str) -> AppResult<()> {
    let n = value.chars().count();
    if !(10..=256).contains(&n) {
        return Err(AppError::Validation(
            "password must be between 10 and 256 characters".into(),
        ));
    }
    Ok(())
}

/// Normalize user input like `https://Example.com/path` to a bare host `example.com`.
pub fn domain(value: &str) -> AppResult<String> {
    let mut v = value.trim().to_lowercase();
    for scheme in ["https://", "http://"] {
        if let Some(rest) = v.strip_prefix(scheme) {
            v = rest.to_string();
        }
    }
    let authority = v.split(['/', '?', '#']).next().unwrap_or_default();
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host_port
        .split(':')
        .next()
        .unwrap_or_default()
        .trim_end_matches('.');
    let valid = host.len() <= 253
        && host.contains('.')
        && host
            .split('.')
            .all(|l| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-'))
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    if !valid {
        return Err(AppError::Validation(
            "enter a domain like example.com (host only, no path)".into(),
        ));
    }
    Ok(host.to_string())
}

pub fn country(value: &str) -> AppResult<String> {
    let v = value.trim().to_uppercase();
    if v.len() == 2 && v.chars().all(|c| c.is_ascii_uppercase()) {
        Ok(v)
    } else {
        Err(AppError::Validation(
            "country must be an ISO 3166-1 alpha-2 code such as US or GB".into(),
        ))
    }
}

pub fn language(value: &str) -> AppResult<String> {
    let v = value.trim();
    let (lang, region) = v.split_once('-').unwrap_or((v, ""));
    let lang_ok = (2..=3).contains(&lang.len()) && lang.chars().all(|c| c.is_ascii_lowercase());
    let region_ok = region.is_empty()
        || ((2..=4).contains(&region.len()) && region.chars().all(|c| c.is_ascii_alphabetic()));
    if lang_ok && region_ok {
        Ok(v.to_string())
    } else {
        Err(AppError::Validation(
            "language must be an ISO 639-1 code such as en or pt-BR".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_is_normalized() {
        assert_eq!(
            domain("https://Example.com/path?q=1").unwrap(),
            "example.com"
        );
        assert_eq!(
            domain("www.shop.example.co.uk:8080").unwrap(),
            "www.shop.example.co.uk"
        );
        assert_eq!(domain("example.com.").unwrap(), "example.com");
        assert!(domain("localhost").is_err());
        assert!(domain("exa mple.com").is_err());
        assert!(domain("-bad.com").is_err());
    }

    #[test]
    fn email_and_market_validation() {
        assert_eq!(email(" A@B.io ").unwrap(), "a@b.io");
        assert!(email("nope").is_err());
        assert_eq!(country("gb").unwrap(), "GB");
        assert!(country("GBR").is_err());
        assert!(language("pt-BR").is_ok());
        assert!(language("EN").is_err());
    }
}
