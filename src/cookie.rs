use std::fmt;

use zeroize::Zeroizing;

/// A cookie value. It cannot be printed by accident: `Debug` and `Display` show only its
/// length, and the memory is cleared when the value is dropped.
///
/// Read it with [`SecretValue::expose`], only where it is really needed (for example when
/// writing the cookie file for a downloader).
#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    /// The real value. Never log it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretValue(<{} bytes hidden>)", self.0.len())
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<hidden>")
    }
}

/// One cookie of the requested site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// The cookie's domain as the browser stores it: `example.com` for a host-only cookie,
    /// `.example.com` for a cookie that also applies to subdomains.
    pub domain: String,
    pub name: String,
    pub value: SecretValue,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// Expiry as Unix seconds, or `None` for a session cookie.
    pub expires: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_never_show_the_value() {
        let value = SecretValue::new("top-secret-session-id".into());
        assert_eq!(format!("{value:?}"), "SecretValue(<21 bytes hidden>)");
        assert_eq!(format!("{value}"), "<hidden>");
        assert_eq!(value.expose(), "top-secret-session-id");
    }

    #[test]
    fn cookie_debug_does_not_leak_the_value() {
        let cookie = Cookie {
            domain: "example.com".into(),
            name: "session".into(),
            value: SecretValue::new("top-secret-session-id".into()),
            path: "/".into(),
            secure: true,
            http_only: true,
            expires: None,
        };
        let printed = format!("{cookie:?}");
        assert!(printed.contains("session"));
        assert!(!printed.contains("top-secret"));
    }
}
