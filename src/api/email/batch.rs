use crate::Endpoint;
use serde::Serialize;
use std::borrow::Cow;
use thiserror::Error;

use super::{SendEmailRequest, SendEmailResponse};

/// Error returned when constructing a [`BatchSendRequest`].
#[derive(Debug, Error)]
pub enum BatchError {
    #[error("batch must contain at least one email")]
    Empty,
    #[error("batch exceeds maximum size of {max} (got {actual})")]
    TooLarge { max: usize, actual: usize },
}

/// Send up to 500 emails in a single batch request.
///
/// ```
/// # use lettermint::api::email::{SendEmailRequest, BatchSendRequest};
/// let emails = vec![
///     SendEmailRequest::builder()
///         .from("sender@example.com")
///         .to(vec!["alice@example.com".into()])
///         .subject("Hello Alice")
///         .text("Hi Alice!")
///         .build(),
///     SendEmailRequest::builder()
///         .from("sender@example.com")
///         .to(vec!["bob@example.com".into()])
///         .subject("Hello Bob")
///         .text("Hi Bob!")
///         .build(),
/// ];
///
/// let batch = BatchSendRequest::new(emails).expect("batch must be 1-500 emails");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BatchSendRequest {
    emails: Vec<SendEmailRequest>,
    #[serde(skip)]
    idempotency_key: Option<String>,
}

/// Maximum number of emails per batch request.
pub const BATCH_MAX_SIZE: usize = 500;

impl BatchSendRequest {
    /// Create a new batch request.
    ///
    /// Errors if `emails` is empty or exceeds [`BATCH_MAX_SIZE`] (500).
    pub fn new(emails: Vec<SendEmailRequest>) -> Result<Self, BatchError> {
        if emails.is_empty() {
            return Err(BatchError::Empty);
        }
        if emails.len() > BATCH_MAX_SIZE {
            return Err(BatchError::TooLarge {
                max: BATCH_MAX_SIZE,
                actual: emails.len(),
            });
        }
        Ok(Self {
            emails,
            idempotency_key: None,
        })
    }

    /// Set an idempotency key to prevent duplicate batch sends.
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// The number of emails in this batch.
    pub fn len(&self) -> usize {
        self.emails.len()
    }

    /// Whether the batch is empty (always `false` after successful construction).
    pub fn is_empty(&self) -> bool {
        self.emails.is_empty()
    }
}

impl Endpoint for BatchSendRequest {
    type Request = BatchSendRequest;
    type Response = Vec<SendEmailResponse>;

    fn endpoint(&self) -> Cow<'static, str> {
        "send/batch".into()
    }

    fn body(&self) -> &Self::Request {
        self
    }

    fn extra_headers(&self) -> Vec<(Cow<'static, str>, Cow<'static, str>)> {
        let mut headers = vec![];
        if let Some(key) = &self.idempotency_key {
            headers.push((Cow::Borrowed("Idempotency-Key"), Cow::Owned(key.clone())));
        }
        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_email(to: &str) -> SendEmailRequest {
        SendEmailRequest::builder()
            .from("sender@example.com")
            .to(vec![to.into()])
            .subject("Test")
            .text("Hello")
            .build()
    }

    #[test]
    fn new_rejects_empty() {
        assert!(matches!(
            BatchSendRequest::new(vec![]),
            Err(BatchError::Empty)
        ));
    }

    #[test]
    fn new_rejects_over_500() {
        let emails: Vec<_> = (0..501)
            .map(|i| minimal_email(&format!("user{i}@example.com")))
            .collect();
        assert!(matches!(
            BatchSendRequest::new(emails),
            Err(BatchError::TooLarge {
                max: 500,
                actual: 501
            })
        ));
    }

    #[test]
    fn new_accepts_valid_batch() {
        let batch = BatchSendRequest::new(vec![
            minimal_email("a@example.com"),
            minimal_email("b@example.com"),
        ]);
        assert!(batch.is_ok());
        assert_eq!(batch.unwrap().len(), 2);
    }

    #[test]
    fn serializes_as_array() {
        let batch = BatchSendRequest::new(vec![
            minimal_email("a@example.com"),
            minimal_email("b@example.com"),
        ])
        .unwrap();

        let val = serde_json::to_value(&batch).unwrap();
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["to"], json!(["a@example.com"]));
        assert_eq!(arr[1]["to"], json!(["b@example.com"]));
    }

    #[test]
    fn endpoint_path() {
        let batch = BatchSendRequest::new(vec![minimal_email("a@example.com")]).unwrap();
        assert_eq!(batch.endpoint(), "send/batch");
    }

    #[test]
    fn idempotency_key_header() {
        let batch = BatchSendRequest::new(vec![minimal_email("a@example.com")])
            .unwrap()
            .with_idempotency_key("batch-key-123");

        let headers = batch.extra_headers();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Idempotency-Key");
        assert_eq!(headers[0].1, "batch-key-123");
    }
}
