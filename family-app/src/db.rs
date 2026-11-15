//! The family database: `family.db` in the data directory, with migrations.

use std::{
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};

use deadpool_diesel::{
    InteractError, Runtime,
    sqlite::{Hook, HookError, Manager},
};
use diesel::connection::SimpleConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub use deadpool_diesel::sqlite::Pool;

/// The database file's name inside the data directory.
pub const DB_FILE: &str = "family.db";

/// The migrations in `migrations/`, embedded at compile time. Diesel records
/// each applied one in the `__diesel_schema_migrations` table and runs each
/// pending one once, in its own transaction.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Set on every new connection: enforce foreign keys, and wait up to five
/// seconds for a lock instead of failing at once with `SQLITE_BUSY`.
const CONNECTION_PRAGMAS: &str = "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;";
const MAX_CONNECTIONS: usize = 10;
/// How long a request waits for a free or new connection.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);

/// Any error from the pool, a query or a migration.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Why the database could not be opened or migrated.
#[derive(Debug)]
pub enum Error {
    /// The data directory does not exist or is not a directory.
    MissingDir(PathBuf),
    /// SQLite could not open the database file.
    Open(PathBuf, BoxError),
    /// A migration could not be applied.
    Migrate(PathBuf, BoxError),
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
pub async fn open(data_dir: &Path) -> Result<Pool, Error> {
    if !data_dir.is_dir() {
        return Err(Error::MissingDir(data_dir.to_path_buf()));
    }
    let path = data_dir.join(DB_FILE);
    let open_error = |err: BoxError| Error::Open(path.clone(), err);
    let pool = pool(&path).map_err(open_error)?;
    // Opening the first connection creates the file if it is missing.
    let conn = pool.get().await.map_err(|err| open_error(err.into()))?;
    conn.interact(|conn| conn.run_pending_migrations(MIGRATIONS).map(drop))
        .await
        .map_err(interact_error)
        .and_then(|result| result)
        .map_err(|err| Error::Migrate(path, err))?;
    Ok(pool)
}

/// A pool of connections to the SQLite file at `path`, each with
/// [`CONNECTION_PRAGMAS`] set. Connections open on first use.
fn pool(path: &Path) -> Result<Pool, BoxError> {
    let url = path.to_str().ok_or("the path is not valid UTF-8")?;
    let pool = Pool::builder(Manager::new(url, Runtime::Tokio1))
        .max_size(MAX_CONNECTIONS)
        .runtime(Runtime::Tokio1)
        .wait_timeout(Some(ACQUIRE_TIMEOUT))
        .create_timeout(Some(ACQUIRE_TIMEOUT))
        .post_create(Hook::async_fn(|conn, _| {
            Box::pin(async move {
                conn.interact(|conn| conn.batch_execute(CONNECTION_PRAGMAS))
                    .await
                    .map_err(|err| HookError::message(interact_error(err).to_string()))?
                    .map_err(|err| HookError::message(format!("cannot set pragmas: {err}")))
            })
        }))
        .build()?;
    Ok(pool)
}

/// A query's closure panicked or was cancelled on the blocking thread.
pub(crate) fn interact_error(err: InteractError) -> BoxError {
    format!("database task failed: {err}").into()
}
