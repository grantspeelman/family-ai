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

/// Where an account's Fly app, volume and Machine are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum Status {
    /// A background task is creating the Fly resources.
    Provisioning,
    /// The family app is running at the account's address.
    Ready,
    /// Provisioning failed, timed out or was interrupted by a restart.
    Failed,
}

/// A family's account.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Account {
    /// Assigned by the store.
    pub id: i64,
    pub email: String,
    pub family_name: String,
    pub status: Status,
    /// The family app's base URL. Set only when the status is `Ready`.
    pub address: Option<String>,
    pub fly_app_name: Option<String>,
    pub fly_volume_id: Option<String>,
    pub fly_machine_id: Option<String>,
}

const ACCOUNT_COLUMNS: &str =
    "id, email, family_name, status, address, fly_app_name, fly_volume_id, fly_machine_id";

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
        let sql = format!(
            "INSERT INTO accounts (email, email_key, family_name, status) \
             VALUES (?, ?, ?, 'provisioning') RETURNING {ACCOUNT_COLUMNS}"
        );
        let account = sqlx::query_as(&sql)
            .bind(email)
            .bind(email_key(email))
            .bind(family_name)
            .fetch_one(&self.pool)
            .await?;
        Ok(account)
    }

    /// Fetches an account by id, or `None` if there is no such account.
    pub async fn get(&self, id: i64) -> Result<Option<Account>, StoreError> {
        let sql = format!("SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE id = ?");
        let account = sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(account)
    }

    /// Records the Fly app name of an account that is still provisioning.
    pub async fn set_fly_app_name(&self, id: i64, name: &str) -> Result<(), StoreError> {
        self.set_while_provisioning(id, "fly_app_name", name).await
    }

    /// Records the Fly volume id of an account that is still provisioning.
    pub async fn set_fly_volume_id(&self, id: i64, volume_id: &str) -> Result<(), StoreError> {
        self.set_while_provisioning(id, "fly_volume_id", volume_id)
            .await
    }

    /// Records the Fly Machine id of an account that is still provisioning.
    pub async fn set_fly_machine_id(&self, id: i64, machine_id: &str) -> Result<(), StoreError> {
        self.set_while_provisioning(id, "fly_machine_id", machine_id)
            .await
    }

    async fn set_while_provisioning(
        &self,
        id: i64,
        column: &'static str,
        value: &str,
    ) -> Result<(), StoreError> {
        let sql =
            format!("UPDATE accounts SET {column} = ? WHERE id = ? AND status = 'provisioning'");
        sqlx::query(&sql)
            .bind(value)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Marks an account that is still provisioning as ready at `address`.
    /// Returns whether a row changed.
    pub async fn mark_ready(&self, id: i64, address: &str) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE accounts SET status = 'ready', address = ? \
             WHERE id = ? AND status = 'provisioning'",
        )
        .bind(address)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Marks an account that is still provisioning as failed. Returns whether
    /// a row changed.
    pub async fn mark_failed(&self, id: i64) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE accounts SET status = 'failed', address = NULL \
             WHERE id = ? AND status = 'provisioning'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Marks every account still provisioning as failed, because its task
    /// died with the previous process. Run at startup, before serving.
    /// Returns how many accounts changed.
    pub async fn fail_interrupted(&self) -> Result<u64, StoreError> {
        let result = sqlx::query(
            "UPDATE accounts SET status = 'failed', address = NULL \
             WHERE status = 'provisioning'",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
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
