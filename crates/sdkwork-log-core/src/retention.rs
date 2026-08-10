//! Request-log retention policies (framework-agnostic value types).
//!
//! Routes may declare their request-log retention through the web-framework
//! `HttpRoute::log_retention` annotation (`"permanent"` or `"<n>d"`). Hosts
//! turn those declarations (plus their own rules, for example billing tags)
//! into a [`LogRetentionPolicy`], and the capture layer resolves each request
//! path against it. `Permanent` rows are stored with `expires_at = NULL` and
//! are never purged; `Days(n)` rows expire after `n` days.

use serde::{Deserialize, Serialize};

/// How long a request log row is kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LogRetention {
    /// Keep the row forever (`expires_at = NULL`; purge skips it).
    Permanent,
    /// Keep the row for the given number of days.
    Days(i64),
}

impl LogRetention {
    /// Parses a route annotation value: `"permanent"` or `"<n>d"` (positive
    /// day count). Returns `None` for any other shape so invalid annotations
    /// fall back to the policy default instead of failing the request.
    pub fn parse(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("permanent") {
            return Some(Self::Permanent);
        }
        let days = trimmed.strip_suffix('d').or_else(|| trimmed.strip_suffix('D'))?;
        let days = days.parse::<i64>().ok()?;
        if days > 0 {
            Some(Self::Days(days))
        } else {
            None
        }
    }

    /// Wire/label form (`"permanent"` / `"30d"`).
    pub fn label(&self) -> String {
        match self {
            Self::Permanent => "permanent".to_owned(),
            Self::Days(days) => format!("{days}d"),
        }
    }

    /// Epoch seconds at which the row expires — `None` means never (permanent).
    pub fn expires_at_epoch(&self, now: i64) -> Option<i64> {
        match self {
            Self::Permanent => None,
            Self::Days(days) => Some(now.saturating_add(days.saturating_mul(86_400))),
        }
    }
}

/// Default retention when a route/path matches no rule (1 month).
pub const DEFAULT_LOG_RETENTION_DAYS: i64 = 30;

/// One policy rule: requests whose captured path starts with `path_prefix`
/// use `retention`. Rules are matched longest-prefix-first so a specific
/// route wins over a broader one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogRetentionRule {
    pub path_prefix: String,
    pub retention: LogRetention,
}

/// Ordered request-log retention policy for one hosting surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogRetentionPolicy {
    /// Retention applied when no rule matches (default 1 month).
    pub default_retention: LogRetention,
    /// Longest-prefix-first rules.
    pub rules: Vec<LogRetentionRule>,
}

impl LogRetentionPolicy {
    /// Policy with the default 1-month retention and no rules.
    pub fn default_month() -> Self {
        Self {
            default_retention: LogRetention::Days(DEFAULT_LOG_RETENTION_DAYS),
            rules: Vec::new(),
        }
    }

    /// Resolves the retention for a captured request path. Rules are sorted by
    /// descending prefix length so the most specific match wins.
    pub fn resolve(&self, path: &str) -> LogRetention {
        let mut best: Option<(&LogRetentionRule, usize)> = None;
        for rule in &self.rules {
            if path.starts_with(&rule.path_prefix) {
                match best {
                    Some((_, best_len)) if best_len >= rule.path_prefix.len() => {}
                    _ => best = Some((rule, rule.path_prefix.len())),
                }
            }
        }
        best.map(|(rule, _)| rule.retention)
            .unwrap_or(self.default_retention)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_annotation_values() {
        assert_eq!(Some(LogRetention::Permanent), LogRetention::parse("permanent"));
        assert_eq!(Some(LogRetention::Permanent), LogRetention::parse("Permanent"));
        assert_eq!(Some(LogRetention::Days(30)), LogRetention::parse("30d"));
        assert_eq!(None, LogRetention::parse(""));
        assert_eq!(None, LogRetention::parse("0d"));
        assert_eq!(None, LogRetention::parse("-5d"));
        assert_eq!(None, LogRetention::parse("forever"));
        assert_eq!(None, LogRetention::parse("d"));
    }

    #[test]
    fn labels_and_expiry_follow_retention() {
        let now = 1_700_000_000;
        assert_eq!("permanent", LogRetention::Permanent.label());
        assert_eq!("30d", LogRetention::Days(30).label());
        assert_eq!(None, LogRetention::Permanent.expires_at_epoch(now));
        assert_eq!(
            Some(now + 30 * 86_400),
            LogRetention::Days(30).expires_at_epoch(now)
        );
    }

    #[test]
    fn policy_resolves_longest_prefix_first() {
        let policy = LogRetentionPolicy {
            default_retention: LogRetention::Days(30),
            rules: vec![
                LogRetentionRule {
                    path_prefix: "/backend/v3/api/billing".to_owned(),
                    retention: LogRetention::Permanent,
                },
                LogRetentionRule {
                    path_prefix: "/v1".to_owned(),
                    retention: LogRetention::Permanent,
                },
                LogRetentionRule {
                    path_prefix: "/backend/v3/api".to_owned(),
                    retention: LogRetention::Days(60),
                },
            ],
        };
        assert_eq!(
            LogRetention::Permanent,
            policy.resolve("/backend/v3/api/billing/recharges/records")
        );
        assert_eq!(
            LogRetention::Days(60),
            policy.resolve("/backend/v3/api/iam/users")
        );
        assert_eq!(LogRetention::Permanent, policy.resolve("/v1/chat/completions"));
        assert_eq!(LogRetention::Days(30), policy.resolve("/app/v3/api/orders"));
    }
}
