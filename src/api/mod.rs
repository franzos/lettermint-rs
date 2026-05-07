//! Endpoint definitions for the Lettermint REST API.
//!
//! Each request type implements [`crate::Endpoint`] and can be sent through
//! anything that implements [`crate::Client`] via [`crate::Query::execute`].

pub mod email;
pub mod ping;
