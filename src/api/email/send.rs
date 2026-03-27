use crate::Endpoint;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use typed_builder::TypedBuilder;

/// Send a single email via Lettermint.
///
/// ```
/// # use lettermint::api::email::SendEmailRequest;
/// let req = SendEmailRequest::builder()
///   .from("sender@example.com")
///   .to(vec!["recipient@example.com".into()])
///   .subject("Hello")
///   .text("Hi there!")
///   .build();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, TypedBuilder)]
pub struct SendEmailRequest {
    /// Sender email address. RFC 5322 format supported: "Name <email>" or "email".
    #[builder(setter(into))]
    pub from: String,

    /// One or more recipient email addresses.
    #[builder(setter(into))]
    pub to: Vec<String>,

    /// Email subject line.
    #[builder(setter(into))]
    pub subject: String,

    /// HTML body content.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub html: Option<String>,

    /// Plain text body content.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub text: Option<String>,

    /// CC recipients.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub cc: Option<Vec<String>>,

    /// BCC recipients.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub bcc: Option<Vec<String>>,

    /// Reply-To addresses.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub reply_to: Option<Vec<String>>,

    /// Custom email headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub headers: Option<HashMap<String, String>>,

    /// File attachments.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub attachments: Option<Vec<Attachment>>,

    /// Routing key / sending domain.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub route: Option<String>,

    /// Custom metadata key-value pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub metadata: Option<HashMap<String, String>>,

    /// Categorization tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into, strip_option))]
    pub tag: Option<String>,

    /// Idempotency key to prevent duplicate sends. Sent as a header, not in the body.
    #[serde(skip)]
    #[builder(default, setter(into, strip_option))]
    pub idempotency_key: Option<String>,
}

/// An email attachment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// Filename.
    pub filename: String,
    /// Base64-encoded content.
    pub content: String,
    /// Content-ID for inline attachments (referenced via `cid:<content_id>` in HTML).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
}

impl Attachment {
    pub fn new(filename: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            filename: filename.into(),
            content: content.into(),
            content_id: None,
        }
    }

    pub fn inline(
        filename: impl Into<String>,
        content: impl Into<String>,
        content_id: impl Into<String>,
    ) -> Self {
        Self {
            filename: filename.into(),
            content: content.into(),
            content_id: Some(content_id.into()),
        }
    }
}

/// Response from sending an email.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendEmailResponse {
    pub message_id: String,
    pub status: EmailStatus,
}

/// Status of an email.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailStatus {
    Pending,
    Queued,
    Processed,
    Delivered,
    SoftBounced,
    HardBounced,
    Failed,
}

impl std::fmt::Display for EmailStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Queued => write!(f, "queued"),
            Self::Processed => write!(f, "processed"),
            Self::Delivered => write!(f, "delivered"),
            Self::SoftBounced => write!(f, "soft_bounced"),
            Self::HardBounced => write!(f, "hard_bounced"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl Endpoint for SendEmailRequest {
    type Request = SendEmailRequest;
    type Response = SendEmailResponse;

    fn endpoint(&self) -> Cow<'static, str> {
        "send".into()
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

    #[test]
    fn serialize_minimal_request() {
        let req = SendEmailRequest::builder()
            .from("sender@example.com")
            .to(vec!["recipient@example.com".into()])
            .subject("Hello")
            .text("Hi there!")
            .build();

        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            json!({
                "from": "sender@example.com",
                "to": ["recipient@example.com"],
                "subject": "Hello",
                "text": "Hi there!",
            })
        );
    }

    #[test]
    fn serialize_full_request() {
        let req = SendEmailRequest::builder()
            .from("John Doe <john@example.com>")
            .to(vec!["user1@example.com".into(), "user2@example.com".into()])
            .subject("Newsletter")
            .html("<h1>News</h1>")
            .text("News")
            .cc(vec!["cc@example.com".into()])
            .bcc(vec!["bcc@example.com".into()])
            .reply_to(vec!["reply@example.com".into()])
            .tag("newsletter")
            .route("my-route")
            .idempotency_key("unique-123")
            .build();

        let val = serde_json::to_value(&req).unwrap();

        assert_eq!(val["from"], "John Doe <john@example.com>");
        assert_eq!(val["to"], json!(["user1@example.com", "user2@example.com"]));
        assert_eq!(val["cc"], json!(["cc@example.com"]));
        assert_eq!(val["tag"], "newsletter");
        assert_eq!(val["route"], "my-route");
        // idempotency_key is a header, not serialized in body
        assert!(val.get("idempotency_key").is_none());
    }

    #[test]
    fn serialize_with_attachment() {
        let req = SendEmailRequest::builder()
            .from("sender@example.com")
            .to(vec!["recipient@example.com".into()])
            .subject("With attachment")
            .text("See attached")
            .attachments(vec![
                Attachment::new("report.pdf", "base64content"),
                Attachment::inline("logo.png", "base64logo", "logo"),
            ])
            .build();

        let val = serde_json::to_value(&req).unwrap();
        let attachments = val["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 2);
        assert_eq!(attachments[0]["filename"], "report.pdf");
        assert!(attachments[0].get("content_id").is_none());
        assert_eq!(attachments[1]["content_id"], "logo");
    }

    #[test]
    fn deserialize_response() {
        let json = r#"{"message_id":"abc-123","status":"queued"}"#;
        let resp: SendEmailResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message_id, "abc-123");
        assert_eq!(resp.status, EmailStatus::Queued);
    }

    #[test]
    fn idempotency_key_in_extra_headers() {
        let req = SendEmailRequest::builder()
            .from("sender@example.com")
            .to(vec!["recipient@example.com".into()])
            .subject("Test")
            .text("Test")
            .idempotency_key("my-key")
            .build();

        let headers = req.extra_headers();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Idempotency-Key");
        assert_eq!(headers[0].1, "my-key");
    }

    #[test]
    fn no_extra_headers_without_idempotency_key() {
        let req = SendEmailRequest::builder()
            .from("sender@example.com")
            .to(vec!["recipient@example.com".into()])
            .subject("Test")
            .text("Test")
            .build();

        assert!(req.extra_headers().is_empty());
    }
}
