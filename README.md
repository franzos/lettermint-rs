# Lettermint

[![ci](https://github.com/franzos/lettermint-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/franzos/lettermint-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/lettermint.svg)](https://crates.io/crates/lettermint)
[![Documentation](https://docs.rs/lettermint/badge.svg)](https://docs.rs/lettermint)

Rust client library for the [Lettermint](https://lettermint.co) email service. HTTP client-agnostic — ships with a `reqwest` implementation, or bring your own by implementing the `Client` trait.

## Usage

```toml
[dependencies]
lettermint = { version = "0.1", features = ["reqwest-rustls"] }
tokio = { version = "1", features = ["rt", "macros"] }
```

### Send an email

```rust
use lettermint::api::email::SendEmailRequest;
use lettermint::reqwest::LettermintClient;
use lettermint::Query;

#[tokio::main]
async fn main() {
    let client = LettermintClient::new("your-api-token");

    let req = SendEmailRequest::builder()
        .from("sender@yourdomain.com")
        .to(vec!["recipient@example.com".into()])
        .subject("Hello from Lettermint")
        .text("Plain text body")
        .build();

    let resp = req.execute(&client).await.unwrap();
    println!("Sent: {} ({})", resp.message_id, resp.status);
}
```

### HTML + text with all options

```rust
use lettermint::api::email::{SendEmailRequest, Attachment};
use lettermint::reqwest::LettermintClient;
use lettermint::Query;
use std::collections::HashMap;

async fn send_full(client: &LettermintClient) {
    let req = SendEmailRequest::builder()
        .from("Jane <jane@yourdomain.com>")
        .to(vec!["user@example.com".into()])
        .subject("Monthly update")
        .html("<h1>Update</h1><p>Here's what happened.</p>")
        .text("Here's what happened.")
        .cc(vec!["team@example.com".into()])
        .bcc(vec!["archive@example.com".into()])
        .reply_to(vec!["support@yourdomain.com".into()])
        .headers(HashMap::from([
            ("X-Campaign".into(), "monthly-update".into()),
        ]))
        .attachments(vec![
            Attachment::new("report.pdf", "<base64-encoded-content>"),
            Attachment::inline("logo.png", "<base64-encoded-logo>", "logo"),
        ])
        .metadata(HashMap::from([
            ("campaign_id".into(), "2025-03".into()),
        ]))
        .tag("newsletter")
        .route("my-route")
        .idempotency_key("monthly-update-2025-03")
        .build();

    let resp = req.execute(client).await.unwrap();
    println!("{:?}", resp);
}
```

### Batch sending

Send up to 500 emails in a single request:

```rust
use lettermint::api::email::{SendEmailRequest, BatchSendRequest};
use lettermint::reqwest::LettermintClient;
use lettermint::Query;

async fn send_batch(client: &LettermintClient) {
    let batch = BatchSendRequest::new(vec![
        SendEmailRequest::builder()
            .from("sender@yourdomain.com")
            .to(vec!["alice@example.com".into()])
            .subject("Hello Alice")
            .text("Hi Alice!")
            .build(),
        SendEmailRequest::builder()
            .from("sender@yourdomain.com")
            .to(vec!["bob@example.com".into()])
            .subject("Hello Bob")
            .text("Hi Bob!")
            .build(),
    ])
    .expect("batch must be 1-500 emails");

    let responses = batch.execute(client).await.unwrap();
    for resp in responses {
        println!("Sent: {} ({})", resp.message_id, resp.status);
    }
}
```

### Ping

Check API connectivity and validate credentials:

```rust
use lettermint::api::ping::PingRequest;
use lettermint::reqwest::LettermintClient;
use lettermint::Query;

async fn ping(client: &LettermintClient) {
    let resp = PingRequest.execute(client).await.unwrap();
    println!("API status: {}", resp.status);
}
```

### Webhook verification

```rust
use lettermint::webhook::Webhook;

let wh = Webhook::new("whsec_your_webhook_secret");

// Simple verification — returns parsed JSON payload
let payload = wh.verify(raw_body, signature_header).unwrap();
println!("Verified event: {}", payload);

// Full header verification — returns WebhookEvent with metadata
let event = wh.verify_headers(
    signature_header,
    delivery_header,    // X-Lettermint-Delivery
    event_header,       // X-Lettermint-Event
    attempt_header,     // X-Lettermint-Attempt
    raw_body,
).unwrap();
println!("Event: {:?}, attempt: {:?}", event.event, event.attempt);
```

### Error handling

```rust
use lettermint::{Query, QueryError};

match req.execute(&client).await {
    Ok(resp) => println!("Sent: {}", resp.message_id),
    Err(QueryError::Validation { errors, message, .. }) => {
        eprintln!("Validation failed: {message:?}, fields: {errors:?}");
    }
    Err(QueryError::Authentication { message, .. }) => {
        eprintln!("Auth failed: {message:?}");
    }
    Err(QueryError::RateLimit { message, .. }) => {
        eprintln!("Rate limited: {message:?}");
    }
    Err(e) => eprintln!("Error: {e}"),
}
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `reqwest` | no | reqwest HTTP client (no TLS) |
| `reqwest-native-tls` | no | reqwest with native TLS |
| `reqwest-rustls` | no | reqwest with rustls TLS |

To use your own HTTP client, implement the `Client` trait and skip the reqwest features entirely — useful if you need a different reqwest version, or a different HTTP client like `ureq` or `hyper`.

### Custom HTTP client example

```rust
use bytes::Bytes;
use http::{Request, Response};
use lettermint::Client;

struct MyClient {
    api_token: String,
    http: reqwest::Client, // any version
}

#[derive(Debug, thiserror::Error)]
enum MyClientError {
    #[error("http: {0}")]
    Http(#[from] http::Error),
    #[error("request: {0}")]
    Request(#[from] reqwest::Error),
    #[error("header: {0}")]
    Header(#[from] http::header::InvalidHeaderValue),
}

impl Client for MyClient {
    type Error = MyClientError;

    async fn execute(&self, mut req: Request<Bytes>) -> Result<Response<Bytes>, Self::Error> {
        req.headers_mut()
            .append("x-lettermint-token", self.api_token.as_str().try_into()?);

        let base = "https://api.lettermint.co/v1";
        let path = req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("");
        *req.uri_mut() = format!("{base}/{}", path.trim_start_matches('/')).parse().unwrap();

        let rr: reqwest::Request = req.try_into()?;
        let rsp = self.http.execute(rr).await?;

        let mut builder = Response::builder().status(rsp.status());
        for (k, v) in rsp.headers() {
            builder.headers_mut().unwrap().insert(k, v.clone());
        }
        Ok(builder.body(rsp.bytes().await?)?)
    }
}
```

## Testing

Unit tests:

```sh
cargo test --all-features
```

### Test email addresses

The `testing::emails` module provides [Lettermint test addresses](https://lettermint.co/docs/platform/emails/sending-test-emails) that simulate delivery scenarios without affecting quotas or bounce rates:

```rust
use lettermint::testing::emails::{self, Scenario};

// Fixed addresses for each scenario
let ok = Scenario::Ok.email();            // ok@testing.lettermint.co
let bounce = Scenario::HardBounce.email(); // hardbounce@testing.lettermint.co

// Unique addresses for CI
let unique = Scenario::SoftBounce.random(); // softbounce+{unique}@testing.lettermint.co

// Custom local part
let tagged = emails::custom("ok+ci");      // ok+ci@testing.lettermint.co
```

Available scenarios: `Ok`, `SoftBounce`, `HardBounce`, `SpamComplaint`, `Dsn`.

### Integration tests

Integration tests hit the live Lettermint API using test addresses that don't count toward quotas:

```sh
LETTERMINT_API_TOKEN=your-token \
LETTERMINT_SENDER=you@yourdomain.com \
cargo test --test integration --all-features -- --ignored
```

## License

Dual-licensed under MIT or Apache 2.0.
