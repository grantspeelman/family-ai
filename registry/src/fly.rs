//! A client for Fly's Machines API: create an app, a volume and a Machine.
//!
//! Configure it with [`FlyConfig`], which reads `FLY_API_HOSTNAME` and
//! `FLY_API_TOKEN`. In dev and CI the hostname points at the Fly mock. Pointing
//! it at real Fly changes only that base URL.

use std::{collections::BTreeMap, ffi::OsString, fmt, time::Duration};

use reqwest::{
    Client, RequestBuilder, Response, Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
    redirect,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const HOSTNAME_VAR: &str = "FLY_API_HOSTNAME";
const TOKEN_VAR: &str = "FLY_API_TOKEN";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const REDACTED: &str = "[redacted]";
const EXAMPLE_BASE_URL: &str = "http://127.0.0.1:4280";

/// Where the client sends requests, and the token it sends with them.
///
/// [`FlyConfig::from_env`] reads the base URL from `FLY_API_HOSTNAME`, which is
/// required and has no default, so nothing calls real Fly by accident, and the
/// optional API token from `FLY_API_TOKEN`. `Debug` output hides the token.
#[derive(Clone)]
pub struct FlyConfig {
    base_url: Url,
    api_token: Option<String>,
}

impl FlyConfig {
    /// Builds a config from an `http` or `https` base URL, with or without a
    /// trailing slash. An empty token counts as no token.
    pub fn new(base_url: &str, api_token: Option<String>) -> Result<Self, FlyError> {
        let base_url = parse_base_url(base_url)
            .map_err(|why| config_error(format!("the Fly API base URL {why}")))?;
        Ok(Self::from_parts(base_url, api_token))
    }

    /// Reads `FLY_API_HOSTNAME` and `FLY_API_TOKEN` from the process environment.
    pub fn from_env() -> Result<Self, FlyError> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// Reads the same variables as [`FlyConfig::from_env`] through `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, FlyError> {
        let read = |name: &str| {
            lookup(name)
                .map(|value| {
                    value
                        .into_string()
                        .map_err(|_| config_error(format!("{name} is not valid UTF-8")))
                })
                .transpose()
        };
        let base_url = read(HOSTNAME_VAR)?
            .filter(|value| !value.is_empty())
            .ok_or_else(|| config_error(format!("{HOSTNAME_VAR} is not set")))?;
        let base_url = parse_base_url(&base_url).map_err(|why| {
            config_error(format!(
                "{HOSTNAME_VAR} {why}; set it to a value such as {EXAMPLE_BASE_URL}"
            ))
        })?;
        Ok(Self::from_parts(base_url, read(TOKEN_VAR)?))
    }

    fn from_parts(base_url: Url, api_token: Option<String>) -> Self {
        Self {
            base_url,
            api_token: api_token.filter(|token| !token.is_empty()),
        }
    }

    /// The base URL requests go to.
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    /// Whether requests carry a token.
    pub fn has_token(&self) -> bool {
        self.api_token.is_some()
    }
}

impl fmt::Debug for FlyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlyConfig")
            .field("base_url", &self.base_url.as_str())
            .field("api_token", &self.api_token.as_ref().map(|_| REDACTED))
            .finish()
    }
}

/// Parses the base URL, or says why it is unusable. The reason never quotes the
/// value, which could hold a secret if it was set by mistake.
fn parse_base_url(value: &str) -> Result<Url, &'static str> {
    let url = Url::parse(value).map_err(|_| "is not a valid URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return Err("must be an http or https URL with a host");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("must not contain credentials");
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("must not have a query or fragment");
    }
    Ok(url)
}

fn config_error(message: String) -> FlyError {
    FlyError::Config { message }
}

/// What went wrong with a Fly API call, so the caller can decide what to do.
///
/// [`FlyError::Config`] covers a bad `FLY_API_HOSTNAME` or `FLY_API_TOKEN`. The
/// text of these errors never contains the `FLY_API_TOKEN` value.
#[derive(Debug)]
pub enum FlyError {
    /// Fly returned `404`. Keeps the message from Fly's `{"error": "..."}` body,
    /// or an empty message if the body has a different shape.
    NotFound { message: String },
    /// Fly returned any other non-2xx status. Keeps the status and the message,
    /// which is empty if the body is not in Fly's error shape.
    Api { status: u16, message: String },
    /// The request could not be sent, or no complete response came back. The
    /// message has the base URL and the cause.
    Transport { message: String },
    /// A 2xx response whose body cannot be read in full or does not match the
    /// expected type. The message says how, and can quote the body, with any
    /// token removed.
    Decode { message: String },
    /// `FLY_API_HOSTNAME` is missing or not a valid URL, or the config cannot be
    /// used, for example because `FLY_API_TOKEN` cannot go in a header.
    Config { message: String },
    /// An app name that is empty, `.` or `..`, which cannot be a URL path segment.
    InvalidAppName,
}

impl fmt::Display for FlyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { message } if message.is_empty() => {
                write!(f, "Fly API returned 404")
            }
            Self::NotFound { message } => write!(f, "Fly API returned 404: {message}"),
            Self::Api { status, message } if message.is_empty() => {
                write!(f, "Fly API returned {status}")
            }
            Self::Api { status, message } => write!(f, "Fly API returned {status}: {message}"),
            Self::Transport { message } => write!(f, "cannot reach the Fly API: {message}"),
            Self::Decode { message } => write!(f, "unexpected Fly API response: {message}"),
            Self::Config { message } => write!(f, "invalid Fly API config: {message}"),
            Self::InvalidAppName => write!(f, "a Fly app name cannot be empty, `.` or `..`"),
        }
    }
}

impl std::error::Error for FlyError {}

/// The body of a create-volume call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CreateVolumeRequest {
    pub name: String,
    pub region: String,
    pub size_gb: u32,
}

/// A volume as Fly reports it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub region: String,
    pub size_gb: u32,
    pub state: String,
}

/// The body of a create-Machine call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CreateMachineRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub config: MachineConfig,
}

/// What a Machine runs. Sent in requests and returned in responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineConfig {
    pub image: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<Mount>>,
}

/// A volume mounted into a Machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mount {
    /// The volume's `id`, not its name.
    pub volume: String,
    /// Where the volume appears inside the Machine.
    pub path: String,
}

/// A Machine as Fly reports it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub state: String,
    pub region: String,
    pub private_ip: String,
    pub config: MachineConfig,
}

#[derive(Serialize)]
struct CreateAppRequest<'a> {
    name: &'a str,
    org_slug: &'a str,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

/// An async client for the Fly API. `Debug` output hides the token.
#[derive(Clone)]
pub struct FlyClient {
    http: Client,
    config: FlyConfig,
}

impl FlyClient {
    /// Builds a client that sends the config's token, if any, on every request.
    /// It follows no redirects, so the token only goes to the configured host,
    /// and a 3xx response is an API error.
    pub fn new(config: FlyConfig) -> Result<Self, FlyError> {
        let mut headers = HeaderMap::new();
        if let Some(token) = &config.api_token {
            let mut value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                config_error(format!("{TOKEN_VAR} cannot be sent in an HTTP header"))
            })?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }
        let http = Client::builder()
            .default_headers(headers)
            .redirect(redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| config_error("cannot build the HTTP client".to_owned()))?;
        Ok(Self { http, config })
    }

    /// Creates an app. Succeeds on any 2xx and ignores the response body.
    pub async fn create_app(&self, app_name: &str, org_slug: &str) -> Result<(), FlyError> {
        check_app_name(app_name)?;
        let body = CreateAppRequest {
            name: app_name,
            org_slug,
        };
        let request = self.post(&["v1", "apps"]).json(&body);
        self.send(request).await.map(drop)
    }

    /// Creates a volume in an app.
    pub async fn create_volume(
        &self,
        app_name: &str,
        request: &CreateVolumeRequest,
    ) -> Result<Volume, FlyError> {
        check_app_name(app_name)?;
        let request = self
            .post(&["v1", "apps", app_name, "volumes"])
            .json(request);
        self.decode(self.send(request).await?).await
    }

    /// Creates a Machine in an app.
    pub async fn create_machine(
        &self,
        app_name: &str,
        request: &CreateMachineRequest,
    ) -> Result<Machine, FlyError> {
        check_app_name(app_name)?;
        let request = self
            .post(&["v1", "apps", app_name, "machines"])
            .json(request);
        self.decode(self.send(request).await?).await
    }

    /// Appends percent-encoded segments to the base URL, so a trailing slash on
    /// the base makes no difference. `url` drops `.` and `..` segments, so
    /// callers pass app names through [`check_app_name`] first.
    fn post(&self, segments: &[&str]) -> RequestBuilder {
        let mut url = self.config.base_url.clone();
        url.path_segments_mut()
            .expect("the config only accepts http and https URLs")
            .pop_if_empty()
            .extend(segments);
        self.http.post(url)
    }

    async fn send(&self, request: RequestBuilder) -> Result<Response, FlyError> {
        let response = request
            .send()
            .await
            .map_err(|error| self.transport_error(&error))?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let message = match response.json::<ErrorBody>().await {
            Ok(body) => self.redact(body.error),
            Err(_) => String::new(),
        };
        Err(match status.as_u16() {
            404 => FlyError::NotFound { message },
            status => FlyError::Api { status, message },
        })
    }

    /// reqwest reports a body that cannot be read in full as a decode error
    /// too, so every error here is [`FlyError::Decode`].
    async fn decode<T: DeserializeOwned>(&self, response: Response) -> Result<T, FlyError> {
        response.json().await.map_err(|error| FlyError::Decode {
            message: self.redact(causes(&error)),
        })
    }

    fn transport_error(&self, error: &reqwest::Error) -> FlyError {
        let base_url = self.config.base_url();
        FlyError::Transport {
            message: self.redact(format!("{base_url}: {}", causes(error))),
        }
    }

    /// Removes the token from text that came from elsewhere, such as a server
    /// echoing the header.
    fn redact(&self, message: String) -> String {
        match &self.config.api_token {
            Some(token) => message.replace(token.as_str(), REDACTED),
            None => message,
        }
    }
}

impl fmt::Debug for FlyClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlyClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

fn check_app_name(app_name: &str) -> Result<(), FlyError> {
    if matches!(app_name, "" | "." | "..") {
        return Err(FlyError::InvalidAppName);
    }
    Ok(())
}

/// The error and everything that caused it, on one line and without the URL.
fn causes(error: &reqwest::Error) -> String {
    let mut text = match error.url() {
        Some(url) => error.to_string().replace(&format!(" for url ({url})"), ""),
        None => error.to_string(),
    };
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        text = format!("{text}: {cause}");
        source = cause.source();
    }
    text
}
