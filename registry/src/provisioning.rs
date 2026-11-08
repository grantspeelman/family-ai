//! Provisioning: creates each new account's Fly app, volume and Machine in the
//! background, and records the outcome in the account store.
//!
//! The account becomes `ready` only when create-Machine returns a `started`
//! Machine with a `private_ip` that parses as an IP address. Its address is
//! then `http://<private_ip>:<machine port>`, with IPv6 in brackets. Anything
//! else makes it `failed`. Log lines carry the account id and the error, never
//! the email, the family name or the Fly token.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use crate::{
    accounts::{AccountStore, StoreError},
    fly::{CreateMachineRequest, CreateVolumeRequest, FlyClient, FlyError, MachineConfig, Mount},
};

/// The environment variable that holds the family app's port on its Machine.
pub const MACHINE_PORT_VAR: &str = "REGISTRY_FAMILY_APP_PORT";
/// The machine port when [`MACHINE_PORT_VAR`] is unset or empty. It must
/// match the Fly mock's `FLY_MOCK_MACHINE_PORT`.
pub const DEFAULT_MACHINE_PORT: u16 = 8080;
/// How long provisioning may take in total before the account fails.
pub const DEFAULT_TIME_LIMIT: Duration = Duration::from_secs(60);

/// Each app name is this prefix plus the account id, such as `family-42`.
pub const APP_NAME_PREFIX: &str = "family-";
/// The Fly organization that owns the apps.
pub const ORG_SLUG: &str = "personal";
/// The region of both the volume and the Machine.
pub const REGION: &str = "jnb";
/// The name of each family's data volume.
pub const VOLUME_NAME: &str = "family_data";
/// The size of each family's data volume.
pub const VOLUME_SIZE_GB: u32 = 1;
/// The family app image. The Fly mock ignores it.
pub const IMAGE: &str = "family-app:latest";
/// Where the volume is mounted inside the Machine.
pub const DATA_DIR: &str = "/data";

/// Reads the machine port from [`MACHINE_PORT_VAR`] through `lookup`. Unset or
/// empty gives [`DEFAULT_MACHINE_PORT`]. The error names the variable but
/// never quotes its value.
pub fn machine_port_from(lookup: impl FnOnce(&str) -> Option<OsString>) -> Result<u16, String> {
    let Some(value) = lookup(MACHINE_PORT_VAR).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_MACHINE_PORT);
    };
    value
        .to_str()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| format!("{MACHINE_PORT_VAR} must be a port number from 1 to 65535"))
}

/// What the background task needs to provision an account.
#[derive(Clone, Debug)]
pub struct Provisioner {
    client: FlyClient,
    machine_port: u16,
    time_limit: Duration,
}

impl Provisioner {
    /// A provisioner with the [`DEFAULT_TIME_LIMIT`].
    pub fn new(client: FlyClient, machine_port: u16) -> Self {
        Self {
            client,
            machine_port,
            time_limit: DEFAULT_TIME_LIMIT,
        }
    }

    /// Replaces the overall time limit, for example a short one in tests.
    pub fn with_time_limit(mut self, time_limit: Duration) -> Self {
        self.time_limit = time_limit;
        self
    }

    /// Starts provisioning the account in a background task and returns at once.
    pub fn spawn(&self, store: AccountStore, id: i64) {
        let provisioner = self.clone();
        tokio::spawn(async move { provisioner.run(&store, id).await });
    }

    async fn run(&self, store: &AccountStore, id: i64) {
        let outcome = match tokio::time::timeout(self.time_limit, self.steps(store, id)).await {
            Ok(outcome) => outcome,
            Err(_) => Err(Failure::TimedOut),
        };
        let recorded = match outcome {
            Ok(address) => store.mark_ready(id, &address).await,
            Err(failure) => {
                eprintln!("registry: provisioning account {id} failed: {failure}");
                store.mark_failed(id).await
            }
        };
        if let Err(error) = recorded {
            eprintln!("registry: cannot record provisioning of account {id}: {error}");
        }
    }

    async fn steps(&self, store: &AccountStore, id: i64) -> Result<String, Failure> {
        let app_name = format!("{APP_NAME_PREFIX}{id}");
        self.client.create_app(&app_name, ORG_SLUG).await?;
        store.set_fly_app_name(id, &app_name).await?;

        let volume = self
            .client
            .create_volume(
                &app_name,
                &CreateVolumeRequest {
                    name: VOLUME_NAME.to_owned(),
                    region: REGION.to_owned(),
                    size_gb: VOLUME_SIZE_GB,
                },
            )
            .await?;
        store.set_fly_volume_id(id, &volume.id).await?;

        let env = BTreeMap::from([
            ("FAMILY_APP_DATA_DIR".to_owned(), DATA_DIR.to_owned()),
            (
                "FAMILY_APP_ADDR".to_owned(),
                format!("[::]:{}", self.machine_port),
            ),
        ]);
        let request = CreateMachineRequest {
            name: None,
            region: Some(REGION.to_owned()),
            config: MachineConfig {
                image: IMAGE.to_owned(),
                env: Some(env),
                mounts: Some(vec![Mount {
                    volume: volume.id,
                    path: DATA_DIR.to_owned(),
                }]),
            },
        };
        let machine = self.client.create_machine(&app_name, &request).await?;
        store.set_fly_machine_id(id, &machine.id).await?;

        if machine.state != "started" {
            return Err(Failure::NotStarted(machine.state));
        }
        let ip: IpAddr = machine
            .private_ip
            .parse()
            .map_err(|_| Failure::BadPrivateIp)?;
        Ok(format!("http://{}", SocketAddr::new(ip, self.machine_port)))
    }
}

/// Why provisioning failed. The text never holds personal data or the token.
enum Failure {
    Fly(FlyError),
    Store(StoreError),
    NotStarted(String),
    BadPrivateIp,
    TimedOut,
}

impl From<FlyError> for Failure {
    fn from(error: FlyError) -> Self {
        Self::Fly(error)
    }
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fly(error) => write!(f, "{error}"),
            Self::Store(error) => write!(f, "{error}"),
            Self::NotStarted(state) => write!(f, "the Machine is {state:?}, not \"started\""),
            Self::BadPrivateIp => f.write_str("the Machine's private_ip is not an IP address"),
            Self::TimedOut => f.write_str("timed out"),
        }
    }
}
