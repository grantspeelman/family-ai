//! The registry app: a GraphQL API served over HTTP.

pub mod accounts;
pub mod fly;

use accounts::{AccountStore, StoreError};
use async_graphql::{
    Context, EmptySubscription, ErrorExtensions, ID, Object, Schema, SimpleObject,
};
use async_graphql_axum::GraphQL;
use axum::{Router, routing::get, routing::post_service};

/// The root of every GraphQL query.
pub struct Query;

#[Object]
impl Query {
    /// The version of the registry app that is answering.
    async fn api_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
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
}

impl From<accounts::Account> for Account {
    fn from(account: accounts::Account) -> Self {
        Self {
            id: ID(account.id.to_string()),
            email: account.email,
            family_name: account.family_name,
        }
    }
}

/// The root of every GraphQL mutation.
pub struct Mutation;

#[Object]
impl Mutation {
    /// Creates an account from an email and a family name. Surrounding
    /// whitespace is trimmed from both. Errors carry `extensions.code`:
    /// `INVALID_EMAIL`, `INVALID_FAMILY_NAME` (empty or over 200 characters),
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
        let internal = || coded_error("Something went wrong. Try again later.", "INTERNAL");
        let store = ctx.data::<AccountStore>().map_err(|_| internal())?;
        match store.create(email, family_name).await {
            Ok(account) => Ok(account.into()),
            Err(StoreError::DuplicateEmail) => Err(coded_error(
                "An account with this email already exists.",
                "EMAIL_TAKEN",
            )),
            Err(_) => Err(coded_error(
                "Something went wrong. Try again later.",
                "INTERNAL",
            )),
        }
    }
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

/// Builds the GraphQL schema, with `store` as shared data for the resolvers.
pub fn schema(store: AccountStore) -> RegistrySchema {
    Schema::build(Query, Mutation, EmptySubscription)
        .data(store)
        .finish()
}

/// Builds the HTTP app: `POST /graphql` and `GET /healthz`.
pub fn app(store: AccountStore) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/graphql", post_service(GraphQL::new(schema(store))))
}
