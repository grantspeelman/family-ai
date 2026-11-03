//! The Fly API client, against the real Fly mock process and small stub servers.
//!
//! The mock runs as its own process, as it does in dev and e2e: no crate links
//! against `fly-mock`, so these tests start it with `cargo run`. It starts a real
//! family app per Machine, so they build that binary first, and stop the mock
//! with SIGTERM, because it can't stop its family apps on a SIGKILL.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{BufRead, BufReader},
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use axum::{
    Router,
    body::Bytes,
    extract::{Request, State},
    http::{
        StatusCode,
        header::{AUTHORIZATION, LOCATION},
    },
};
use registry::fly::{
    CreateMachineRequest, CreateVolumeRequest, FlyClient, FlyConfig, FlyError, MachineConfig, Mount,
};
use serde_json::{Value, json};

const TOKEN: &str = "test-token-abc123";
const LISTENING: &str = "fly-mock: listening on ";
/// Long enough for `cargo run` to build the mock on a cold cache.
const MOCK_START_TIMEOUT: Duration = Duration::from_secs(300);
const MACHINE_BODY: &str = r#"{"id":"m1","name":"n","state":"created","region":"r",
    "private_ip":"fdaa::1","config":{"image":"i"}}"#;

/// A running `fly-mock` process, killed on drop.
struct Mock {
    child: Child,
    base_url: String,
    data_dir: PathBuf,
}

impl Mock {
    fn start() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let built = Command::new(&cargo)
            .args(["build", "--quiet", "--locked", "-p", "family-app"])
            .current_dir(manifest_dir)
            .status()
            .expect("cannot run cargo");
        assert!(built.success(), "cannot build family-app");
        let data_dir = std::env::temp_dir().join(format!(
            "registry-fly-mock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut child = Command::new(cargo)
            .args(["run", "--quiet", "--locked", "-p", "fly-mock"])
            .current_dir(manifest_dir)
            .env("FLY_MOCK_ADDR", "127.0.0.1:0")
            .env("FLY_MOCK_DATA_DIR", &data_dir)
            // Not 8080, so a server on this host's wildcard address can't block it.
            .env("FLY_MOCK_MACHINE_PORT", "18082")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("cannot run cargo");
        let stderr = child.stderr.take().unwrap();
        let (addr_tx, addr_rx) = mpsc::channel();
        // Keep reading until the process exits, so stderr never fills up.
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                match line.strip_prefix(LISTENING) {
                    Some(addr) => {
                        let _ = addr_tx.send(addr.to_owned());
                    }
                    // Shows why, if cargo cannot build or run the mock.
                    None => eprintln!("{line}"),
                }
            }
        });
        let addr = addr_rx
            .recv_timeout(MOCK_START_TIMEOUT)
            .expect("fly-mock did not start");
        Self {
            child,
            base_url: format!("http://{addr}"),
            data_dir,
        }
    }

    fn client(&self) -> FlyClient {
        client(&self.base_url, None)
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        // `cargo run` replaces itself with the mock, so this signals the mock.
        let _ = Command::new("kill")
            .arg(self.child.id().to_string())
            .status();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

fn client(base_url: &str, token: Option<&str>) -> FlyClient {
    let config = FlyConfig::new(base_url, token.map(str::to_owned)).unwrap();
    FlyClient::new(config).unwrap()
}

#[derive(Debug)]
struct Seen {
    path: String,
    authorization: Option<String>,
    body: Bytes,
}

type Log = Arc<Mutex<Vec<Seen>>>;

/// Starts a server that answers everything with `status` and `body`, recording
/// each request. Returns its base URL and the record.
async fn stub(status: u16, body: &'static str) -> (String, Log) {
    async fn handle(
        State((status, body, log)): State<(u16, &'static str, Log)>,
        request: Request,
    ) -> (StatusCode, &'static str) {
        let (parts, request_body) = request.into_parts();
        let bytes = axum::body::to_bytes(request_body, usize::MAX)
            .await
            .unwrap();
        log.lock().unwrap().push(Seen {
            path: parts.uri.path().to_owned(),
            authorization: parts
                .headers
                .get(AUTHORIZATION)
                .map(|value| value.to_str().unwrap().to_owned()),
            body: bytes,
        });
        (StatusCode::from_u16(status).unwrap(), body)
    }

    let log = Log::default();
    let app = Router::new()
        .fallback(handle)
        .with_state((status, body, log.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), log)
}

fn volume_request() -> CreateVolumeRequest {
    CreateVolumeRequest {
        name: "data".into(),
        region: "jnb".into(),
        size_gb: 1,
    }
}

fn machine_request(volume: Option<&str>) -> CreateMachineRequest {
    CreateMachineRequest {
        name: Some("web".into()),
        region: Some("jnb".into()),
        config: MachineConfig {
            image: "family-app:dev".into(),
            env: Some(BTreeMap::from([("KEY".to_owned(), "value".to_owned())])),
            mounts: volume.map(|volume| {
                vec![Mount {
                    volume: volume.into(),
                    path: "/data".into(),
                }]
            }),
        },
    }
}

fn bare_machine_request() -> CreateMachineRequest {
    CreateMachineRequest {
        name: None,
        region: None,
        config: MachineConfig {
            image: "family-app:dev".into(),
            env: None,
            mounts: None,
        },
    }
}

fn assert_not_found_with_message(error: FlyError) {
    match error {
        FlyError::NotFound { message } => assert!(!message.is_empty()),
        other => panic!("expected not found, got {other:?}"),
    }
}

#[tokio::test]
async fn creates_an_app_a_volume_and_a_machine_on_the_mock() {
    let mock = Mock::start();
    let fly = mock.client();

    fly.create_app("fam-1", "personal").await.unwrap();
    let volume = fly.create_volume("fam-1", &volume_request()).await.unwrap();
    assert!(volume.id.starts_with("vol_"), "{}", volume.id);
    assert_eq!(volume.name, "data");
    assert_eq!(volume.region, "jnb");
    assert_eq!(volume.size_gb, 1);
    assert_eq!(volume.state, "created");

    let request = machine_request(Some(&volume.id));
    let machine = fly.create_machine("fam-1", &request).await.unwrap();
    assert_eq!(machine.id.len(), 14);
    assert!(machine.id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(machine.name, "web");
    assert_eq!(machine.region, "jnb");
    assert_eq!(machine.state, "started");
    machine.private_ip.parse::<IpAddr>().unwrap();
    assert_eq!(machine.config, request.config);
}

#[tokio::test]
async fn an_unknown_app_or_volume_on_the_mock_is_not_found() {
    let mock = Mock::start();
    let fly = mock.client();

    let error = fly
        .create_volume("nope", &volume_request())
        .await
        .unwrap_err();
    assert_not_found_with_message(error);
    let error = fly
        .create_machine("nope", &bare_machine_request())
        .await
        .unwrap_err();
    assert_not_found_with_message(error);

    fly.create_app("fam-1", "personal").await.unwrap();
    let request = machine_request(Some("vol_unknown"));
    let error = fly.create_machine("fam-1", &request).await.unwrap_err();
    assert_not_found_with_message(error);
}

#[tokio::test]
async fn a_taken_app_name_on_the_mock_is_an_api_error() {
    let mock = Mock::start();
    let fly = mock.client();

    fly.create_app("fam-1", "personal").await.unwrap();
    match fly.create_app("fam-1", "personal").await.unwrap_err() {
        FlyError::Api { status, message } => {
            assert_eq!(status, 422);
            assert!(!message.is_empty());
        }
        other => panic!("expected an API error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_500_in_fly_error_shape_keeps_status_and_message() {
    let (base, _) = stub(500, r#"{"error": "boom"}"#).await;
    match client(&base, None).create_app("a", "o").await.unwrap_err() {
        FlyError::Api { status, message } => {
            assert_eq!(status, 500);
            assert_eq!(message, "boom");
        }
        other => panic!("expected an API error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_500_with_a_non_json_body_is_still_an_api_error() {
    let (base, _) = stub(500, "<html>bad gateway</html>").await;
    let error = client(&base, None).create_app("a", "o").await.unwrap_err();
    assert!(
        matches!(error, FlyError::Api { status: 500, .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_redirect_is_an_api_error_and_is_not_followed() {
    let (target, target_log) = stub(200, "").await;
    let location = format!("{target}/elsewhere");
    let app =
        Router::new().fallback(move || async move { (StatusCode::FOUND, [(LOCATION, location)]) });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let error = client(&base, Some(TOKEN))
        .create_app("a", "o")
        .await
        .unwrap_err();

    assert!(
        matches!(error, FlyError::Api { status: 302, .. }),
        "{error:?}"
    );
    assert!(target_log.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_app_name_that_is_not_a_path_segment_is_rejected_without_a_request() {
    let (base, log) = stub(200, MACHINE_BODY).await;
    let fly = client(&base, None);

    for name in ["", ".", ".."] {
        let errors = [
            fly.create_app(name, "o").await.unwrap_err(),
            fly.create_volume(name, &volume_request())
                .await
                .unwrap_err(),
            fly.create_machine(name, &bare_machine_request())
                .await
                .unwrap_err(),
        ];
        for error in errors {
            assert!(matches!(error, FlyError::InvalidAppName), "{error:?}");
        }
    }
    assert!(log.lock().unwrap().is_empty());
}

#[test]
fn an_invalid_hostname_error_names_the_variable_and_an_example_but_not_the_value() {
    for value in ["bad value", "localhost:4280"] {
        let text =
            FlyConfig::from_lookup(|name| (name == "FLY_API_HOSTNAME").then(|| value.into()))
                .unwrap_err()
                .to_string();
        assert!(text.contains("FLY_API_HOSTNAME"), "{text}");
        assert!(text.contains("http://127.0.0.1:4280"), "{text}");
        assert!(!text.contains(value), "{text}");
    }
}

#[tokio::test]
async fn an_empty_error_message_leaves_no_trailing_separator() {
    let (base, _) = stub(500, "not json").await;
    let error = client(&base, None).create_app("a", "o").await.unwrap_err();
    assert_eq!(error.to_string(), "Fly API returned 500");
}

#[tokio::test]
async fn a_200_that_is_not_a_machine_is_a_decode_error() {
    let (base, _) = stub(200, r#"{"not": "a machine"}"#).await;
    let error = client(&base, None)
        .create_machine("a", &bare_machine_request())
        .await
        .unwrap_err();
    assert!(matches!(error, FlyError::Decode { .. }), "{error:?}");
}

#[tokio::test]
async fn response_fields_the_client_does_not_list_are_ignored() {
    let body = r#"{"id":"m1","name":"n","state":"created","region":"r","private_ip":"fdaa::1",
        "instance_id":"x","config":{"image":"i","guest":{"cpus":1},
        "mounts":[{"volume":"v","path":"/d","size_gb":1}]}}"#;
    let (base, _) = stub(200, body).await;
    let machine = client(&base, None)
        .create_machine("a", &bare_machine_request())
        .await
        .unwrap();
    assert_eq!(machine.id, "m1");
    assert_eq!(machine.config.image, "i");
}

#[tokio::test]
async fn a_closed_port_is_a_transport_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let base = format!("http://{addr}");
    let error = client(&base, None).create_app("a", "o").await.unwrap_err();
    assert!(matches!(error, FlyError::Transport { .. }), "{error:?}");
    let text = error.to_string();
    assert!(text.contains(&base), "{text}");
    assert!(text.contains("refused"), "{text}");
}

#[tokio::test]
async fn a_token_is_sent_as_a_bearer_header_on_every_call() {
    let (base, log) = stub(200, MACHINE_BODY).await;
    let fly = client(&base, Some(TOKEN));

    fly.create_app("a", "o").await.unwrap();
    let _ = fly.create_volume("a", &volume_request()).await;
    fly.create_machine("a", &bare_machine_request())
        .await
        .unwrap();

    let seen = log.lock().unwrap();
    assert_eq!(seen.len(), 3);
    let expected = format!("Bearer {TOKEN}");
    for request in seen.iter() {
        assert_eq!(request.authorization.as_deref(), Some(expected.as_str()));
    }
}

#[tokio::test]
async fn without_a_token_no_authorization_header_is_sent() {
    let (base, log) = stub(200, MACHINE_BODY).await;
    let fly = client(&base, None);

    fly.create_app("a", "o").await.unwrap();
    let _ = fly.create_volume("a", &volume_request()).await;
    fly.create_machine("a", &bare_machine_request())
        .await
        .unwrap();

    let seen = log.lock().unwrap();
    assert_eq!(seen.len(), 3);
    assert!(seen.iter().all(|request| request.authorization.is_none()));
}

#[tokio::test]
async fn the_token_is_not_in_debug_or_error_text() {
    let config = FlyConfig::new("http://127.0.0.1:1", Some(TOKEN.to_owned())).unwrap();
    assert!(!format!("{config:?}").contains(TOKEN));
    let closed = FlyClient::new(config).unwrap();
    assert!(!format!("{closed:?}").contains(TOKEN));

    const ECHO: &str = r#"{"error": "bad token test-token-abc123"}"#;
    // A wrong type makes serde quote the value in its message.
    const WRONG_TYPE: &str = r#"{"size_gb": "test-token-abc123"}"#;
    assert!(ECHO.contains(TOKEN) && WRONG_TYPE.contains(TOKEN));
    let mut errors = vec![closed.create_app("a", "o").await.unwrap_err()];
    for (status, body) in [
        (404, ECHO),
        (500, ECHO),
        (500, "x"),
        (200, "x"),
        (200, WRONG_TYPE),
    ] {
        let (base, _) = stub(status, body).await;
        let fly = client(&base, Some(TOKEN));
        errors.push(fly.create_volume("a", &volume_request()).await.unwrap_err());
    }
    errors.push(FlyConfig::new("not a url", Some(TOKEN.to_owned())).unwrap_err());
    let bad_token = FlyConfig::new("http://127.0.0.1:1", Some(format!("{TOKEN}\n"))).unwrap();
    errors.push(FlyClient::new(bad_token).unwrap_err());

    assert_eq!(errors.len(), 8);
    for error in errors {
        assert!(!error.to_string().contains(TOKEN), "{error}");
        assert!(!format!("{error:?}").contains(TOKEN), "{error:?}");
    }
}

#[test]
fn a_missing_or_invalid_base_url_is_a_config_error() {
    let lookup = |value: Option<&'static str>| {
        move |name: &str| match name {
            "FLY_API_HOSTNAME" => value.map(OsString::from),
            _ => None,
        }
    };
    for value in [
        None,
        Some(""),
        Some("not a url"),
        Some("localhost:4280"),
        Some("ftp://host"),
    ] {
        let error = FlyConfig::from_lookup(lookup(value)).unwrap_err();
        assert!(
            matches!(error, FlyError::Config { .. }),
            "{value:?}: {error:?}"
        );
    }
}

#[test]
fn a_base_url_with_credentials_is_a_config_error() {
    let error = FlyConfig::new("http://user:secret@host", None).unwrap_err();
    assert!(matches!(error, FlyError::Config { .. }), "{error:?}");
    assert!(!error.to_string().contains("secret"), "{error}");
}

#[test]
fn the_config_reads_both_environment_variables() {
    let config = FlyConfig::from_lookup(|name| match name {
        "FLY_API_HOSTNAME" => Some("https://api.machines.dev".into()),
        "FLY_API_TOKEN" => Some(TOKEN.into()),
        other => panic!("unexpected variable {other}"),
    })
    .unwrap();
    assert_eq!(config.base_url(), "https://api.machines.dev/");
    assert!(config.has_token());

    let config =
        FlyConfig::from_lookup(|name| (name == "FLY_API_HOSTNAME").then(|| "http://host".into()))
            .unwrap();
    assert!(!config.has_token());
}

#[tokio::test]
async fn a_trailing_slash_gives_the_same_request_paths() {
    let (base, log) = stub(200, MACHINE_BODY).await;
    for base in [base.clone(), format!("{base}/")] {
        let fly = client(&base, None);
        fly.create_app("a", "o").await.unwrap();
        let _ = fly.create_volume("a", &volume_request()).await;
        fly.create_machine("a", &bare_machine_request())
            .await
            .unwrap();
    }

    let seen = log.lock().unwrap();
    let paths: Vec<_> = seen.iter().map(|request| request.path.as_str()).collect();
    let expected = ["/v1/apps", "/v1/apps/a/volumes", "/v1/apps/a/machines"];
    assert_eq!(paths[..3], expected);
    assert_eq!(paths[3..], expected);
}

#[tokio::test]
async fn an_app_name_cannot_change_the_request_path() {
    let (base, log) = stub(200, MACHINE_BODY).await;
    let fly = client(&base, None);
    let _ = fly.create_volume("../x?y", &volume_request()).await;

    let seen = log.lock().unwrap();
    assert_eq!(seen[0].path, "/v1/apps/..%2Fx%3Fy/volumes");
}

#[tokio::test]
async fn request_bodies_use_flys_field_names_and_leave_out_unset_fields() {
    let (base, log) = stub(200, MACHINE_BODY).await;
    let fly = client(&base, None);

    fly.create_app("a", "o").await.unwrap();
    let _ = fly.create_volume("a", &volume_request()).await;
    fly.create_machine("a", &bare_machine_request())
        .await
        .unwrap();
    fly.create_machine("a", &machine_request(Some("vol_1")))
        .await
        .unwrap();

    let seen = log.lock().unwrap();
    let bodies: Vec<Value> = seen
        .iter()
        .map(|request| serde_json::from_slice(&request.body).unwrap())
        .collect();
    assert_eq!(bodies[0], json!({"name": "a", "org_slug": "o"}));
    assert_eq!(
        bodies[1],
        json!({"name": "data", "region": "jnb", "size_gb": 1})
    );
    assert_eq!(bodies[2], json!({"config": {"image": "family-app:dev"}}));
    assert_eq!(
        bodies[3],
        json!({"name": "web", "region": "jnb", "config": {
            "image": "family-app:dev",
            "env": {"KEY": "value"},
            "mounts": [{"volume": "vol_1", "path": "/data"}],
        }})
    );
}
