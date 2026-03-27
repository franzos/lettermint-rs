//! Integration tests against the live Lettermint API.
//!
//! These tests require a valid API token set via the `LETTERMINT_API_TOKEN` env var,
//! and a verified sender address via `LETTERMINT_SENDER`.
//!
//! Run with:
//!   LETTERMINT_API_TOKEN=your-token LETTERMINT_SENDER=you@yourdomain.com cargo test --test integration --all-features -- --ignored
//!
//! Lettermint provides test addresses at @testing.lettermint.co that don't count
//! toward quotas or affect bounce/complaint rates.

use lettermint::api::email::*;
use lettermint::reqwest::{LettermintClient, LettermintClientError};
use lettermint::{Query, QueryError};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn client() -> LettermintClient {
    let token = std::env::var("LETTERMINT_API_TOKEN").expect("LETTERMINT_API_TOKEN must be set");
    LettermintClient::new(token)
}

fn sender() -> String {
    std::env::var("LETTERMINT_SENDER").expect("LETTERMINT_SENDER must be set")
}

fn format_api_error(err: &QueryError<LettermintClientError>) -> String {
    match err {
        QueryError::Api {
            status,
            message,
            errors,
            ..
        } => {
            let mut msg = format!("API {status}");
            if let Some(m) = message {
                msg.push_str(&format!(": {m}"));
            }
            if let Some(errs) = errors {
                for (field, msgs) in errs {
                    for m in msgs {
                        msg.push_str(&format!("\n  {field}: {m}"));
                    }
                }
            }
            msg
        }
        other => format!("{other}"),
    }
}

#[tokio::test]
#[ignore]
async fn send_from_unverified_domain_returns_validation_error() -> Result {
    let err = SendEmailRequest::builder()
        .from("test@unverified-domain-that-does-not-exist.example")
        .to(vec!["ok@testing.lettermint.co".into()])
        .subject("Integration test: unverified domain")
        .text("This should fail with a validation error.")
        .build()
        .execute(&client())
        .await
        .expect_err("should fail with unverified domain");

    match &err {
        QueryError::Api { status, errors, .. } => {
            assert_eq!(*status, http::StatusCode::UNPROCESSABLE_ENTITY);
            assert!(errors.is_some(), "expected per-field validation errors");
            let errs = errors.as_ref().unwrap();
            assert!(errs.contains_key("from"), "expected error on 'from' field");
        }
        _ => return Err(format!("expected Api error, got: {err:?}").into()),
    }
    Ok(())
}

#[tokio::test]
#[ignore]
async fn send_text_email_ok() -> Result {
    let resp = SendEmailRequest::builder()
        .from(sender())
        .to(vec!["ok@testing.lettermint.co".into()])
        .subject("Integration test: text")
        .text("This is a plain text integration test.")
        .build()
        .execute(&client())
        .await
        .map_err(|e| format_api_error(&e))?;

    assert!(!resp.message_id.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore]
async fn send_html_email_ok() -> Result {
    let resp = SendEmailRequest::builder()
        .from(sender())
        .to(vec!["ok@testing.lettermint.co".into()])
        .subject("Integration test: html")
        .html("<h1>Hello</h1><p>HTML integration test.</p>")
        .build()
        .execute(&client())
        .await
        .map_err(|e| format_api_error(&e))?;

    assert!(!resp.message_id.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore]
async fn send_html_and_text_email_ok() -> Result {
    let resp = SendEmailRequest::builder()
        .from(sender())
        .to(vec!["ok@testing.lettermint.co".into()])
        .subject("Integration test: html+text")
        .html("<h1>Hello</h1>")
        .text("Hello")
        .build()
        .execute(&client())
        .await
        .map_err(|e| format_api_error(&e))?;

    assert!(!resp.message_id.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore]
async fn send_with_all_options() -> Result {
    let from = sender();

    let resp = SendEmailRequest::builder()
        .from(from.clone())
        .to(vec!["ok@testing.lettermint.co".into()])
        .subject("Integration test: full options")
        .html("<h1>Full test</h1>")
        .text("Full test")
        .cc(vec!["ok+cc@testing.lettermint.co".into()])
        .reply_to(vec![from])
        .tag("integration-test")
        .metadata(std::collections::HashMap::from([(
            "test".to_string(),
            "true".to_string(),
        )]))
        .idempotency_key(format!(
            "integration-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ))
        .build()
        .execute(&client())
        .await
        .map_err(|e| format_api_error(&e))?;

    assert!(!resp.message_id.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore]
async fn send_with_attachment() -> Result {
    use base64::Engine;
    let content = base64::engine::general_purpose::STANDARD.encode(b"Hello from integration test");

    let resp = SendEmailRequest::builder()
        .from(sender())
        .to(vec!["ok@testing.lettermint.co".into()])
        .subject("Integration test: attachment")
        .text("See attached file.")
        .attachments(vec![Attachment::new("test.txt", content)])
        .build()
        .execute(&client())
        .await
        .map_err(|e| format_api_error(&e))?;

    assert!(!resp.message_id.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore]
async fn send_to_soft_bounce() -> Result {
    let resp = SendEmailRequest::builder()
        .from(sender())
        .to(vec!["softbounce@testing.lettermint.co".into()])
        .subject("Integration test: soft bounce")
        .text("This should soft bounce.")
        .build()
        .execute(&client())
        .await
        .map_err(|e| format_api_error(&e))?;

    assert!(!resp.message_id.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore]
async fn send_to_hard_bounce() -> Result {
    let resp = SendEmailRequest::builder()
        .from(sender())
        .to(vec!["hardbounce@testing.lettermint.co".into()])
        .subject("Integration test: hard bounce")
        .text("This should hard bounce.")
        .build()
        .execute(&client())
        .await
        .map_err(|e| format_api_error(&e))?;

    assert!(!resp.message_id.is_empty());
    Ok(())
}
