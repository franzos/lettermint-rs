//! HTTP client-agnostic Rust bindings for the Lettermint email service.
//!
//! The crate is split into a transport-agnostic core ([`Client`], [`Endpoint`],
//! [`Query`]) and an optional [`reqwest`]-backed implementation behind a feature
//! flag. Bring your own HTTP client by implementing [`Client`].
//!
//! # Quick start
//!
//! ```
//! use lettermint::reqwest::LettermintClient;
//! use lettermint::*;
//!
//! # async fn send_email() {
//! let client = LettermintClient::new("your-api-token");
//!
//! let req = api::email::SendEmailRequest::builder()
//!   .from("sender@example.com")
//!   .to(vec!["recipient@example.com".into()])
//!   .subject("Hello")
//!   .html("<h1>Welcome!</h1>")
//!   .build();
//!
//! let resp = req.execute(&client).await;
//! # }
//! ```
//!
//! # Features
//!
//! - `reqwest` — enables [`crate::reqwest::LettermintClient`], a ready-made
//!   [`Client`] backed by the `reqwest` HTTP crate. No TLS backend is selected
//!   by default; pair it with one of the two below.
//! - `reqwest-native-tls` — enables `reqwest` with the OS-native TLS stack.
//! - `reqwest-rustls` — enables `reqwest` with `rustls`.
//! - `tracing` — emits `tracing` spans/events around requests and webhook
//!   verification.
//!
//! All features are off by default. If none of the `reqwest-*` features fit,
//! implement [`Client`] yourself against any HTTP backend.
//!
//! # Modules
//!
//! - [`api`] — endpoint types (e.g. [`api::email::SendEmailRequest`],
//!   [`api::ping::PingRequest`]). Each implements [`Endpoint`].
//! - [`webhook`] — verify HMAC-SHA256 signatures on inbound Lettermint webhook
//!   deliveries.
//! - [`testing`] — predictable scenario addresses for exercising the API
//!   without sending real mail.
//!
//! # Error handling
//!
//! Calls to [`Query::execute`] return [`QueryError`], which distinguishes
//! transport failures, body (de)serialization, and typed API errors
//! (`Validation`, `Authentication`, `RateLimit`, `Api`).

/// Default Lettermint API URL.
pub const LETTERMINT_API_URL: &str = "https://api.lettermint.co/v1/";

pub mod api;
mod client;
pub mod testing;
pub mod webhook;

pub use client::*;

#[cfg(feature = "reqwest")]
pub mod reqwest;
