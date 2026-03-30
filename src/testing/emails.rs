//! Hardcoded testing email addresses for CI and integration testing.
//!
//! The local part (before `@`) determines delivery behavior. Any address
//! ending in `@testing.lettermint.co` is accepted — the [`Scenario`] enum
//! covers the documented simulation scenarios.
//!
//! See <https://lettermint.co/docs/platform/emails/sending-test-emails>.

/// Testing email domain.
pub const DOMAIN: &str = "testing.lettermint.co";

/// Delivery scenario for a testing email.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// Successful delivery (`message.delivered`).
    Ok,
    /// Soft bounce — mailbox full (`message.soft_bounced`).
    SoftBounce,
    /// Hard bounce — user unknown (`message.hard_bounced`).
    HardBounce,
    /// Delivery followed by a spam complaint (`message.spam_complaint`).
    SpamComplaint,
    /// Out-of-band DSN bounce (`message.hard_bounced`).
    Dsn,
}

impl Scenario {
    /// The local part used in the email address for this scenario.
    pub fn local_part(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::SoftBounce => "softbounce",
            Self::HardBounce => "hardbounce",
            Self::SpamComplaint => "spamcomplaint",
            Self::Dsn => "dsn",
        }
    }

    /// The base testing email address for this scenario.
    ///
    /// ```
    /// # use lettermint::testing::emails::Scenario;
    /// assert_eq!(Scenario::Ok.email(), "ok@testing.lettermint.co");
    /// assert_eq!(Scenario::HardBounce.email(), "hardbounce@testing.lettermint.co");
    /// ```
    pub fn email(&self) -> String {
        custom(self.local_part())
    }

    /// Generate a unique testing email for this scenario.
    ///
    /// ```
    /// # use lettermint::testing::emails::Scenario;
    /// let a = Scenario::Ok.random();
    /// let b = Scenario::Ok.random();
    /// assert_ne!(a, b);
    /// assert!(a.starts_with("ok+"));
    /// assert!(a.ends_with("@testing.lettermint.co"));
    ///
    /// let bounce = Scenario::HardBounce.random();
    /// assert!(bounce.starts_with("hardbounce+"));
    /// ```
    pub fn random(&self) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();

        custom(&format!("{}+{ts}-{pid}-{seq}", self.local_part()))
    }
}

/// Build a testing email with a custom local part.
///
/// ```
/// # use lettermint::testing::emails;
/// assert_eq!(emails::custom("ok+tag"), "ok+tag@testing.lettermint.co");
/// ```
pub fn custom(local_part: &str) -> String {
    format!("{local_part}@{DOMAIN}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_emails() {
        assert_eq!(Scenario::Ok.email(), "ok@testing.lettermint.co");
        assert_eq!(Scenario::SoftBounce.email(), "softbounce@testing.lettermint.co");
        assert_eq!(Scenario::HardBounce.email(), "hardbounce@testing.lettermint.co");
        assert_eq!(Scenario::SpamComplaint.email(), "spamcomplaint@testing.lettermint.co");
        assert_eq!(Scenario::Dsn.email(), "dsn@testing.lettermint.co");
    }

    #[test]
    fn custom_builds_address() {
        assert_eq!(custom("hello"), "hello@testing.lettermint.co");
        assert_eq!(custom("ok+tag"), "ok+tag@testing.lettermint.co");
    }

    #[test]
    fn random_is_unique() {
        let a = Scenario::Ok.random();
        let b = Scenario::Ok.random();
        assert_ne!(a, b);
        assert!(a.starts_with("ok+"));
        assert!(a.ends_with(&format!("@{DOMAIN}")));
    }

    #[test]
    fn random_respects_scenario() {
        let addr = Scenario::HardBounce.random();
        assert!(addr.starts_with("hardbounce+"));
        assert!(addr.ends_with(&format!("@{DOMAIN}")));
    }
}
