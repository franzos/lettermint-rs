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

### Webhook verification

```rust
use lettermint::webhook::Webhook;

let wh = Webhook::new("whsec_your_webhook_secret");
let payload = wh.verify(raw_body, signature_header).unwrap();
println!("Verified event: {}", payload);
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `reqwest` | no | reqwest HTTP client (no TLS) |
| `reqwest-native-tls` | no | reqwest with native TLS |
| `reqwest-rustls` | no | reqwest with rustls TLS |

To use your own HTTP client, implement the `Client` trait and skip the reqwest features entirely.

## Testing

Unit tests:

```sh
cargo test --all-features
```

### Integration tests

Integration tests hit the live Lettermint API using [test addresses](https://lettermint.co/docs/platform/emails/sending-test-emails) that don't count toward quotas:

```sh
LETTERMINT_API_TOKEN=your-token \
LETTERMINT_SENDER=you@yourdomain.com \
cargo test --test integration --all-features -- --ignored
```

## License

Dual-licensed under MIT or Apache 2.0.
