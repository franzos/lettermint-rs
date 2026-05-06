//! Mock-based contract tests.
//!
//! Cover request/response details that can't be verified against the live API:
//! exact header values, body structure, and error codes that aren't triggerable
//! on demand (403, 429, 500, 502).
//!
//! Run with: `cargo test --test mock --all-features`

use std::collections::HashMap;

use lettermint::api::email::*;
use lettermint::api::ping::PingRequest;
use lettermint::reqwest::LettermintClient;
use lettermint::{Query, QueryError};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_client(server: &MockServer) -> LettermintClient {
    LettermintClient::with_base_url("test-token", server.uri())
}

fn ok_send_response() -> serde_json::Value {
    json!({ "message_id": "msg-123", "status": "queued" })
}

fn minimal_email() -> SendEmailRequest {
    SendEmailRequest::builder()
        .from("sender@example.com")
        .to(vec!["recipient@example.com".into()])
        .subject("Test")
        .text("Hello")
        .build()
}

// -- header contract --------------------------------------------------------

#[tokio::test]
async fn auth_header_is_set() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .and(header("x-lettermint-token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    minimal_email()
        .execute(&mock_client(&server))
        .await
        .unwrap();
}

#[tokio::test]
async fn content_type_and_accept_on_post() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .and(header("content-type", "application/json"))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    minimal_email()
        .execute(&mock_client(&server))
        .await
        .unwrap();
}

#[tokio::test]
async fn accept_on_get_no_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ping"))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("pong"))
        .expect(1)
        .mount(&server)
        .await;

    let resp = PingRequest.execute(&mock_client(&server)).await.unwrap();
    assert_eq!(resp.message, "pong");

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests[0].headers.get("content-type").is_none(),
        "GET request should not have Content-Type header"
    );
}

#[tokio::test]
async fn user_agent_header_is_set() {
    let expected_ua = format!("Lettermint/{} (Rust)", env!("CARGO_PKG_VERSION"));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    minimal_email()
        .execute(&mock_client(&server))
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let ua = requests[0]
        .headers
        .get("user-agent")
        .expect("User-Agent header should be present")
        .to_str()
        .unwrap();
    assert_eq!(ua, expected_ua);
}

#[tokio::test]
async fn idempotency_key_sent_as_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .and(header("Idempotency-Key", "unique-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    SendEmailRequest::builder()
        .from("sender@example.com")
        .to(vec!["recipient@example.com".into()])
        .subject("Test")
        .text("Hello")
        .idempotency_key("unique-123")
        .build()
        .execute(&mock_client(&server))
        .await
        .unwrap();
}

#[tokio::test]
async fn batch_idempotency_key_sent_as_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send/batch"))
        .and(header("Idempotency-Key", "batch-key-456"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{ "message_id": "msg-1", "status": "queued" }])),
        )
        .expect(1)
        .mount(&server)
        .await;

    BatchSendRequest::new(vec![minimal_email()])
        .unwrap()
        .with_idempotency_key("batch-key-456")
        .execute(&mock_client(&server))
        .await
        .unwrap();
}

// -- body contract ----------------------------------------------------------

#[tokio::test]
async fn minimal_payload_has_no_optional_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .and(body_json(json!({
            "from": "sender@example.com",
            "to": ["recipient@example.com"],
            "subject": "Test",
            "text": "Hello"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    minimal_email()
        .execute(&mock_client(&server))
        .await
        .unwrap();
}

#[tokio::test]
async fn full_payload_includes_all_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .and(body_json(json!({
            "from": "John Doe <john@example.com>",
            "to": ["alice@example.com", "bob@example.com"],
            "subject": "Newsletter",
            "html": "<h1>News</h1>",
            "text": "News",
            "cc": ["cc@example.com"],
            "bcc": ["bcc@example.com"],
            "reply_to": ["reply@example.com"],
            "headers": { "X-Custom": "value" },
            "attachments": [
                { "filename": "report.pdf", "content": "base64data" },
                { "filename": "logo.png", "content": "base64logo", "content_id": "logo" }
            ],
            "route": "my-route",
            "metadata": { "campaign": "spring" },
            "tag": "newsletter"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    SendEmailRequest::builder()
        .from("John Doe <john@example.com>")
        .to(vec!["alice@example.com".into(), "bob@example.com".into()])
        .subject("Newsletter")
        .html("<h1>News</h1>")
        .text("News")
        .cc(vec!["cc@example.com".into()])
        .bcc(vec!["bcc@example.com".into()])
        .reply_to(vec!["reply@example.com".into()])
        .headers(HashMap::from([("X-Custom".into(), "value".into())]))
        .attachments(vec![
            Attachment::new("report.pdf", "base64data"),
            Attachment::inline("logo.png", "base64logo", "logo"),
        ])
        .route("my-route")
        .metadata(HashMap::from([("campaign".into(), "spring".into())]))
        .tag("newsletter")
        .idempotency_key("idem-key")
        .build()
        .execute(&mock_client(&server))
        .await
        .unwrap();
}

#[tokio::test]
async fn idempotency_key_excluded_from_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    SendEmailRequest::builder()
        .from("sender@example.com")
        .to(vec!["recipient@example.com".into()])
        .subject("Test")
        .text("Hello")
        .idempotency_key("secret-key")
        .build()
        .execute(&mock_client(&server))
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert!(
        body.get("idempotency_key").is_none(),
        "idempotency_key must not appear in the request body"
    );
}

#[tokio::test]
async fn batch_serializes_as_array() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send/batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "message_id": "msg-1", "status": "queued" },
            { "message_id": "msg-2", "status": "queued" },
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let email_a = SendEmailRequest::builder()
        .from("sender@example.com")
        .to(vec!["alice@example.com".into()])
        .subject("Hello Alice")
        .text("Hi Alice!")
        .build();
    let email_b = SendEmailRequest::builder()
        .from("sender@example.com")
        .to(vec!["bob@example.com".into()])
        .subject("Hello Bob")
        .text("Hi Bob!")
        .build();

    BatchSendRequest::new(vec![email_a, email_b])
        .unwrap()
        .execute(&mock_client(&server))
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    let arr = body.as_array().expect("batch body must be a JSON array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["to"], json!(["alice@example.com"]));
    assert_eq!(arr[1]["to"], json!(["bob@example.com"]));
}

// -- custom client configuration -------------------------------------------

#[tokio::test]
async fn custom_base_url_with_trailing_slash() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_send_response()))
        .expect(1)
        .mount(&server)
        .await;

    let client = LettermintClient::with_base_url("test-token", format!("{}/", server.uri()));
    let resp = minimal_email().execute(&client).await.unwrap();
    assert_eq!(resp.message_id, "msg-123");
}

// -- untriggerable error scenarios ------------------------------------------

#[tokio::test]
async fn error_403_is_authentication() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({ "message": "Access denied" })),
        )
        .mount(&server)
        .await;

    let err = minimal_email()
        .execute(&mock_client(&server))
        .await
        .expect_err("should fail with 403");

    assert!(
        matches!(err, QueryError::Authentication { .. }),
        "expected Authentication error, got: {err:?}"
    );
}

#[tokio::test]
async fn error_429_is_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(json!({ "message": "Rate limit exceeded" })),
        )
        .mount(&server)
        .await;

    let err = minimal_email()
        .execute(&mock_client(&server))
        .await
        .expect_err("should fail with 429");

    match err {
        QueryError::RateLimit { message, .. } => {
            assert_eq!(message.as_deref(), Some("Rate limit exceeded"));
        }
        other => panic!("expected RateLimit error, got: {other:?}"),
    }
}

#[tokio::test]
async fn error_500_is_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(
                json!({ "error_type": "InternalError", "message": "Something broke" }),
            ),
        )
        .mount(&server)
        .await;

    let err = minimal_email()
        .execute(&mock_client(&server))
        .await
        .expect_err("should fail with 500");

    match err {
        QueryError::Api {
            status,
            error_type,
            message,
            ..
        } => {
            assert_eq!(status, http::StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(error_type.as_deref(), Some("InternalError"));
            assert_eq!(message.as_deref(), Some("Something broke"));
        }
        other => panic!("expected Api error, got: {other:?}"),
    }
}

#[tokio::test]
async fn non_json_error_body_handled() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/send"))
        .respond_with(ResponseTemplate::new(502).set_body_string("gateway timeout"))
        .mount(&server)
        .await;

    let err = minimal_email()
        .execute(&mock_client(&server))
        .await
        .expect_err("should fail with 502");

    match err {
        QueryError::Api {
            status,
            message,
            body,
            ..
        } => {
            assert_eq!(status, http::StatusCode::BAD_GATEWAY);
            assert_eq!(message, None);
            assert_eq!(body.as_ref(), b"gateway timeout");
        }
        other => panic!("expected Api error, got: {other:?}"),
    }
}
