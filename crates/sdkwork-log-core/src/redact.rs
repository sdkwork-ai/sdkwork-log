//! Redaction for captured request parameters and safe request headers.
//!
//! Query parameters are captured for troubleshooting but values whose field name
//! matches known credential/secret patterns are replaced with `[REDACTED]`
//! (`OBSERVABILITY_SPEC.md` §2 / `DATABASE_SPEC.md` §18: raw tokens, passwords,
//! verification codes, OAuth codes, and private keys MUST NOT be stored).
//!
//! The sensitive-name list mirrors `sdkwork_web_core::redact` (the framework's
//! structured-log redaction authority); keep both lists in sync.

pub const REDACTED: &str = "[REDACTED]";

/// Header names whose values must never be captured (credential, cookie, or
/// idempotency headers).
fn is_sensitive_header_name(normalized: &str) -> bool {
    matches!(
        normalized,
        "authorization"
            | "x-api-key"
            | "api-key"
            | "access-token"
            | "x-access-token"
            | "x-sdkwork-access-token"
            | "x-sdkwork-auth-token"
            | "x-sdkwork-ingress-token"
            | "x-sdkwork-agent-token"
            | "cookie"
            | "set-cookie"
            | "x-idempotency-key"
            | "idempotency-key"
    )
}

/// True when a query/field name must have its value redacted.
pub fn is_sensitive_field_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    if is_sensitive_header_name(&normalized) {
        return true;
    }
    matches!(
        normalized.as_str(),
        "password"
            | "passwd"
            | "secret"
            | "api_key"
            | "apikey"
            | "token"
            | "auth_token"
            | "access_token"
            | "refresh_token"
            | "bearer"
            | "credential"
            | "credentials"
            | "code"
            | "sign"
            | "signature"
            | "otp"
            | "pin"
    ) || normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("signature")
        || normalized.contains("verification_code")
        || normalized.contains("verificationcode")
        || normalized.contains("verify_code")
        || normalized.contains("verifycode")
        || normalized.contains("sms_code")
        || normalized.contains("otp")
}

/// Redacts one captured value by field name.
pub fn redact_sensitive_value(field_name: &str, value: &str) -> String {
    if is_sensitive_field_name(field_name) {
        REDACTED.to_owned()
    } else {
        value.to_owned()
    }
}

/// Redacts query string parameters (`k=v&k2=v2`), preserving order and
/// separators. Returns `None` when there is nothing safe to store.
pub fn redact_query_params(query: Option<&str>) -> Option<String> {
    let query = query?;
    if query.trim().is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut first = true;
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if !first {
            out.push('&');
        }
        first = false;
        match pair.split_once('=') {
            Some((key, value)) => {
                out.push_str(key);
                out.push('=');
                out.push_str(&redact_sensitive_value(key, value));
            }
            None => out.push_str(pair),
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Allow-list of request headers whose values are safe to capture. Credential,
/// cookie, and idempotency headers are always excluded; signed-URL carriers
/// (for example `referer`) are excluded as well.
pub fn is_safe_request_header(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    !is_sensitive_header_name(&normalized)
        && matches!(
            normalized.as_str(),
            "user-agent"
                | "content-type"
                | "accept"
                | "accept-language"
                | "origin"
                | "x-forwarded-for"
                | "x-real-ip"
                | "x-client-kind"
                | "x-sdkwork-client-kind"
                | "x-request-id"
                | "traceparent"
                | "tracestate"
                | "x-sdkwork-trace-id"
        )
}

/// Captures allow-listed headers as a JSON object string
/// (`{"user-agent":"...","content-type":"..."}`). Returns `None` when nothing
/// was captured.
pub fn capture_safe_headers(headers: &[(String, String)]) -> Option<String> {
    if headers.is_empty() {
        return None;
    }
    let mut map = serde_json::Map::new();
    for (name, value) in headers {
        if is_safe_request_header(name) {
            map.insert(
                name.trim().to_ascii_lowercase(),
                serde_json::Value::String(redact_sensitive_value(name, value)),
            );
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_credential_query_values() {
        assert_eq!("[REDACTED]", redact_sensitive_value("access_token", "abc"));
        assert_eq!("[REDACTED]", redact_sensitive_value("sign", "hmac-value"));
        assert_eq!("[REDACTED]", redact_sensitive_value("code", "oauth-code"));
        assert_eq!("[REDACTED]", redact_sensitive_value("verifyCode", "123456"));
        assert_eq!("123", redact_sensitive_value("page_size", "123"));
    }

    #[test]
    fn redact_query_params_preserves_safe_pairs() {
        let redacted = redact_query_params(Some("page=2&token=secret&page_size=20"))
            .expect("query");
        assert_eq!("page=2&token=[REDACTED]&page_size=20", redacted);
    }

    #[test]
    fn redact_query_params_handles_empty_and_key_only() {
        assert_eq!(None, redact_query_params(None));
        assert_eq!(None, redact_query_params(Some("")));
        assert_eq!(Some("flag".to_owned()), redact_query_params(Some("flag")));
    }

    #[test]
    fn capture_safe_headers_excludes_credentials_and_signed_url_carriers() {
        let headers = vec![
            ("Authorization".to_owned(), "Bearer xyz".to_owned()),
            ("User-Agent".to_owned(), "test-agent".to_owned()),
            ("Referer".to_owned(), "https://x/signed?token=abc".to_owned()),
            ("X-Request-Id".to_owned(), "req-1".to_owned()),
        ];
        let captured = capture_safe_headers(&headers).expect("headers");
        assert!(captured.contains("\"user-agent\":\"test-agent\""));
        assert!(captured.contains("\"x-request-id\":\"req-1\""));
        assert!(!captured.contains("Authorization"));
        assert!(!captured.contains("Referer"));
        assert!(!captured.contains("token"));
    }

    #[test]
    fn capture_safe_headers_returns_none_when_nothing_safe() {
        let headers = vec![("Authorization".to_owned(), "Bearer xyz".to_owned())];
        assert_eq!(None, capture_safe_headers(&headers));
    }
}
