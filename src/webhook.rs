use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

/// Default timestamp tolerance in seconds (5 minutes).
const DEFAULT_TOLERANCE: u64 = 300;

/// Errors that can occur during webhook verification.
#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("invalid signature format: {0}")]
    InvalidFormat(String),

    #[error("signature mismatch")]
    InvalidSignature,

    #[error("timestamp outside tolerance window ({tolerance}s)")]
    TimestampTolerance { tolerance: u64 },

    #[error("invalid JSON payload: {0}")]
    JsonDecode(#[from] serde_json::Error),

    #[error("system clock is set before Unix epoch")]
    SystemClock,
}

/// Webhook verifier for Lettermint webhook payloads.
///
/// ```
/// # use lettermint::webhook::Webhook;
/// let wh = Webhook::new("whsec_your_secret");
///
/// // Verify using raw signature header
/// // let payload = wh.verify(body, signature_header).unwrap();
/// ```
pub struct Webhook {
    secret: String,
    tolerance: u64,
}

impl Webhook {
    /// Create a new webhook verifier.
    ///
    /// # Panics
    ///
    /// Panics if `secret` is empty.
    pub fn new(secret: impl Into<String>) -> Self {
        let secret = secret.into();
        assert!(!secret.is_empty(), "webhook secret must not be empty");
        Self {
            secret,
            tolerance: DEFAULT_TOLERANCE,
        }
    }

    /// Create a new webhook verifier with a custom timestamp tolerance in seconds.
    ///
    /// # Panics
    ///
    /// Panics if `secret` is empty.
    pub fn with_tolerance(secret: impl Into<String>, tolerance: u64) -> Self {
        let secret = secret.into();
        assert!(!secret.is_empty(), "webhook secret must not be empty");
        Self { secret, tolerance }
    }

    /// Verify a webhook payload using the `X-Lettermint-Signature` header value.
    ///
    /// The signature header format is: `t=<timestamp>,v1=<hex_digest>`
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

    /// Verify using HTTP headers directly.
    ///
    /// Looks for `X-Lettermint-Signature` and optionally `X-Lettermint-Delivery`.
    pub fn verify_headers(
        &self,
        signature_header: &str,
        delivery_header: Option<&str>,
        payload: &str,
    ) -> Result<serde_json::Value, WebhookError> {
        let (timestamp, signature) = parse_signature_header(signature_header)?;

        // Cross-validate with delivery header if present
        if let Some(delivery) = delivery_header {
            let delivery_ts: u64 = delivery
                .parse()
                .map_err(|_| WebhookError::InvalidFormat("invalid delivery timestamp".into()))?;
            if delivery_ts != timestamp {
                return Err(WebhookError::InvalidFormat(
                    "signature timestamp does not match delivery header".into(),
                ));
            }
        }

        verify_signature(
            payload,
            &signature,
            &self.secret,
            Some(timestamp),
            self.tolerance,
        )?;
        Ok(serde_json::from_str(payload)?)
    }

    /// One-off verification without instantiating a Webhook.
    ///
    /// Uses the default tolerance of 300 seconds (5 minutes).
    pub fn verify_once(
        payload: &str,
        signature_header: &str,
        secret: &str,
    ) -> Result<serde_json::Value, WebhookError> {
        let (ts, sig) = parse_signature_header(signature_header)?;
        verify_signature(payload, &sig, secret, Some(ts), DEFAULT_TOLERANCE)?;
        Ok(serde_json::from_str(payload)?)
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
        let wh = Webhook::new(secret);
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
        let wh = Webhook::new("wrong-secret");
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
        let wh = Webhook::new(secret);
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
        let wh = Webhook::new(secret);
        assert!(wh.verify(payload, &header).is_ok());

        // Tight tolerance (10s) should fail
        let wh_tight = Webhook::with_tolerance(secret, 10);
        assert!(matches!(
            wh_tight.verify(payload, &header),
            Err(WebhookError::TimestampTolerance { .. })
        ));
    }

    #[test]
    #[should_panic(expected = "webhook secret must not be empty")]
    fn empty_secret_panics() {
        Webhook::new("");
    }

    #[test]
    #[should_panic(expected = "webhook secret must not be empty")]
    fn empty_secret_with_tolerance_panics() {
        Webhook::with_tolerance("", 300);
    }
}
