//! The family database: `family.db` in the data directory, with migrations.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use sqlx::{
    SqlitePool,
    migrate::{MigrateError, Migrator},
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

/// The database file's name inside the data directory.
pub const DB_FILE: &str = "family.db";

/// The migrations in `migrations/`, embedded at compile time. sqlx records
/// each applied one in the `_sqlx_migrations` table and runs each pending one
/// once, in its own transaction.
static MIGRATOR: Migrator = sqlx::migrate!();

/// Why the database could not be opened or migrated.
#[derive(Debug)]
pub enum Error {
    /// The data directory does not exist or is not a directory.
    MissingDir(PathBuf),
    /// SQLite could not open the database file.
    Open(PathBuf, sqlx::Error),
    /// A migration could not be applied.
    Migrate(PathBuf, MigrateError),
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
            Error::Migrate(path, err) => {
                write!(f, "cannot migrate {}: {err}", path.display())
            }
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
    MIGRATOR
        .run(&pool)
        .await
        .map_err(|err| Error::Migrate(path, err))?;
    Ok(pool)
}
