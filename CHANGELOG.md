# Changelog

## [Unreleased]

### Added
- `testing::emails` module with `Scenario` enum for CI/testing email addresses
- `Scenario::email()` for base addresses, `Scenario::random()` for unique addresses
- `emails::custom()` for arbitrary local parts

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
