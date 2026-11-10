//! SSRF guard. Every URL we fetch for a user (start URL, redirects, links, sitemaps, robots)
//! must pass [`UrlPolicy::check_url`], and every resolved IP must pass [`UrlPolicy::allows_ip`].
//! The fetcher installs a DNS resolver that applies the IP check, so DNS rebinding and
//! redirects to internal hosts are blocked at connect time.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::{Host, Url};

#[derive(Debug, Clone, Copy, Default)]
pub struct UrlPolicy {
    /// Allow private/loopback targets. Only for local testing (`CRAWLER_ALLOW_PRIVATE=true`).
    pub allow_private: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("only http and https URLs can be crawled")]
    Scheme,
    #[error("URLs with credentials are not allowed")]
    Credentials,
    #[error("this address is on a private or reserved network and cannot be crawled")]
    PrivateAddress,
    #[error("URL has no host")]
    NoHost,
}

impl UrlPolicy {
    pub fn check_url(&self, url: &Url) -> Result<(), PolicyError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(PolicyError::Scheme);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(PolicyError::Credentials);
        }
        match url.host() {
            None => Err(PolicyError::NoHost),
            Some(Host::Ipv4(ip)) => self.ip_result(IpAddr::V4(ip)),
            Some(Host::Ipv6(ip)) => self.ip_result(IpAddr::V6(ip)),
            Some(Host::Domain(d)) => {
                let d = d.trim_end_matches('.').to_ascii_lowercase();
                let internal = d == "localhost"
                    || d.ends_with(".localhost")
                    || d.ends_with(".local")
                    || d.ends_with(".internal")
                    || !d.contains('.');
                if internal && !self.allow_private {
                    Err(PolicyError::PrivateAddress)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn ip_result(&self, ip: IpAddr) -> Result<(), PolicyError> {
        if self.allows_ip(ip) {
            Ok(())
        } else {
            Err(PolicyError::PrivateAddress)
        }
    }

    pub fn allows_ip(&self, ip: IpAddr) -> bool {
        self.allow_private || is_public(ip)
    }
}

/// True for globally routable unicast addresses.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            is_public_v6(v6)
        }
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || o[0] == 0
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64/10 carrier-grade NAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0/24 IETF
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18/15 benchmarking
        || o[0] >= 240) // reserved
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (s[0] & 0xffc0) == 0xfe80 // link local fe80::/10
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
        || (s[0] == 0x64 && s[1] == 0xff9b)) // NAT64 can reach IPv4 internals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(u: &str) -> Result<(), PolicyError> {
        UrlPolicy::default().check_url(&Url::parse(u).unwrap())
    }

    #[test]
    fn blocks_internal_targets() {
        for u in [
            "http://127.0.0.1/",
            "http://10.1.2.3/",
            "http://169.254.169.254/latest/meta-data",
            "http://[::1]/",
            "http://[::ffff:192.168.0.1]/",
            "http://[fd00::1]/",
            "http://localhost:8080/",
            "http://db/",
            "http://metadata.google.internal/",
            "http://100.100.1.1/",
        ] {
            assert_eq!(check(u), Err(PolicyError::PrivateAddress), "{u}");
        }
        assert_eq!(check("ftp://example.com/"), Err(PolicyError::Scheme));
        assert_eq!(
            check("http://user:pw@example.com/"),
            Err(PolicyError::Credentials)
        );
    }

    #[test]
    fn allows_public_targets() {
        assert!(check("https://example.com/").is_ok());
        assert!(check("http://93.184.216.34/").is_ok());
        assert!(
            UrlPolicy {
                allow_private: true
            }
            .check_url(&Url::parse("http://badseo/").unwrap())
            .is_ok()
        );
    }
}
