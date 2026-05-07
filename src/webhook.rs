//! Verify HMAC-SHA256 signatures on inbound Lettermint webhook deliveries.
//!
//! Lettermint signs each webhook with the shared secret you configured for the
//! endpoint. The `X-Lettermint-Signature` header carries the signed timestamp
//! and digest in the form `t=<unix_seconds>,v1=<hex_hmac_sha256>`. Verification
//! re-computes the HMAC over `"{timestamp}.{raw_body}"` and rejects messages
//! whose timestamp is more than 5 minutes off (configurable via
//! [`Webhook::with_tolerance`]).
//!
//! Use [`Webhook::verify`] when you only need the parsed JSON body, or
//! [`Webhook::verify_headers`] to also surface the event type, delivery
//! timestamp, and retry attempt as a [`WebhookEvent`].

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// Default timestamp tolerance in seconds (5 minutes).
const DEFAULT_TOLERANCE: u64 = 300;

/// Errors that can occur during webhook construction or verification.
#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("webhook secret must not be empty")]
    EmptySecret,

    #[error("invalid signature format: {0}")]
    InvalidFormat(String),

    #[error("signature mismatch")]
    InvalidSignature,

    #[error("timestamp outside tolerance window ({tolerance}s)")]
    TimestampTolerance { tolerance: u64 },

    /// The signature was valid but the body was not parseable as JSON.
    #[error("invalid JSON payload: {0}")]
    JsonDecode(#[from] serde_json::Error),

    /// The host clock is set before the Unix epoch, so we can't compare
    /// timestamps. Almost always indicates a misconfigured machine.
    #[error("system clock is set before Unix epoch")]
    SystemClock,
}

/// A verified webhook event with metadata from Lettermint headers.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    /// The parsed JSON payload.
    pub payload: serde_json::Value,
    /// Event type from `X-Lettermint-Event` (e.g., `message.delivered`).
    pub event: Option<String>,
    /// Delivery timestamp from `X-Lettermint-Delivery`.
    pub delivery_timestamp: Option<u64>,
    /// Retry attempt number from `X-Lettermint-Attempt` (starts at 1).
    pub attempt: Option<u32>,
}

/// Webhook verifier for Lettermint webhook payloads.
///
/// ```no_run
/// # use lettermint::webhook::Webhook;
/// # fn handler(body: &str, signature_header: &str) -> Result<(), Box<dyn std::error::Error>> {
/// let wh = Webhook::new("whsec_your_secret")?;
/// let payload = wh.verify(body, signature_header)?;
/// println!("verified event: {payload}");
/// # Ok(())
/// # }
/// ```
pub struct Webhook {
    secret: String,
    tolerance: u64,
}

impl Webhook {
    /// Create a new webhook verifier.
    ///
    /// Errors if `secret` is empty.
    pub fn new(secret: impl Into<String>) -> Result<Self, WebhookError> {
        let secret = secret.into();
        if secret.is_empty() {
            return Err(WebhookError::EmptySecret);
        }
        Ok(Self {
            secret,
            tolerance: DEFAULT_TOLERANCE,
        })
    }

    /// Create a new webhook verifier with a custom timestamp tolerance in seconds.
    ///
    /// Errors if `secret` is empty.
    pub fn with_tolerance(secret: impl Into<String>, tolerance: u64) -> Result<Self, WebhookError> {
        let secret = secret.into();
        if secret.is_empty() {
            return Err(WebhookError::EmptySecret);
        }
        Ok(Self { secret, tolerance })
    }

    /// Verify a webhook payload using the `X-Lettermint-Signature` header value.
    ///
    /// The signature header format is: `t=<timestamp>,v1=<hex_digest>`
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "lettermint.webhook.verify", skip_all)
    )]
    pub fn verify(
        &self,
        payload: &str,
        signature_header: &str,
    ) -> Result<serde_json::Value, WebhookError> {
        let (timestamp, signature) = parse_signature_header(signature_header)?;
        verify_signature(
            payload,
            &signature,
            &self.secret,
            Some(timestamp),
            self.tolerance,
        )?;
        Ok(serde_json::from_str(payload)?)
    }

    /// Verify using HTTP headers and return a [`WebhookEvent`] with metadata.
    ///
    /// Headers:
    /// - `X-Lettermint-Signature` (required) — `t=<ts>,v1=<hash>`
    /// - `X-Lettermint-Delivery` (optional) — delivery timestamp, cross-validated against signature
    /// - `X-Lettermint-Event` (optional) — event type (e.g., `message.delivered`)
    /// - `X-Lettermint-Attempt` (optional) — retry attempt number
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "lettermint.webhook.verify_headers",
            skip_all,
            fields(event = event_header, attempt = attempt_header),
        )
    )]
    pub fn verify_headers(
        &self,
        signature_header: &str,
        delivery_header: Option<&str>,
        event_header: Option<&str>,
        attempt_header: Option<&str>,
        payload: &str,
    ) -> Result<WebhookEvent, WebhookError> {
        let (timestamp, signature) = parse_signature_header(signature_header)?;

        let delivery_timestamp = if let Some(delivery) = delivery_header {
            let delivery_ts: u64 = delivery
                .parse()
                .map_err(|_| WebhookError::InvalidFormat("invalid delivery timestamp".into()))?;
            if delivery_ts != timestamp {
                return Err(WebhookError::InvalidFormat(
                    "signature timestamp does not match delivery header".into(),
                ));
            }
            Some(delivery_ts)
        } else {
            None
        };

        let attempt = attempt_header.and_then(|a| a.parse::<u32>().ok());

        verify_signature(
            payload,
            &signature,
            &self.secret,
            Some(timestamp),
            self.tolerance,
        )?;

        Ok(WebhookEvent {
            payload: serde_json::from_str(payload)?,
            event: event_header.map(String::from),
            delivery_timestamp,
            attempt,
        })
    }
}

/// Parse `t=<timestamp>,v1=<signature>` from the header.
fn parse_signature_header(header: &str) -> Result<(u64, String), WebhookError> {
    let mut timestamp = None;
    let mut signature = None;

    for part in header.split(',') {
        let part = part.trim();
        if let Some(ts) = part.strip_prefix("t=") {
            timestamp = Some(ts.parse::<u64>().map_err(|_| {
                WebhookError::InvalidFormat("invalid timestamp in signature".into())
            })?);
        } else if let Some(sig) = part.strip_prefix("v1=") {
            signature = Some(sig.to_string());
        }
    }

    match (timestamp, signature) {
        (Some(ts), Some(sig)) => Ok((ts, sig)),
        _ => Err(WebhookError::InvalidFormat(
            "missing t= or v1= in signature header".into(),
        )),
    }
}

/// Core signature verification.
fn verify_signature(
    payload: &str,
    expected_signature: &str,
    secret: &str,
    timestamp: Option<u64>,
    tolerance: u64,
) -> Result<(), WebhookError> {
    // Check timestamp tolerance
    if let Some(ts) = timestamp {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| WebhookError::SystemClock)?
            .as_secs();
        if now.abs_diff(ts) > tolerance {
            return Err(WebhookError::TimestampTolerance { tolerance });
        }
    }

    // Compute HMAC-SHA256 of "{timestamp}.{payload}"
    let signed_content = match timestamp {
        Some(ts) => format!("{ts}.{payload}"),
        None => payload.to_string(),
    };

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(signed_content.as_bytes());

    let expected_bytes = hex::decode(expected_signature)
        .map_err(|_| WebhookError::InvalidFormat("invalid hex in signature".into()))?;
    mac.verify_slice(&expected_bytes)
        .map_err(|_| WebhookError::InvalidSignature)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signature(payload: &str, secret: &str, timestamp: u64) -> String {
        let signed = format!("{timestamp}.{payload}");
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(signed.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        format!("t={timestamp},v1={sig}")
    }

    #[test]
    fn valid_signature() {
        let secret = "test-secret";
        let payload = r#"{"event":"delivered"}"#;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let header = make_signature(payload, secret, now);
        let wh = Webhook::new(secret).unwrap();
        let result = wh.verify(payload, &header);
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_signature() {
        let payload = r#"{"event":"delivered"}"#;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let header = make_signature(payload, "correct-secret", now);
        let wh = Webhook::new("wrong-secret").unwrap();
        let result = wh.verify(payload, &header);
        assert!(matches!(result, Err(WebhookError::InvalidSignature)));
    }

    #[test]
    fn expired_timestamp() {
        let secret = "test-secret";
        let payload = r#"{"event":"delivered"}"#;
        let old_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 600; // 10 minutes ago

        let header = make_signature(payload, secret, old_ts);
        let wh = Webhook::new(secret).unwrap();
        let result = wh.verify(payload, &header);
        assert!(matches!(
            result,
            Err(WebhookError::TimestampTolerance { .. })
        ));
    }

    #[test]
    fn parse_signature_header_valid() {
        let (ts, sig) = parse_signature_header("t=1234567890,v1=abcdef").unwrap();
        assert_eq!(ts, 1234567890);
        assert_eq!(sig, "abcdef");
    }

    #[test]
    fn parse_signature_header_missing_parts() {
        assert!(parse_signature_header("t=123").is_err());
        assert!(parse_signature_header("v1=abc").is_err());
        assert!(parse_signature_header("garbage").is_err());
    }

    #[test]
    fn custom_tolerance() {
        let secret = "test-secret";
        let payload = r#"{"event":"delivered"}"#;
        let old_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 60; // 1 minute ago

        let header = make_signature(payload, secret, old_ts);

        // Default tolerance (300s) should pass
        let wh = Webhook::new(secret).unwrap();
        assert!(wh.verify(payload, &header).is_ok());

        // Tight tolerance (10s) should fail
        let wh_tight = Webhook::with_tolerance(secret, 10).unwrap();
        assert!(matches!(
            wh_tight.verify(payload, &header),
            Err(WebhookError::TimestampTolerance { .. })
        ));
    }

    #[test]
    fn verify_headers_with_event_metadata() {
        let secret = "test-secret";
        let payload = r#"{"event":"delivered"}"#;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let sig_header = make_signature(payload, secret, now);
        let wh = Webhook::new(secret).unwrap();

        let event = wh
            .verify_headers(
                &sig_header,
                Some(&now.to_string()),
                Some("message.delivered"),
                Some("1"),
                payload,
            )
            .unwrap();

        assert_eq!(event.event.as_deref(), Some("message.delivered"));
        assert_eq!(event.delivery_timestamp, Some(now));
        assert_eq!(event.attempt, Some(1));
        assert_eq!(event.payload["event"], "delivered");
    }

    #[test]
    fn verify_headers_without_optional_headers() {
        let secret = "test-secret";
        let payload = r#"{"event":"delivered"}"#;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let sig_header = make_signature(payload, secret, now);
        let wh = Webhook::new(secret).unwrap();

        let event = wh
            .verify_headers(&sig_header, None, None, None, payload)
            .unwrap();

        assert!(event.event.is_none());
        assert!(event.delivery_timestamp.is_none());
        assert!(event.attempt.is_none());
    }

    #[test]
    fn empty_secret_returns_error() {
        assert!(matches!(Webhook::new(""), Err(WebhookError::EmptySecret)));
    }

    #[test]
    fn empty_secret_with_tolerance_returns_error() {
        assert!(matches!(
            Webhook::with_tolerance("", 300),
            Err(WebhookError::EmptySecret)
        ));
    }
}
