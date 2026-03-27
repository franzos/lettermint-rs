use std::convert::TryInto;
use std::time::Duration;

use crate::{Client, Endpoint, Query, QueryError, LETTERMINT_API_URL};
use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, Response};
use thiserror::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!("Lettermint/", env!("CARGO_PKG_VERSION"), " (Rust)");

/// A reqwest-based Lettermint API client.
///
/// ```
/// # use lettermint::reqwest::LettermintClient;
/// let client = LettermintClient::new("your-api-token");
/// ```
///
/// With a custom base URL:
/// ```
/// # use lettermint::reqwest::LettermintClient;
/// let client = LettermintClient::with_base_url("your-api-token", "https://custom.api/v1/");
/// ```
///
/// With a pre-configured reqwest client:
/// ```
/// # use lettermint::reqwest::LettermintClient;
/// let http_client = reqwest::Client::builder()
///     .timeout(std::time::Duration::from_secs(60))
///     .build()
///     .unwrap();
/// let client = LettermintClient::with_reqwest_client("your-api-token", http_client);
/// ```
#[derive(Clone)]
pub struct LettermintClient {
    api_token: String,
    base_url: String,
    client: ::reqwest::Client,
}

fn default_reqwest_client() -> ::reqwest::Client {
    ::reqwest::Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .expect("default reqwest client should build")
}

impl LettermintClient {
    /// Create a new client with the default Lettermint API URL and a 30s timeout.
    pub fn new(api_token: impl Into<String>) -> Self {
        Self {
            api_token: api_token.into(),
            base_url: LETTERMINT_API_URL.into(),
            client: default_reqwest_client(),
        }
    }

    /// Create a new client with a custom base URL.
    ///
    /// The URL should include the API version path (e.g., `https://api.lettermint.co/v1/`).
    pub fn with_base_url(api_token: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_token: api_token.into(),
            base_url: base_url.into(),
            client: default_reqwest_client(),
        }
    }

    /// Create a new client with a pre-configured `reqwest::Client`.
    ///
    /// Use this when you need custom timeouts, proxy settings, or TLS configuration.
    pub fn with_reqwest_client(api_token: impl Into<String>, client: ::reqwest::Client) -> Self {
        Self {
            api_token: api_token.into(),
            base_url: LETTERMINT_API_URL.into(),
            client,
        }
    }

    pub async fn execute_endpoint<T>(
        &self,
        request: T,
    ) -> Result<T::Response, QueryError<LettermintClientError>>
    where
        T: Endpoint + Send + Sync,
    {
        request.execute(self).await
    }
}

impl std::fmt::Debug for LettermintClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LettermintClient")
            .field("api_token", &"***")
            .field("base_url", &self.base_url)
            .finish()
    }
}

#[derive(Error, Debug)]
pub enum LettermintClientError {
    #[error("error setting auth header: {}", source)]
    AuthError {
        #[from]
        source: http::header::InvalidHeaderValue,
    },
    #[error("communication with lettermint: {}", source)]
    Communication {
        #[from]
        source: ::reqwest::Error,
    },
    #[error("http error: {}", source)]
    Http {
        #[from]
        source: http::Error,
    },
    #[error("invalid uri: {}", source)]
    InvalidUri {
        #[from]
        source: http::uri::InvalidUri,
    },
}

#[async_trait]
impl Client for LettermintClient {
    type Error = LettermintClientError;

    async fn execute(&self, mut req: Request<Bytes>) -> Result<Response<Bytes>, Self::Error> {
        req.headers_mut()
            .append("x-lettermint-token", self.api_token.as_str().try_into()?);

        // Build URL by joining base_url and the endpoint path, avoiding Url::join
        // pitfalls with leading slashes and missing trailing slashes.
        let path = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("");
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );

        *req.uri_mut() = url.parse()?;

        let reqwest_req: ::reqwest::Request = req.try_into()?;
        let reqwest_rsp = self.client.execute(reqwest_req).await?;

        let mut rsp = Response::builder()
            .status(reqwest_rsp.status())
            .version(reqwest_rsp.version());

        let headers = rsp
            .headers_mut()
            .expect("response builder should have headers");
        for (k, v) in reqwest_rsp.headers() {
            headers.insert(k, v.clone());
        }

        Ok(rsp.body(reqwest_rsp.bytes().await?)?)
    }
}
