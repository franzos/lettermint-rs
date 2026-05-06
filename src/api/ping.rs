use crate::Endpoint;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Ping the Lettermint API to verify connectivity and credentials.
///
/// ```
/// # use lettermint::api::ping::PingRequest;
/// let req = PingRequest;
/// ```
pub struct PingRequest;

#[doc(hidden)]
#[derive(Debug, Serialize)]
pub struct EmptyBody;

/// Response from the ping endpoint.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PingResponse {
    pub message: String,
}

impl Endpoint for PingRequest {
    type Request = EmptyBody;
    type Response = PingResponse;

    fn endpoint(&self) -> Cow<'static, str> {
        "ping".into()
    }

    fn body(&self) -> &Self::Request {
        static BODY: EmptyBody = EmptyBody;
        &BODY
    }

    fn method(&self) -> http::Method {
        http::Method::GET
    }

    fn parse_response(&self, body: &[u8]) -> Result<Self::Response, serde_json::Error> {
        let message = String::from_utf8_lossy(body).trim().to_owned();
        Ok(PingResponse { message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_path_and_method() {
        let req = PingRequest;
        assert_eq!(req.endpoint(), "ping");
        assert_eq!(req.method(), http::Method::GET);
    }

    #[test]
    fn parse_response_from_plain_text() {
        let resp = PingRequest.parse_response(b"pong").unwrap();
        assert_eq!(resp.message, "pong");
    }

    #[test]
    fn parse_response_trims_whitespace() {
        let resp = PingRequest.parse_response(b"pong\n").unwrap();
        assert_eq!(resp.message, "pong");
    }
}
