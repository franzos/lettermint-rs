use std::borrow::Cow;
use std::future::Future;

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use std::error::Error;
use thiserror::Error;

/// A trait for providing the necessary information for a single REST API endpoint.
pub trait Endpoint {
    type Request: serde::Serialize + Send + Sync;
    type Response: serde::de::DeserializeOwned + Send + Sync;

    /// The path to the endpoint.
    fn endpoint(&self) -> Cow<'static, str>;
    /// The body for the endpoint.
    fn body(&self) -> &Self::Request;
    /// The HTTP method for the endpoint.
    fn method(&self) -> http::Method {
        http::Method::POST
    }
    /// Optional extra headers (e.g., Idempotency-Key).
    fn extra_headers(&self) -> Vec<(Cow<'static, str>, Cow<'static, str>)> {
        vec![]
    }
    /// Parse the response body. Override for non-JSON responses.
    fn parse_response(&self, body: &[u8]) -> Result<Self::Response, serde_json::Error> {
        serde_json::from_slice(body)
    }
}

/// A trait which represents an asynchronous query which may be made to a Lettermint client.
pub trait Query<C> {
    type Result;
    /// Perform the query against the client.
    fn execute(self, client: &C) -> impl Future<Output = Self::Result> + Send;
}

/// An error thrown by the [`Query`] trait.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum QueryError<E>
where
    E: Error + Send + Sync + 'static,
{
    /// Underlying HTTP client error (DNS, TLS, timeout, header build, ...).
    #[error("client error: {}", source)]
    Client { source: E },

    /// The request body could not be serialized to JSON.
    #[error("failed to serialize request body: {}", source)]
    SerializeBody { source: serde_json::Error },

    /// A successful response body could not be parsed as JSON into the
    /// endpoint's [`Endpoint::Response`] type.
    #[error("could not parse JSON response: {}", source)]
    DeserializeResponse { source: serde_json::Error },

    /// Failed to construct the HTTP request (e.g., invalid header value or URI).
    #[error("failed to build request: {}", source)]
    Body {
        #[from]
        source: http::Error,
    },

    /// Validation error (HTTP 422) with per-field details.
    #[error("validation error: {message:?}")]
    Validation {
        error_type: Option<String>,
        message: Option<String>,
        /// Per-field validation errors (e.g., `{"from": ["domain not verified"]}`)
        errors: Option<std::collections::HashMap<String, Vec<String>>>,
        body: Bytes,
    },

    /// Authentication or authorization error (HTTP 401/403).
    #[error("authentication error: {message:?}")]
    Authentication {
        message: Option<String>,
        body: Bytes,
    },

    /// Rate limit exceeded (HTTP 429).
    #[error("rate limit exceeded: {message:?}")]
    RateLimit {
        message: Option<String>,
        body: Bytes,
    },

    /// Any other non-success API response.
    #[error("api error: status={status}, error_type={error_type:?}, message={message:?}")]
    Api {
        status: StatusCode,
        error_type: Option<String>,
        message: Option<String>,
        body: Bytes,
    },
}

impl<E> QueryError<E>
where
    E: Error + Send + Sync + 'static,
{
    /// Wrap an underlying client error in [`QueryError::Client`].
    pub fn client(source: E) -> Self {
        QueryError::Client { source }
    }
}

impl<T, C> Query<C> for T
where
    T: Endpoint + Send + Sync,
    C: Client + Send + Sync,
{
    type Result = Result<T::Response, QueryError<C::Error>>;

    async fn execute(self, client: &C) -> Self::Result {
        let method = self.method();
        let endpoint = self.endpoint();

        #[cfg(feature = "tracing")]
        let span = tracing::debug_span!(
            "lettermint.request",
            method = %method,
            endpoint = %endpoint,
            status = tracing::field::Empty,
        );

        let fut = async move {
            // Ensure the path starts with '/' so http::Uri parses it as a valid path.
            let uri = if endpoint.starts_with('/') {
                endpoint.into_owned()
            } else {
                format!("/{endpoint}")
            };
            let mut req_builder = http::Request::builder()
                .method(method.clone())
                .uri(uri)
                .header("Accept", "application/json");

            for (name, value) in self.extra_headers() {
                req_builder = req_builder.header(name.as_ref(), value.as_ref());
            }

            let body = match method {
                http::Method::GET | http::Method::DELETE | http::Method::HEAD => Bytes::new(),
                _ => {
                    req_builder = req_builder.header("Content-Type", "application/json");
                    serde_json::to_vec(self.body())
                        .map_err(|e| QueryError::SerializeBody { source: e })?
                        .into()
                }
            };

            let http_req = req_builder.body(body)?;
            let response = client.execute(http_req).await.map_err(QueryError::client)?;

            let status = response.status();
            #[cfg(feature = "tracing")]
            tracing::Span::current().record("status", status.as_u16());

            if !status.is_success() {
                #[derive(serde::Deserialize)]
                struct LettermintErrorBody {
                    error_type: Option<String>,
                    error: Option<String>,
                    message: Option<String>,
                    errors: Option<std::collections::HashMap<String, Vec<String>>>,
                }

                let body = response.body().clone();
                let parsed = serde_json::from_slice::<LettermintErrorBody>(&body).ok();
                let error_type = parsed
                    .as_ref()
                    .and_then(|p| p.error_type.clone().or_else(|| p.error.clone()));
                let message = parsed.as_ref().and_then(|p| p.message.clone());

                #[cfg(feature = "tracing")]
                tracing::warn!(
                    status = status.as_u16(),
                    error_type = error_type.as_deref(),
                    message = message.as_deref(),
                    "API error response",
                );

                return Err(match status.as_u16() {
                    422 => QueryError::Validation {
                        error_type,
                        message,
                        errors: parsed.and_then(|p| p.errors),
                        body,
                    },
                    401 | 403 => QueryError::Authentication { message, body },
                    429 => QueryError::RateLimit { message, body },
                    _ => QueryError::Api {
                        status,
                        error_type,
                        message,
                        body,
                    },
                });
            }

            self.parse_response(response.body())
                .map_err(|e| QueryError::DeserializeResponse { source: e })
        };

        #[cfg(feature = "tracing")]
        {
            use tracing::Instrument;
            fut.instrument(span).await
        }
        #[cfg(not(feature = "tracing"))]
        {
            fut.await
        }
    }
}

/// A trait representing a client which can communicate with a Lettermint instance.
pub trait Client {
    type Error: Error + Send + Sync + 'static;
    /// Send the request and return the response. Implementors are responsible
    /// for setting auth headers and resolving the relative URI to the API base
    /// URL.
    fn execute(
        &self,
        req: Request<Bytes>,
    ) -> impl Future<Output = Result<Response<Bytes>, Self::Error>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, thiserror::Error)]
    #[error("test client error")]
    struct MockClientError;

    #[derive(Clone)]
    struct MockClient {
        last_request: Arc<Mutex<Option<Request<Bytes>>>>,
        response_status: StatusCode,
        response_body: Bytes,
    }

    impl MockClient {
        fn ok(body: &'static [u8]) -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_status: StatusCode::OK,
                response_body: Bytes::from_static(body),
            }
        }

        fn error(status: StatusCode, body: &'static [u8]) -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_status: status,
                response_body: Bytes::from_static(body),
            }
        }

        fn last_request(&self) -> Request<Bytes> {
            self.last_request
                .lock()
                .expect("lock")
                .clone()
                .expect("request present")
        }
    }

    impl Client for MockClient {
        type Error = MockClientError;

        async fn execute(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Self::Error> {
            *self.last_request.lock().expect("lock") = Some(req);
            Ok(Response::builder()
                .status(self.response_status)
                .body(self.response_body.clone())
                .expect("response"))
        }
    }

    #[derive(serde::Serialize)]
    struct TestBody {
        value: &'static str,
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct TestResponse {
        ok: bool,
    }

    struct PostEndpoint {
        body: TestBody,
        extra: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    }

    impl PostEndpoint {
        fn new() -> Self {
            Self {
                body: TestBody { value: "hello" },
                extra: vec![],
            }
        }

        fn with_extra_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
            self.extra
                .push((Cow::Borrowed(name), Cow::Owned(value.into())));
            self
        }
    }

    impl Endpoint for PostEndpoint {
        type Request = TestBody;
        type Response = TestResponse;

        fn endpoint(&self) -> Cow<'static, str> {
            "send".into()
        }

        fn body(&self) -> &Self::Request {
            &self.body
        }

        fn extra_headers(&self) -> Vec<(Cow<'static, str>, Cow<'static, str>)> {
            self.extra.clone()
        }
    }

    #[derive(serde::Serialize)]
    struct NoBody;

    struct GetEndpoint;
    impl Endpoint for GetEndpoint {
        type Request = NoBody;
        type Response = TestResponse;

        fn endpoint(&self) -> Cow<'static, str> {
            "messages".into()
        }

        fn body(&self) -> &Self::Request {
            static BODY: NoBody = NoBody;
            &BODY
        }

        fn method(&self) -> http::Method {
            http::Method::GET
        }
    }

    #[tokio::test]
    async fn post_request_has_json_body_and_content_type() {
        let client = MockClient::ok(br#"{"ok":true}"#);
        let resp = PostEndpoint::new().execute(&client).await.expect("execute");
        assert!(resp.ok);

        let req = client.last_request();
        assert_eq!(req.method(), http::Method::POST);
        assert_eq!(req.body(), &Bytes::from_static(br#"{"value":"hello"}"#));
        assert_eq!(
            req.headers().get("Content-Type").unwrap().to_str().unwrap(),
            "application/json"
        );
        assert_eq!(
            req.headers().get("Accept").unwrap().to_str().unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn get_request_has_no_body_or_content_type() {
        let client = MockClient::ok(br#"{"ok":true}"#);
        let resp = GetEndpoint.execute(&client).await.expect("execute");
        assert!(resp.ok);

        let req = client.last_request();
        assert_eq!(req.method(), http::Method::GET);
        assert!(req.body().is_empty());
        assert!(req.headers().get("Content-Type").is_none());
        assert!(req.headers().get("Accept").is_some());
    }

    #[tokio::test]
    async fn extra_headers_are_applied() {
        let client = MockClient::ok(br#"{"ok":true}"#);
        PostEndpoint::new()
            .with_extra_header("Idempotency-Key", "test-key")
            .execute(&client)
            .await
            .expect("execute");

        let req = client.last_request();
        assert_eq!(
            req.headers()
                .get("Idempotency-Key")
                .unwrap()
                .to_str()
                .unwrap(),
            "test-key"
        );
    }

    #[tokio::test]
    async fn validation_error_422() {
        let client = MockClient::error(
            StatusCode::UNPROCESSABLE_ENTITY,
            br#"{"error_type":"DailyLimitExceeded","message":"Limit reached"}"#,
        );

        let err = PostEndpoint::new()
            .execute(&client)
            .await
            .expect_err("should fail");

        match err {
            QueryError::Validation {
                error_type,
                message,
                ..
            } => {
                assert_eq!(error_type.as_deref(), Some("DailyLimitExceeded"));
                assert_eq!(message.as_deref(), Some("Limit reached"));
            }
            _ => panic!("expected Validation error, got: {err:?}"),
        }
    }

    #[tokio::test]
    async fn authentication_error_401() {
        let client = MockClient::error(
            StatusCode::UNAUTHORIZED,
            br#"{"message":"Invalid API token"}"#,
        );

        let err = PostEndpoint::new()
            .execute(&client)
            .await
            .expect_err("should fail");

        match err {
            QueryError::Authentication { message, .. } => {
                assert_eq!(message.as_deref(), Some("Invalid API token"));
            }
            _ => panic!("expected Authentication error, got: {err:?}"),
        }
    }

    #[tokio::test]
    async fn rate_limit_error_429() {
        let client = MockClient::error(
            StatusCode::TOO_MANY_REQUESTS,
            br#"{"message":"Rate limit exceeded"}"#,
        );

        let err = PostEndpoint::new()
            .execute(&client)
            .await
            .expect_err("should fail");

        match err {
            QueryError::RateLimit { message, .. } => {
                assert_eq!(message.as_deref(), Some("Rate limit exceeded"));
            }
            _ => panic!("expected RateLimit error, got: {err:?}"),
        }
    }

    #[tokio::test]
    async fn api_error_with_non_json_body() {
        let client = MockClient::error(StatusCode::BAD_GATEWAY, b"gateway timeout");

        let err = PostEndpoint::new()
            .execute(&client)
            .await
            .expect_err("should fail");

        match err {
            QueryError::Api {
                status,
                error_type,
                message,
                body,
            } => {
                assert_eq!(status, StatusCode::BAD_GATEWAY);
                assert_eq!(error_type, None);
                assert_eq!(message, None);
                assert_eq!(body, Bytes::from_static(b"gateway timeout"));
            }
            _ => panic!("expected Api error, got: {err:?}"),
        }
    }

    #[tokio::test]
    async fn success_with_invalid_json_returns_deserialize_error() {
        let client = MockClient::ok(b"not json");
        let err = PostEndpoint::new()
            .execute(&client)
            .await
            .expect_err("should fail");

        assert!(matches!(err, QueryError::DeserializeResponse { .. }));
    }

    #[tokio::test]
    async fn api_error_with_error_field_fallback() {
        let client = MockClient::error(
            StatusCode::BAD_REQUEST,
            br#"{"error":"invalid_request","message":"Bad from address"}"#,
        );

        let err = PostEndpoint::new()
            .execute(&client)
            .await
            .expect_err("should fail");

        match err {
            QueryError::Api {
                status,
                error_type,
                message,
                ..
            } => {
                assert_eq!(status, StatusCode::BAD_REQUEST);
                assert_eq!(error_type.as_deref(), Some("invalid_request"));
                assert_eq!(message.as_deref(), Some("Bad from address"));
            }
            _ => panic!("expected Api error, got: {err:?}"),
        }
    }

    #[tokio::test]
    async fn authentication_error_403() {
        let client = MockClient::error(StatusCode::FORBIDDEN, br#"{"message":"Access denied"}"#);

        let err = PostEndpoint::new()
            .execute(&client)
            .await
            .expect_err("should fail");

        assert!(matches!(err, QueryError::Authentication { .. }));
    }
}
