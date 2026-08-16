//! Client IP handling for request logs.
//!
//! Raw client IP addresses are personal data and are never persisted
//! (`DATABASE_SPEC.md` §18). Rows store a SHA-256 hex digest for exact-match
//! lookup plus a masked form for display — IPv4 hides the last octet
//! (`1.2.3.x`), IPv6 is truncated to its `/64` subnet — mirroring the
//! `ai_metering_request_trace` convention in the cloudrouter data model.

use std::net::IpAddr;

/// Parses a client IP from a header value, returning the canonical `IpAddr`.
/// Rejects values that are not valid IPs so spoofed or malformed header
/// entries never reach the log.
pub fn parse_client_ip(value: &str) -> Option<IpAddr> {
    value.trim().parse::<IpAddr>().ok()
}

/// First parseable IP from an `x-forwarded-for` list (the leftmost entry is
/// the original client as appended by proxies).
pub fn first_forwarded_ip(value: &str) -> Option<IpAddr> {
    value.split(',').map(str::trim).find_map(parse_client_ip)
}

/// Masks an IP for display: IPv4 hides the last octet (`1.2.3.x`), IPv6 keeps
/// the `/64` subnet prefix (`2001:db8:1:2::/64`).
pub fn mask_client_ip(addr: IpAddr) -> String {
    match addr {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            format!("{}.{}.{}.x", octets[0], octets[1], octets[2])
        }
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            format!(
                "{:x}:{:x}:{:x}:{:x}::/64",
                segments[0], segments[1], segments[2], segments[3]
            )
        }
    }
}

/// SHA-256 hex digest of the canonical IP string, for exact-match lookups
/// without persisting the plaintext address.
pub fn hash_client_ip(addr: IpAddr) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(addr.to_string().as_bytes());
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn parses_valid_ips_only() {
        assert_eq!(
            Some(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))),
            parse_client_ip("1.2.3.4")
        );
        assert_eq!(None, parse_client_ip("not-an-ip"));
        assert_eq!(None, parse_client_ip(""));
        assert_eq!(None, parse_client_ip("1.2.3"));
    }

    #[test]
    fn first_forwarded_ip_takes_leftmost_valid() {
        assert_eq!(
            Some(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))),
            first_forwarded_ip("1.2.3.4, 10.0.0.1")
        );
        // Skips malformed leading entries like spoofed garbage.
        assert_eq!(
            Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
            first_forwarded_ip("garbage, 10.0.0.1")
        );
        assert_eq!(None, first_forwarded_ip("garbage"));
    }

    #[test]
    fn masks_ipv4_last_octet() {
        assert_eq!(
            "1.2.3.x".to_owned(),
            mask_client_ip(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)))
        );
        assert_eq!(
            "192.168.0.x".to_owned(),
            mask_client_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 7)))
        );
    }

    #[test]
    fn masks_ipv6_to_subnet_prefix() {
        let addr: IpAddr = "2001:db8:0:1:dead:beef::1".parse().expect("ipv6");
        assert_eq!("2001:db8:0:1::/64".to_owned(), mask_client_ip(addr));
    }

    #[test]
    fn hash_is_stable_64_char_hex() {
        let addr = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let hash = hash_client_ip(addr);
        assert_eq!(64, hash.len());
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(hash, hash_client_ip(addr));
        assert_ne!(hash, hash_client_ip(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 5))));
    }
}
