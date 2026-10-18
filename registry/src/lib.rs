//! The registry app: a GraphQL API served over HTTP.

use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema};
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

pub type RegistrySchema = Schema<Query, EmptyMutation, EmptySubscription>;

/// Builds the GraphQL schema.
pub fn schema() -> RegistrySchema {
    Schema::build(Query, EmptyMutation, EmptySubscription).finish()
}

/// Builds the HTTP app: `POST /graphql` and `GET /healthz`.
pub fn app() -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/graphql", post_service(GraphQL::new(schema())))
}
