# Changelog

## [0.3.2] - 2026-07-05

### Added
- reqwest 0.12 support via `reqwest-012` features (0.13 remains the default)

### Changed
- reqwest majors are mutually exclusive: enabling both is a compile error

## [0.3.1] - 2026-05-07

### Added
- Module-level docs for `api`, `api::email`, `webhook`, and the `reqwest` client
- Expanded crate-level docs covering features, modules, and error handling
- Doc comments on `EmailStatus` variants, `QueryError` variants, `WebhookError` variants, `LettermintClientError` variants, and other public fields

### Fixed
- Webhook doctest now compiles and exercises the verify path

## [0.3.0] - 2026-05-06

### Added
- Optional `tracing` feature: spans for API requests, HTTP transport, and webhook verification
- `Endpoint::parse_response()` for endpoints with non-JSON responses
- `BatchError` enum (`Empty`, `TooLarge`) for batch construction
- `WebhookError::EmptySecret` variant
- `EmailStatus::Unknown` catch-all (`#[serde(other)]`) for forward compatibility
- `#[non_exhaustive]` on `QueryError` and `EmailStatus`
- `wiremock`-based contract tests in `tests/mock.rs` covering header/body shape and untriggerable error paths (403/429/500/502)

### Changed
- `PingResponse` field `status: u16` → `message: String` (the API returns plain-text `"pong"`)
- `QueryError::Json` split into `SerializeBody` and `DeserializeResponse`
- `Webhook::new` and `Webhook::with_tolerance` now return `Result<Self, WebhookError>` instead of panicking
- `BatchSendRequest::new` returns `Result<Self, BatchError>` instead of `Option<Self>`

## [0.2.2] - 2026-04-10

### Added
- `testing::emails` module with `Scenario` enum for CI/testing email addresses
- `Scenario::email()` for base addresses, `Scenario::random()` for unique addresses
- `emails::custom()` for arbitrary local parts

### Changed
- Bumped `hmac` to 0.13, `sha2` to 0.11

## [0.2.0] - 2026-03-27

### Changed
- Rust edition bumped to 2024
- Replaced `async-trait` with native async fn in traits

### Removed
- `async-trait` dependency

## [0.1.1] - 2026-03-27

### Added
- Batch sending via `BatchSendRequest` (up to 500 emails per request)
- `PingRequest` endpoint for health checks and credential validation
- `WebhookEvent` struct with event type, delivery timestamp, and attempt number
- `content_type` field on `Attachment` for explicit MIME types
- Granular error variants: `Validation` (422), `Authentication` (401/403), `RateLimit` (429)
- `EmailStatus` variants: `Suppressed`, `Opened`, `Clicked`, `SpamComplaint`, `Blocked`, `PolicyRejected`, `Unsubscribed`

### Changed
- `Webhook::verify_headers` now accepts event/attempt headers and returns `WebhookEvent`
- `QueryError::Api` split into specific variants; generic `Api` remains as catch-all for other status codes

### Removed
- `Webhook::verify_once` (use `Webhook::new(secret).verify(...)` instead)
