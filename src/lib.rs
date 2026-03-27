//! Lettermint is an HTTP client-agnostic Rust client for the Lettermint email service.
//!
//! It provides a `reqwest` implementation that can be initialized and passed
//! into the execute function of a [`Query`]. All [`Endpoint`] types implement
//! the Query trait.
//!
//! To use the [`reqwest`] based client, enable the `"reqwest"` feature.
//! You can also implement your own client by implementing the [`Client`] trait.
//!
//! # Example:
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

/// Default Lettermint API URL.
pub const LETTERMINT_API_URL: &str = "https://api.lettermint.co/v1/";

pub mod api;
mod client;
pub mod webhook;

pub use client::*;

#[cfg(feature = "reqwest")]
pub mod reqwest;
