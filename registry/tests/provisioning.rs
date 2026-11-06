//! Account provisioning: `createAccount` starts it, `accountStatus` reports it.
//! Runs against the real Fly mock process and small stub servers.

use std::time::{Duration, Instant};

use axum::Router;
use registry::{accounts::AccountStore, provisioning::Provisioner};
use serde_json::{Value, json};

mod common;

use common::{Log, Reply};

const CREATE: &str = "mutation($email: String!, $familyName: String!) { \
    createAccount(email: $email, familyName: $familyName) { id email familyName status address } }";
const STATUS: &str = "query($id: ID!) { accountStatus(id: $id) { id status address } }";
const EMAIL: &str = "ana@example.com";
const FAMILY: &str = "Silva";
const PORT: u16 = 18093;
const VOLUME: &str =
    r#"{"id":"vol_abc","name":"family_data","region":"jnb","size_gb":1,"state":"created"}"#;
/// Long enough for the mock to start a family app.
const MOCK_LIMIT: Duration = Duration::from_secs(60);
const STUB_LIMIT: Duration = Duration::from_secs(10);

fn machine(state: &str, private_ip: &str) -> String {
    json!({
        "id": "m_123", "name": "n", "state": state, "region": "jnb",
        "private_ip": private_ip, "config": { "image": "i" }
    })
    .to_string()
}

/// A stub that answers create-app, create-volume and create-Machine.
async fn fly_stub(app: Reply, volume: Reply, machine: Reply) -> (String, Log) {
    common::stub(move |path| {
        if path.ends_with("/volumes") {
            volume.clone()
        } else if path.ends_with("/machines") {
            machine.clone()
        } else {
            app.clone()
        }
    })
    .await
}

fn ok(body: &str) -> Reply {
    Reply::With(200, body.to_owned())
}

fn server_error() -> Reply {
    Reply::With(500, r#"{"error":"boom"}"#.to_owned())
}

struct Setup {
    store: AccountStore,
    provisioner: Option<Provisioner>,
}

impl Setup {
    async fn new(name: &str, provisioner: Option<Provisioner>) -> Self {
        Self {
            store: common::fresh_store(&format!("provisioning-{name}")).await,
            provisioner,
        }
    }

    fn app(&self) -> Router {
        registry::app(self.store.clone(), self.provisioner.clone())
    }

    /// Creates an account and returns the `createAccount` result.
    async fn create(&self) -> Value {
        let variables = json!({ "email": EMAIL, "familyName": FAMILY });
        let json = common::graphql(self.app(), CREATE, variables).await;
        assert!(json.get("errors").is_none(), "unexpected errors: {json}");
        json["data"]["createAccount"].clone()
    }

    async fn status(&self, id: &str) -> Value {
        let json = common::graphql(self.app(), STATUS, json!({ "id": id })).await;
        assert!(json.get("errors").is_none(), "unexpected errors: {json}");
        json["data"]["accountStatus"].clone()
    }

    /// Polls `accountStatus` until it is no longer `PROVISIONING`.
    async fn settled(&self, id: &str, limit: Duration) -> Value {
        let deadline = Instant::now() + limit;
        loop {
            let status = self.status(id).await;
            if status["status"] != "PROVISIONING" {
                return status;
            }
            assert!(Instant::now() < deadline, "still provisioning: {status}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Creates an account and waits for provisioning to settle.
    async fn create_and_settle(&self, limit: Duration) -> (i64, Value) {
        let created = self.create().await;
        assert_eq!(created["status"], "PROVISIONING");
        assert!(created["address"].is_null());
        let id = created["id"].as_str().unwrap();
        let settled = self.settled(id, limit).await;
        (id.parse().unwrap(), settled)
    }
}

fn assert_failed(status: &Value) {
    assert_eq!(status["status"], "FAILED", "{status}");
    assert!(status["address"].is_null(), "{status}");
}

#[tokio::test]
async fn an_account_becomes_ready_on_the_fly_mock() {
    let mock = common::Mock::start();
    let provisioner = common::provisioner_at(&mock.base_url, common::MOCK_MACHINE_PORT, MOCK_LIMIT);
    let setup = Setup::new("mock-ready", Some(provisioner)).await;

    let created = setup.create().await;
    assert_eq!(created["status"], "PROVISIONING");
    assert!(created["address"].is_null());
    assert_eq!(created["email"], EMAIL);
    let id = created["id"].as_str().unwrap();
    let status = setup.settled(id, MOCK_LIMIT).await;

    assert_eq!(status["status"], "READY", "{status}");
    assert_eq!(status["id"], id);
    let address = status["address"].as_str().unwrap();
    let url = reqwest::Url::parse(address).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.port(), Some(common::MOCK_MACHINE_PORT));
    assert_eq!(url.path(), "/");
    url.host_str()
        .unwrap()
        .trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
        .unwrap();
    let health = reqwest::get(format!("{address}/healthz")).await.unwrap();
    assert_eq!(health.status(), 200);
    assert_eq!(health.text().await.unwrap(), "ok");

    let stored = setup.store.get(id.parse().unwrap()).await.unwrap().unwrap();
    assert_eq!(
        stored.fly_app_name.as_deref(),
        Some(format!("family-{id}").as_str())
    );
    assert!(stored.fly_volume_id.unwrap().starts_with("vol_"));
    assert!(!stored.fly_machine_id.unwrap().is_empty());
    assert_eq!(stored.address.as_deref(), Some(address));
}

#[tokio::test]
async fn requests_go_in_order_with_the_volume_mounted_and_the_env_set() {
    let (url, log) = fly_stub(ok("{}"), ok(VOLUME), ok(&machine("started", "10.0.0.7"))).await;
    let setup = Setup::new(
        "stub-order",
        Some(common::provisioner_at(&url, PORT, STUB_LIMIT)),
    )
    .await;

    let (id, status) = setup.create_and_settle(STUB_LIMIT).await;

    assert_eq!(status["status"], "READY", "{status}");
    assert_eq!(status["address"], format!("http://10.0.0.7:{PORT}"));
    let seen = log.lock().unwrap().clone();
    let app_name = format!("family-{id}");
    let paths: Vec<&str> = seen.iter().map(|seen| seen.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/v1/apps".to_owned(),
            format!("/v1/apps/{app_name}/volumes"),
            format!("/v1/apps/{app_name}/machines"),
        ]
    );
    let app: Value = serde_json::from_slice(&seen[0].body).unwrap();
    let name = app["name"].as_str().unwrap();
    assert_eq!(name, app_name);
    for personal in [EMAIL, "ana", FAMILY, "silva"] {
        assert!(!name.contains(personal), "{name}");
    }
    let volume: Value = serde_json::from_slice(&seen[1].body).unwrap();
    let machine: Value = serde_json::from_slice(&seen[2].body).unwrap();
    assert_eq!(machine["region"], volume["region"]);
    assert_eq!(
        machine["config"]["mounts"],
        json!([{ "volume": "vol_abc", "path": "/data" }])
    );
    assert_eq!(machine["config"]["env"]["FAMILY_APP_DATA_DIR"], "/data");
    assert_eq!(
        machine["config"]["env"]["FAMILY_APP_ADDR"],
        format!("[::]:{PORT}")
    );

    let stored = setup.store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.fly_volume_id.as_deref(), Some("vol_abc"));
    assert_eq!(stored.fly_machine_id.as_deref(), Some("m_123"));
}

#[tokio::test]
async fn an_ipv6_private_ip_is_bracketed_in_the_address() {
    let (url, _) = fly_stub(ok("{}"), ok(VOLUME), ok(&machine("started", "fdaa::3"))).await;
    let setup = Setup::new(
        "stub-ipv6",
        Some(common::provisioner_at(&url, PORT, STUB_LIMIT)),
    )
    .await;

    let (_, status) = setup.create_and_settle(STUB_LIMIT).await;

    assert_eq!(status["status"], "READY", "{status}");
    assert_eq!(status["address"], format!("http://[fdaa::3]:{PORT}"));
}

async fn assert_stub_fails(name: &str, app: Reply, volume: Reply, machine: Reply) -> (Setup, i64) {
    let (url, _) = fly_stub(app, volume, machine).await;
    let provisioner = common::provisioner_at(&url, PORT, Duration::from_millis(500));
    let setup = Setup::new(name, Some(provisioner)).await;
    let (id, status) = setup.create_and_settle(STUB_LIMIT).await;
    assert_failed(&status);
    let stored = setup.store.get(id).await.unwrap().unwrap();
    assert!(stored.address.is_none());
    (setup, id)
}

#[tokio::test]
async fn a_500_from_create_app_fails_the_account() {
    let (setup, id) = assert_stub_fails("app-500", server_error(), ok(VOLUME), ok("{}")).await;
    let stored = setup.store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.fly_app_name, None);
}

#[tokio::test]
async fn a_500_from_create_volume_fails_the_account() {
    let (setup, id) = assert_stub_fails("volume-500", ok("{}"), server_error(), ok("{}")).await;
    let stored = setup.store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.fly_app_name, Some(format!("family-{id}")));
    assert_eq!(stored.fly_volume_id, None);
}

#[tokio::test]
async fn a_500_from_create_machine_fails_the_account_and_keeps_earlier_ids() {
    let (setup, id) = assert_stub_fails("machine-500", ok("{}"), ok(VOLUME), server_error()).await;
    let stored = setup.store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.fly_app_name, Some(format!("family-{id}")));
    assert_eq!(stored.fly_volume_id.as_deref(), Some("vol_abc"));
    assert_eq!(stored.fly_machine_id, None);
}

#[tokio::test]
async fn a_machine_that_is_only_created_fails_the_account() {
    let created = ok(&machine("created", "10.0.0.7"));
    let (setup, id) = assert_stub_fails("machine-created", ok("{}"), ok(VOLUME), created).await;
    let stored = setup.store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.fly_machine_id.as_deref(), Some("m_123"));
}

#[tokio::test]
async fn an_unparsable_private_ip_fails_the_account() {
    let bad_ip = ok(&machine("started", "not-an-ip"));
    assert_stub_fails("machine-bad-ip", ok("{}"), ok(VOLUME), bad_ip).await;
}

#[tokio::test]
async fn a_stub_that_never_answers_fails_the_account_at_the_time_limit() {
    let started = Instant::now();
    assert_stub_fails("never", Reply::Never, Reply::Never, Reply::Never).await;
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn a_closed_port_fails_the_account() {
    let url = common::closed_port_url().await;
    let provisioner = common::provisioner_at(&url, PORT, STUB_LIMIT);
    let setup = Setup::new("closed-port", Some(provisioner)).await;

    let (_, status) = setup.create_and_settle(STUB_LIMIT).await;

    assert_failed(&status);
}

#[tokio::test]
async fn account_status_is_null_for_unknown_and_malformed_ids() {
    let setup = Setup::new("status-unknown", None).await;

    for id in ["1", "999", "abc", "", "1.5", "-1"] {
        assert!(setup.status(id).await.is_null(), "id {id:?}");
    }
}

#[tokio::test]
async fn account_status_cannot_ask_for_personal_data() {
    let setup = Setup::new("status-personal", None).await;

    for field in ["email", "familyName"] {
        let query = format!("{{ accountStatus(id: \"1\") {{ id {field} }} }}");
        let json = common::graphql(setup.app(), &query, json!({})).await;
        let errors = json["errors"].as_array().expect("errors");
        assert!(
            errors[0]["message"].as_str().unwrap().contains(field),
            "{json}"
        );
        assert!(json.get("data").is_none_or(Value::is_null), "{json}");
    }
}

#[tokio::test]
async fn account_provisioning_has_only_id_status_and_address() {
    let query = r#"{ __type(name: "AccountProvisioning") { fields { name } } }"#;
    let setup = Setup::new("status-fields", None).await;

    let json = common::graphql(setup.app(), query, json!({})).await;

    let mut names: Vec<&str> = json["data"]["__type"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["address", "id", "status"]);
}

#[tokio::test]
async fn without_fly_create_account_is_unavailable_and_stores_nothing() {
    let path = common::fresh_db("provisioning-unavailable");
    let store = AccountStore::open(&path).await.unwrap();
    let variables = json!({ "email": EMAIL, "familyName": FAMILY });

    let json = common::graphql(registry::app(store.clone(), None), CREATE, variables).await;

    assert!(json["data"].is_null(), "{json}");
    let errors = json["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["extensions"]["code"], "PROVISIONING_UNAVAILABLE");
    let message = errors[0]["message"].as_str().unwrap();
    assert!(!message.contains(EMAIL) && !message.contains(FAMILY));
    assert!(store.get(1).await.unwrap().is_none());
}
