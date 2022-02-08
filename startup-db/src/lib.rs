use std::marker::PhantomData;
use std::str::FromStr;

use futures_core::future::BoxFuture;
use serde::{Deserialize, Serialize};
use sqlx::{ConnectOptions, Database, PgPool, Pool, Postgres};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnectOptions;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseConfig<DB> {
    #[serde(skip)]
    pub _marker: PhantomData<fn() -> DB>,

    /// database connection url. See [sqlx::connect] for more information.
    pub url: url::Url,

    /// default search path or schema to use.
    pub schema: String,

    /// Enable query logging at 'debug' level.
    #[serde(default)]
    pub query_logging: bool,
}

pub trait ConnectExt<DB: Database> {
    /// Connect to the database and runs the given migrations.
    fn connect(&self, migrator: Migrator) -> BoxFuture<Result<Pool<DB>, sqlx::Error>>;
}

impl ConnectExt<Postgres> for DatabaseConfig<Postgres> {
    fn connect(&self, migrator: Migrator) -> BoxFuture<Result<PgPool, sqlx::Error>> {
        Box::pin(async move {
            let options = PgConnectOptions::from_str(&self.url.to_string())?;

            // make the requested schema the default search path.
            let mut options = options.options([("search_path", &self.schema)]);

            if self.query_logging {
                info!("Query logging is enabled (at debug level)");
                options.log_statements(log::LevelFilter::Debug);
            } else {
                options.log_statements(log::LevelFilter::Off);
            }

            info!("Connecting to postgres database");
            let pool = PgPool::connect_with(options).await?;

            info!("Ensure schema {:?} exists", self.schema);
            sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {:?}", self.schema))
                .execute(&pool)
                .await?;

            info!("Run database migrations");
            migrator.run(&pool).await?;

            Ok(pool)
        })
    }
}
