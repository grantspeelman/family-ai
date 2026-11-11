//! The registry app: a GraphQL API served over HTTP.

pub mod accounts;
pub mod fly;
pub mod provisioning;

use accounts::{AccountStore, StoreError};
use async_graphql::{
    Context, EmptySubscription, Enum, ErrorExtensions, ID, Object, Schema, SimpleObject,
};
use async_graphql_axum::GraphQL;
use axum::{Router, routing::get, routing::post_service};
use provisioning::Provisioner;

/// The root of every GraphQL query.
pub struct Query;

#[Object]
impl Query {
    /// The version of the registry app that is answering.
    async fn api_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// The provisioning status and address of an account, for polling after
    /// `createAccount`. Carries no email or family name, because it needs no
    /// auth yet and ids are guessable. Null for an unknown or malformed id.
    async fn account_status(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> async_graphql::Result<Option<AccountProvisioning>> {
        let Ok(id) = id.parse::<i64>() else {
            return Ok(None);
        };
        let store = ctx.data::<AccountStore>().map_err(|_| internal_error())?;
        match store.get(id).await {
            Ok(account) => Ok(account.map(AccountProvisioning::from)),
            Err(_) => Err(internal_error()),
        }
    }
}

/// Where an account's family app is in being set up.
#[derive(Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountStatus {
    /// The family app's Fly resources are being created.
    Provisioning,
    /// The family app is running at the account's address.
    Ready,
    /// Provisioning failed, timed out or was interrupted by a restart.
    Failed,
}

impl From<accounts::Status> for AccountStatus {
    fn from(status: accounts::Status) -> Self {
        match status {
            accounts::Status::Provisioning => Self::Provisioning,
            accounts::Status::Ready => Self::Ready,
            accounts::Status::Failed => Self::Failed,
        }
    }
}

/// An account's provisioning status, without any personal data.
#[derive(SimpleObject)]
pub struct AccountProvisioning {
    /// The account's id.
    id: ID,
    /// Where the account's family app is in being set up.
    status: AccountStatus,
    /// The family app's base URL, such as `http://[fdaa::3]:8080`. Null
    /// unless the status is `READY`.
    address: Option<String>,
}

impl From<accounts::Account> for AccountProvisioning {
    fn from(account: accounts::Account) -> Self {
        Self {
            id: ID(account.id.to_string()),
            status: account.status.into(),
            address: account.address,
        }
    }
}

/// The longest email accepted, in characters.
const MAX_EMAIL_CHARS: usize = 254;
/// The longest family name accepted, in characters.
const MAX_FAMILY_NAME_CHARS: usize = 200;

/// A family's account.
#[derive(SimpleObject)]
pub struct Account {
    /// The id the registry assigned to the account.
    id: ID,
    /// The account's email, trimmed of surrounding whitespace.
    email: String,
    /// The family's name, trimmed of surrounding whitespace.
    family_name: String,
    /// Where the account's family app is in being set up.
    status: AccountStatus,
    /// The family app's base URL. Null unless the status is `READY`.
    address: Option<String>,
}

impl From<accounts::Account> for Account {
    fn from(account: accounts::Account) -> Self {
        Self {
            id: ID(account.id.to_string()),
            email: account.email,
            family_name: account.family_name,
            status: account.status.into(),
            address: account.address,
        }
    }
}

/// The root of every GraphQL mutation.
pub struct Mutation;

#[Object]
impl Mutation {
    /// Creates an account from an email and a family name. Surrounding
    /// whitespace is trimmed from both. The account comes back as
    /// `PROVISIONING` while its family app is set up in the background; poll
    /// `accountStatus` for the outcome. Errors carry `extensions.code`:
    /// `INVALID_EMAIL`, `INVALID_FAMILY_NAME` (empty or over 200 characters),
    /// `PROVISIONING_UNAVAILABLE` (the registry has no Fly API configured),
    /// `EMAIL_TAKEN` (ignoring letter case) or `INTERNAL`.
    async fn create_account(
        &self,
        ctx: &Context<'_>,
        email: String,
        family_name: String,
    ) -> async_graphql::Result<Account> {
        let email = email.trim();
        let family_name = family_name.trim();
        // Messages never include the inputs, which are personal data.
        if !is_valid_email(email) {
            return Err(coded_error(
                "The email address is not valid.",
                "INVALID_EMAIL",
            ));
        }
        if family_name.is_empty() || family_name.chars().count() > MAX_FAMILY_NAME_CHARS {
            return Err(coded_error(
                "The family name must be between 1 and 200 characters.",
                "INVALID_FAMILY_NAME",
            ));
        }
        let Some(provisioner) = ctx
            .data::<Option<Provisioner>>()
            .ok()
            .and_then(Option::as_ref)
        else {
            return Err(coded_error(
                "Accounts cannot be created right now.",
                "PROVISIONING_UNAVAILABLE",
            ));
        };
        let store = ctx.data::<AccountStore>().map_err(|_| internal_error())?;
        match store.create(email, family_name).await {
            Ok(account) => {
                provisioner.spawn(store.clone(), account.id);
                Ok(account.into())
            }
            Err(StoreError::DuplicateEmail) => Err(coded_error(
                "An account with this email already exists.",
                "EMAIL_TAKEN",
            )),
            Err(_) => Err(internal_error()),
        }
    }
}

fn internal_error() -> async_graphql::Error {
    coded_error("Something went wrong. Try again later.", "INTERNAL")
}

fn coded_error(message: &str, code: &'static str) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| extensions.set("code", code))
}

/// A sanity check, not a deliverability check: one `@` with something before
/// it, a domain with a `.` that is neither first nor last, no whitespace, and
/// at most 254 characters.
fn is_valid_email(email: &str) -> bool {
    if email.chars().count() > MAX_EMAIL_CHARS || email.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.contains('@')
        && domain
            .char_indices()
            .any(|(i, c)| c == '.' && i != 0 && i != domain.len() - 1)
}

pub type RegistrySchema = Schema<Query, Mutation, EmptySubscription>;

/// Builds the GraphQL schema, with `store` and `provisioner` as shared data
/// for the resolvers. Without a provisioner, `createAccount` returns
/// `PROVISIONING_UNAVAILABLE` and stores nothing.
pub fn schema(store: AccountStore, provisioner: Option<Provisioner>) -> RegistrySchema {
    Schema::build(Query, Mutation, EmptySubscription)
        .data(store)
        .data(provisioner)
        .finish()
}

/// Builds the HTTP app: `POST /graphql` and `GET /healthz`.
pub fn app(store: AccountStore, provisioner: Option<Provisioner>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/graphql",
            post_service(GraphQL::new(schema(store, provisioner))),
        )
}
