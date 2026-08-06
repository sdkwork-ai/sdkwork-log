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

/// Redacts a JSON value recursively: object values whose key matches a
/// sensitive field name are replaced with `[REDACTED]` while the key is
/// preserved (so the payload shape stays readable for troubleshooting);
/// arrays recurse element-wise; primitive values are returned untouched.
pub fn redact_json_body(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut redacted = serde_json::Map::with_capacity(map.len());
            for (key, entry) in map {
                if is_sensitive_field_name(key) {
                    redacted.insert(
                        key.clone(),
                        serde_json::Value::String(REDACTED.to_owned()),
                    );
                } else {
                    redacted.insert(key.clone(), redact_json_body(entry));
                }
            }
            serde_json::Value::Object(redacted)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(redact_json_body).collect())
        }
        other => other.clone(),
    }
}

/// Redacts one text line conservatively: `key=value` pairs and `"key": value`
/// JSON-like pairs are detected by their first separator, and the value is
/// replaced when the key matches a sensitive field name.
fn redact_text_line(line: &str) -> String {
    if let Some((key, _)) = line.split_once('=') {
        if is_sensitive_field_name(key.trim()) {
            return format!("{key}=[REDACTED]");
        }
    }
    if let Some((key, value)) = line.split_once(':') {
        let key = key.trim();
        let unquoted = key.trim_start_matches('"').trim_end_matches('"');
        if is_sensitive_field_name(unquoted) {
            if value.trim_start().starts_with('"') {
                return format!("{key}: \"[REDACTED]\"");
            }
            return format!("{key}: [REDACTED]");
        }
    }
    line.to_owned()
}

/// Redacts request/response body text before persistence. JSON bodies are
/// parsed and redacted structurally (shape preserved, sensitive values
/// replaced); non-JSON bodies get a conservative line-oriented replacement.
/// Returns `None` when there is nothing to store.
pub fn redact_body_text(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return Some(redact_json_body(&value).to_string());
    }
    let redacted = text
        .lines()
        .map(redact_text_line)
        .collect::<Vec<String>>()
        .join("\n");
    if redacted.trim().is_empty() {
        None
    } else {
        Some(redacted)
    }
}

/// Truncates body text to at most `max_chars` characters, appending a
/// `[TRUNCATED]` marker when the input was cut. Capture adapters apply this
/// before persisting so oversized streaming bodies cannot balloon the row.
pub fn truncate_body_text(text: &str, max_chars: usize) -> String {
    let max = max_chars.max(1);
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let head: String = text.chars().take(max).collect();
        format!("{head}[TRUNCATED]")
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

    #[test]
    fn redact_json_body_replaces_sensitive_values_recursively() {
        let body = serde_json::json!({
            "prompt": "hello",
            "user": { "id": 7, "password": "hunter2" },
            "credentials": { "apiKey": "sk-abc", "safe": "kept" },
            "session": { "authToken": "t-1", "name": "ok" }
        });
        let redacted = redact_json_body(&body);
        assert_eq!(REDACTED, redacted["user"]["password"]);
        // A sensitive field name redacts the whole subtree.
        assert_eq!(REDACTED, redacted["credentials"]);
        assert_eq!(REDACTED, redacted["session"]["authToken"]);
        assert_eq!("hello", redacted["prompt"]);
        assert_eq!("ok", redacted["session"]["name"]);
        // Payload shape is preserved.
        assert!(redacted["user"]["id"].is_number());
    }

    #[test]
    fn redact_body_text_handles_json_and_plain_text() {
        let json = r#"{"prompt":"hi","secret":"s3cr3t"}"#;
        let redacted = redact_body_text(json).expect("json body");
        assert_eq!(
            r#"{"prompt":"hi","secret":"[REDACTED]"}"#,
            redacted
        );

        let text = "tenant_id=1001\ntoken=abc123\nstatus=ok";
        let redacted_text = redact_body_text(text).expect("text body");
        assert!(redacted_text.contains("token=[REDACTED]"));
        assert!(redacted_text.contains("tenant_id=1001"));
        assert!(redacted_text.contains("status=ok"));
    }

    #[test]
    fn redact_body_text_returns_none_for_empty_input() {
        assert_eq!(None, redact_body_text(""));
        assert_eq!(None, redact_body_text("   \n  "));
    }

    #[test]
    fn truncate_body_text_marks_cut_bodies() {
        let text = "abcdefgh";
        assert_eq!(text, truncate_body_text(text, 100));
        let truncated = truncate_body_text(text, 4);
        assert!(truncated.starts_with("abcd"));
        assert!(truncated.ends_with("[TRUNCATED]"));
    }
}
