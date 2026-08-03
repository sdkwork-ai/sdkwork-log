//! Dialect-selected pool enum mirroring `sdkwork-web-store-sqlx::WebStorePool`.

/// SQLx pool storage backend for the request log store.
#[derive(Clone)]
pub enum LogStorePool {
    #[cfg(feature = "sqlite")]
    Sqlite(sqlx::SqlitePool),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),
}
