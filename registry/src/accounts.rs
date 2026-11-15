//! The account store: each family's email and family name, in the registry's
//! own SQLite database.
//!
//! Accounts hold personal data, so nothing here logs them, and values reach
//! SQL only as bound parameters, which Diesel does not log. Errors never carry
//! an email or a family name.

use std::{error::Error, ffi::OsString, fmt, path::Path, path::PathBuf, time::Duration};

use deadpool_diesel::{
    InteractError, Runtime,
    sqlite::{Hook, HookError, Manager, Pool},
};
use diesel::{
    backend::Backend,
    connection::SimpleConnection,
    deserialize::{self, FromSql, FromSqlRow},
    expression::AsExpression,
    prelude::*,
    result::DatabaseErrorKind,
    serialize::{self, IsNull, Output, ToSql},
    sql_types::Text,
    sqlite::Sqlite,
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::schema::accounts;

/// The migrations in `migrations/`, embedded at compile time. Diesel records
/// each applied one in the `__diesel_schema_migrations` table and runs each
/// pending one once, in its own transaction.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Set on every new connection: enforce foreign keys, and wait up to five
/// seconds for a lock instead of failing at once with `SQLITE_BUSY`.
const CONNECTION_PRAGMAS: &str = "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;";
const MAX_CONNECTIONS: usize = 10;
/// How long a query waits for a free or new connection.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text)]
pub enum Status {
    /// A background task is creating the Fly resources.
    Provisioning,
    /// The family app is running at the account's address.
    Ready,
    /// Provisioning failed, timed out or was interrupted by a restart.
    Failed,
}

impl Status {
    /// The value in the `status` column.
    fn as_str(self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

impl ToSql<Text, Sqlite> for Status {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.as_str());
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Sqlite> for Status {
    fn from_sql(value: <Sqlite as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        [Self::Provisioning, Self::Ready, Self::Failed]
            .into_iter()
            .find(|status| status.as_str() == text)
            .ok_or_else(|| "unknown account status".into())
    }
}

/// A family's account.
#[derive(Clone, Debug, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = accounts, check_for_backend(Sqlite))]
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

/// What can go wrong in the store.
#[derive(Debug)]
pub enum StoreError {
    /// Another account already has this email, ignoring letter case.
    DuplicateEmail,
    /// The schema could not be brought up to date.
    Migration(Box<dyn Error + Send + Sync>),
    /// Any other database failure, including the connection pool's.
    Database(Box<dyn Error + Send + Sync>),
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
            Self::Migration(error) => Some(error.as_ref()),
            Self::Database(error) => Some(error.as_ref()),
        }
    }
}

impl From<diesel::result::Error> for StoreError {
    fn from(error: diesel::result::Error) -> Self {
        match &error {
            diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info)
                if info.message().contains(EMAIL_CONSTRAINT) =>
            {
                Self::DuplicateEmail
            }
            _ => Self::Database(Box::new(error)),
        }
    }
}

/// A query's closure panicked or was cancelled on the blocking thread.
fn interact_error(error: InteractError) -> Box<dyn Error + Send + Sync> {
    format!("database task failed: {error}").into()
}

/// The Fly resource ids that provisioning records, one column each.
#[derive(Clone, Copy)]
enum FlyId {
    AppName,
    VolumeId,
    MachineId,
}

/// A handle to the account database. Clones are cheap and share one
/// connection pool, so async handlers can each hold their own.
#[derive(Clone)]
pub struct AccountStore {
    pool: Pool,
}

impl fmt::Debug for AccountStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountStore")
            .field("status", &self.pool.status())
            .finish()
    }
}

impl AccountStore {
    /// Opens the database at `path`, creating the file if needed, and applies
    /// any pending migrations.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let store = Self { pool: pool(path)? };
        store
            .run_raw(|conn| conn.run_pending_migrations(MIGRATIONS).map(drop))
            .await
            .map_err(StoreError::Database)?
            .map_err(StoreError::Migration)?;
        Ok(store)
    }

    /// Creates an account, or returns [`StoreError::DuplicateEmail`] if the
    /// email is taken. Both values are stored exactly as given.
    pub async fn create(&self, email: &str, family_name: &str) -> Result<Account, StoreError> {
        let row = (
            accounts::email.eq(email.to_owned()),
            accounts::email_key.eq(email_key(email)),
            accounts::family_name.eq(family_name.to_owned()),
            accounts::status.eq(Status::Provisioning),
        );
        self.run(move |conn| {
            diesel::insert_into(accounts::table)
                .values(row)
                .returning(Account::as_returning())
                .get_result(conn)
        })
        .await
    }

    /// Fetches an account by id, or `None` if there is no such account.
    pub async fn get(&self, id: i64) -> Result<Option<Account>, StoreError> {
        self.run(move |conn| {
            accounts::table
                .find(id)
                .select(Account::as_select())
                .first(conn)
                .optional()
        })
        .await
    }

    /// Records the Fly app name of an account that is still provisioning.
    pub async fn set_fly_app_name(&self, id: i64, name: &str) -> Result<(), StoreError> {
        self.set_while_provisioning(id, FlyId::AppName, name).await
    }

    /// Records the Fly volume id of an account that is still provisioning.
    pub async fn set_fly_volume_id(&self, id: i64, volume_id: &str) -> Result<(), StoreError> {
        self.set_while_provisioning(id, FlyId::VolumeId, volume_id)
            .await
    }

    /// Records the Fly Machine id of an account that is still provisioning.
    pub async fn set_fly_machine_id(&self, id: i64, machine_id: &str) -> Result<(), StoreError> {
        self.set_while_provisioning(id, FlyId::MachineId, machine_id)
            .await
    }

    async fn set_while_provisioning(
        &self,
        id: i64,
        column: FlyId,
        value: &str,
    ) -> Result<(), StoreError> {
        let value = value.to_owned();
        self.run(move |conn| {
            let target = provisioning(id);
            match column {
                FlyId::AppName => diesel::update(target)
                    .set(accounts::fly_app_name.eq(value))
                    .execute(conn),
                FlyId::VolumeId => diesel::update(target)
                    .set(accounts::fly_volume_id.eq(value))
                    .execute(conn),
                FlyId::MachineId => diesel::update(target)
                    .set(accounts::fly_machine_id.eq(value))
                    .execute(conn),
            }
        })
        .await?;
        Ok(())
    }

    /// Marks an account that is still provisioning as ready at `address`.
    /// Returns whether a row changed.
    pub async fn mark_ready(&self, id: i64, address: &str) -> Result<bool, StoreError> {
        let address = address.to_owned();
        let changed = self
            .run(move |conn| {
                diesel::update(provisioning(id))
                    .set((
                        accounts::status.eq(Status::Ready),
                        accounts::address.eq(Some(address)),
                    ))
                    .execute(conn)
            })
            .await?;
        Ok(changed > 0)
    }

    /// Marks an account that is still provisioning as failed. Returns whether
    /// a row changed.
    pub async fn mark_failed(&self, id: i64) -> Result<bool, StoreError> {
        let changed = self
            .run(move |conn| diesel::update(provisioning(id)).set(failed()).execute(conn))
            .await?;
        Ok(changed > 0)
    }

    /// Marks every account still provisioning as failed, because its task
    /// died with the previous process. Run at startup, before serving.
    /// Returns how many accounts changed.
    pub async fn fail_interrupted(&self) -> Result<u64, StoreError> {
        let changed = self
            .run(|conn| {
                diesel::update(accounts::table.filter(accounts::status.eq(Status::Provisioning)))
                    .set(failed())
                    .execute(conn)
            })
            .await?;
        Ok(changed as u64)
    }

    /// Runs `query` on a pooled connection, off the async runtime's threads.
    async fn run<T: Send + 'static>(
        &self,
        query: impl FnOnce(&mut SqliteConnection) -> QueryResult<T> + Send + 'static,
    ) -> Result<T, StoreError> {
        self.run_raw(move |conn| query(conn).map_err(StoreError::from))
            .await
            .map_err(StoreError::Database)?
    }

    /// Runs `task` on a pooled connection. The outer error is the pool's or
    /// the blocking task's; the inner one is `task`'s own.
    async fn run_raw<T: Send + 'static, E: Send + 'static>(
        &self,
        task: impl FnOnce(&mut SqliteConnection) -> Result<T, E> + Send + 'static,
    ) -> Result<Result<T, E>, Box<dyn Error + Send + Sync>> {
        let conn = self.pool.get().await?;
        conn.interact(task).await.map_err(interact_error)
    }
}

/// The row with `id`, if it is still provisioning.
fn provisioning(
    id: i64,
) -> diesel::dsl::Filter<
    diesel::dsl::Find<accounts::table, i64>,
    diesel::dsl::Eq<accounts::status, Status>,
> {
    accounts::table
        .find(id)
        .filter(accounts::status.eq(Status::Provisioning))
}

/// The change that marks an account failed: no address.
fn failed() -> (
    diesel::dsl::Eq<accounts::status, Status>,
    diesel::dsl::Eq<accounts::address, Option<String>>,
) {
    (
        accounts::status.eq(Status::Failed),
        accounts::address.eq(None),
    )
}

/// A pool of connections to the SQLite file at `path`, each with
/// [`CONNECTION_PRAGMAS`] set. Connections open on first use, and opening one
/// creates the file if it is missing.
fn pool(path: &Path) -> Result<Pool, StoreError> {
    let url = path
        .to_str()
        .ok_or_else(|| StoreError::Database("the path is not valid UTF-8".into()))?;
    Pool::builder(Manager::new(url, Runtime::Tokio1))
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
        .build()
        .map_err(|error| StoreError::Database(Box::new(error)))
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

#[cfg(test)]
mod tests {
    use diesel::sql_types::BigInt;

    use super::*;

    /// A store on a fresh database file, removed with the returned guard.
    async fn fresh(name: &str) -> (Removed, AccountStore) {
        let path =
            std::env::temp_dir().join(format!("registry-unit-{name}-{}.db", std::process::id()));
        let removed = Removed(path);
        removed.clear();
        let store = AccountStore::open(&removed.0).await.unwrap();
        (removed, store)
    }

    struct Removed(PathBuf);

    impl Removed {
        fn clear(&self) {
            for suffix in ["", "-wal", "-shm", "-journal"] {
                let mut path = self.0.clone().into_os_string();
                path.push(suffix);
                let _ = std::fs::remove_file(path);
            }
        }
    }

    impl Drop for Removed {
        fn drop(&mut self) {
            self.clear();
        }
    }

    #[derive(QueryableByName, Debug, PartialEq)]
    struct Settings {
        #[diesel(sql_type = BigInt)]
        foreign_keys: i64,
        #[diesel(sql_type = BigInt)]
        busy_timeout: i64,
    }

    #[tokio::test]
    async fn every_pooled_connection_enforces_foreign_keys_and_waits_when_busy() {
        let (_removed, store) = fresh("settings").await;
        // Held at once, so the pool has to open a second connection.
        let first = store.pool.get().await.unwrap();
        let second = store.pool.get().await.unwrap();

        for conn in [first, second] {
            let settings: Settings = conn
                .interact(|conn| {
                    diesel::sql_query(
                        "SELECT (SELECT foreign_keys FROM pragma_foreign_keys) AS foreign_keys, \
                         (SELECT timeout FROM pragma_busy_timeout) AS busy_timeout",
                    )
                    .get_result(conn)
                })
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                settings,
                Settings {
                    foreign_keys: 1,
                    busy_timeout: 5000
                }
            );
        }
    }

    #[tokio::test]
    async fn a_closed_pool_is_a_database_error() {
        let (_removed, store) = fresh("closed").await;
        store.pool.close();

        let error = store.create("ana@example.com", "Silva").await.unwrap_err();

        assert!(matches!(error, StoreError::Database(_)), "got {error:?}");
        assert!(error.source().is_some());
    }

    #[tokio::test]
    async fn a_panicking_query_is_a_database_error() {
        let (_removed, store) = fresh("panic").await;

        let error = store
            .run(|_| -> QueryResult<()> { panic!("query bug") })
            .await
            .unwrap_err();

        let StoreError::Database(inner) = &error else {
            panic!("got {error:?}");
        };
        assert!(
            inner.to_string().contains("database task failed"),
            "{inner}"
        );
        // The store still works afterwards.
        assert!(store.get(1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn an_unknown_status_in_the_table_is_a_database_error() {
        let (_removed, store) = fresh("unknown-status").await;
        let account = store.create("ana@example.com", "Silva").await.unwrap();
        store
            .run(move |conn| {
                conn.batch_execute("PRAGMA ignore_check_constraints = ON")?;
                diesel::sql_query("UPDATE accounts SET status = 'paused' WHERE id = ?")
                    .bind::<BigInt, _>(account.id)
                    .execute(conn)
            })
            .await
            .unwrap();

        let error = store.get(account.id).await.unwrap_err();

        assert!(matches!(error, StoreError::Database(_)), "got {error:?}");
    }
}
