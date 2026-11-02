//! The account store: each family's email and family name, in the registry's
//! own SQLite database.
//!
//! Accounts hold personal data, so nothing here logs them, and values reach
//! SQL only as bound parameters, which sqlx does not log. Errors never carry
//! an email or a family name.

use std::{error::Error, ffi::OsString, fmt, path::Path, path::PathBuf};

use sqlx::{
    SqlitePool,
    migrate::MigrateError,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

/// The environment variable that holds the database path.
pub const DB_PATH_VAR: &str = "REGISTRY_DB_PATH";

/// The constraint SQLite names when an email is already taken.
const EMAIL_CONSTRAINT: &str = "accounts.email_key";

/// Reads the database path from [`DB_PATH_VAR`] through `lookup`, which in
/// production is `|name| std::env::var_os(name)`.
pub fn db_path_from(
    lookup: impl FnOnce(&str) -> Option<OsString>,
) -> Result<PathBuf, MissingDbPath> {
    match lookup(DB_PATH_VAR) {
        Some(value) if !value.is_empty() => Ok(PathBuf::from(value)),
        _ => Err(MissingDbPath),
    }
}

/// [`DB_PATH_VAR`] is unset or empty.
#[derive(Debug, PartialEq, Eq)]
pub struct MissingDbPath;

impl fmt::Display for MissingDbPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{DB_PATH_VAR} is unset or empty")
    }
}

impl Error for MissingDbPath {}

/// A family's account.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Account {
    /// Assigned by the store.
    pub id: i64,
    pub email: String,
    pub family_name: String,
}

/// What can go wrong in the store.
#[derive(Debug)]
pub enum StoreError {
    /// Another account already has this email, ignoring letter case.
    DuplicateEmail,
    /// The schema could not be brought up to date.
    Migration(MigrateError),
    /// Any other database failure.
    Database(sqlx::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateEmail => f.write_str("an account with this email already exists"),
            Self::Migration(error) => write!(f, "account database migration failed: {error}"),
            Self::Database(error) => write!(f, "account database error: {error}"),
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DuplicateEmail => None,
            Self::Migration(error) => Some(error),
            Self::Database(error) => Some(error),
        }
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(error: sqlx::Error) -> Self {
        let is_duplicate_email = error.as_database_error().is_some_and(|database_error| {
            database_error.is_unique_violation()
                && database_error.message().contains(EMAIL_CONSTRAINT)
        });
        if is_duplicate_email {
            Self::DuplicateEmail
        } else {
            Self::Database(error)
        }
    }
}

/// A handle to the account database. Clones are cheap and share one
/// connection pool, so async handlers can each hold their own.
#[derive(Clone, Debug)]
pub struct AccountStore {
    pool: SqlitePool,
}

impl AccountStore {
    /// Opens the database at `path`, creating the file if needed, and applies
    /// any pending migrations.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        sqlx::migrate!()
            .run(&pool)
            .await
            .map_err(StoreError::Migration)?;
        Ok(Self { pool })
    }

    /// Creates an account, or returns [`StoreError::DuplicateEmail`] if the
    /// email is taken. Both values are stored exactly as given.
    pub async fn create(&self, email: &str, family_name: &str) -> Result<Account, StoreError> {
        let account = sqlx::query_as(
            "INSERT INTO accounts (email, email_key, family_name) VALUES (?, ?, ?) \
             RETURNING id, email, family_name",
        )
        .bind(email)
        .bind(email_key(email))
        .bind(family_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(account)
    }

    /// Fetches an account by id, or `None` if there is no such account.
    pub async fn get(&self, id: i64) -> Result<Option<Account>, StoreError> {
        let account = sqlx::query_as("SELECT id, email, family_name FROM accounts WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(account)
    }
}

/// The form of an email that uniqueness compares: it folds letter case in any
/// script. Going through upper case first also matches `ß` with `SS` and `ς`
/// with `Σ`. Stored rows depend on this, so changing it needs a migration.
fn email_key(email: &str) -> String {
    email
        .chars()
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect()
}
