//! Throttled TTL purge for `log_request` rows (best-effort, silent failure).

use crate::pool::LogStorePool;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

const DEFAULT_PURGE_INTERVAL_SECS: i64 = 60;

/// Deletes expired `log_request` rows at most once per interval.
///
/// Mirrors the throttled purge pattern of `sdkwork-web-store-sqlx`; failures are
/// logged at debug level and never propagate into the request path. SQL strings
/// are static literals per dialect (audited, no dynamic SQL).
#[derive(Clone)]
pub struct ThrottledPurge {
    pool: LogStorePool,
    last_purge_secs: Arc<AtomicI64>,
    interval_secs: i64,
}

impl ThrottledPurge {
    pub(crate) fn request_log(pool: LogStorePool) -> Self {
        Self {
            pool,
            last_purge_secs: Arc::new(AtomicI64::new(0)),
            interval_secs: DEFAULT_PURGE_INTERVAL_SECS,
        }
    }

    /// Runs the purge when the throttle interval elapsed; always fails silently.
    pub(crate) async fn maybe_run(&self) {
        let now = crate::now_epoch_secs();
        let last = self.last_purge_secs.load(Ordering::Relaxed);
        if now - last < self.interval_secs {
            return;
        }
        if self
            .last_purge_secs
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        let result: Result<(), sqlx::Error> = match &self.pool {
            #[cfg(feature = "sqlite")]
            LogStorePool::Sqlite(pool) => {
                sqlx::query(
                    "DELETE FROM log_request \
                     WHERE expires_at IS NOT NULL AND expires_at <= ?",
                )
                .bind(now)
                .execute(pool)
                .await
                .map(|_| ())
            }
            #[cfg(feature = "postgres")]
            LogStorePool::Postgres(pool) => {
                sqlx::query(
                    "DELETE FROM log_request \
                     WHERE expires_at IS NOT NULL AND expires_at <= $1",
                )
                .bind(now)
                .execute(pool)
                .await
                .map(|_| ())
            }
        };
        if let Err(error) = result {
            tracing::debug!(%error, "request log purge failed (best-effort)");
        }
    }
}
