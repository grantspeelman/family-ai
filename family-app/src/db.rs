//! The family database: `family.db` in the data directory, with migrations.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

/// The database file's name inside the data directory.
pub const DB_FILE: &str = "family.db";

/// Migrations in order. Migration `i` (from zero) takes the schema from
/// version `i` to `i + 1`, recorded in `PRAGMA user_version`. Append only:
/// never edit or reorder a migration that has shipped.
const MIGRATIONS: &[&str] = &["CREATE TABLE notes (
        id INTEGER PRIMARY KEY,
        body TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    )"];

/// The schema version once every migration has run.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Why the database could not be opened or migrated.
#[derive(Debug)]
pub enum Error {
    /// The data directory does not exist or is not a directory.
    MissingDir(PathBuf),
    /// SQLite could not open the database file.
    Open(PathBuf, sqlx::Error),
    /// The current schema version could not be read.
    Version(PathBuf, sqlx::Error),
    /// A migration failed; the schema stays at `from`.
    Migrate {
        path: PathBuf,
        from: i64,
        source: sqlx::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingDir(dir) => {
                write!(
                    f,
                    "data directory \"{}\" does not exist or is not a directory",
                    dir.display()
                )
            }
            Error::Open(path, err) => write!(f, "cannot open {}: {err}", path.display()),
            Error::Version(path, err) => {
                write!(f, "cannot read schema version of {}: {err}", path.display())
            }
            Error::Migrate { path, from, source } => write!(
                f,
                "migration from version {from} failed on {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Opens or creates `family.db` in `data_dir` and applies pending migrations.
/// The directory must already exist; it is never created.
pub async fn open(data_dir: &Path) -> Result<SqlitePool, Error> {
    if !data_dir.is_dir() {
        return Err(Error::MissingDir(data_dir.to_path_buf()));
    }
    let path = data_dir.join(DB_FILE);
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .map_err(|err| Error::Open(path.clone(), err))?;
    migrate(&pool).await.map_err(|(from, source)| match from {
        None => Error::Version(path, source),
        Some(from) => Error::Migrate { path, from, source },
    })?;
    Ok(pool)
}

/// Runs each pending migration in its own transaction, which also bumps
/// `user_version`, so a migration and its version commit together.
async fn migrate(pool: &SqlitePool) -> Result<(), (Option<i64>, sqlx::Error)> {
    loop {
        let mut tx = pool.begin().await.map_err(|err| (None, err))?;
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *tx)
            .await
            .map_err(|err| (None, err))?;
        let Some(sql) = usize::try_from(version)
            .ok()
            .and_then(|index| MIGRATIONS.get(index))
        else {
            return Ok(());
        };
        let step = async {
            sqlx::query(sql).execute(&mut *tx).await?;
            sqlx::query(&format!("PRAGMA user_version = {}", version + 1))
                .execute(&mut *tx)
                .await?;
            tx.commit().await
        };
        step.await.map_err(|err| (Some(version), err))?;
    }
}
