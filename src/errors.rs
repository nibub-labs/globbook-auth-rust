//! Errors returned by this SDK.

use std::fmt;

/// `invalid_request` — a required field was missing or malformed.
pub const ERROR_INVALID_REQUEST: &str = "invalid_request";
/// `invalid_grant` — the code was invalid, expired, or already used.
pub const ERROR_INVALID_GRANT: &str = "invalid_grant";
/// `invalid_token` — the access token was missing, malformed, or expired.
pub const ERROR_INVALID_TOKEN: &str = "invalid_token";
/// `unsupported_media_type` — wrong request content type.
pub const ERROR_UNSUPPORTED_MEDIA_TYPE: &str = "unsupported_media_type";

// Raised by this SDK itself (never returned by the Globbook API), so callers
// can tell a real API rejection from a client-side transport failure.
/// The request timed out.
pub const ERROR_TIMEOUT: &str = "timeout";
/// The request failed at the transport level.
pub const ERROR_NETWORK_ERROR: &str = "network_error";
/// Globbook returned a response that could not be decoded.
pub const ERROR_INVALID_RESPONSE: &str = "invalid_response";

/// An OAuth-standard error from Globbook's authorization server (RFC 6749
/// §5.2), or a client-side transport failure normalized into the same shape
/// (see the `ERROR_*` constants).
///
/// Every failure from [`Client::exchange_code_for_token`](crate::Client::exchange_code_for_token)
/// and [`Client::get_user_info`](crate::Client::get_user_info) is an
/// `AuthError`, so callers only ever handle one error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthError {
    /// OAuth error code, e.g. [`ERROR_INVALID_GRANT`].
    pub code: String,
    /// Human-readable description; may be empty.
    pub description: String,
    /// HTTP status, when the error came from a Globbook response.
    pub status_code: Option<u16>,
}

impl AuthError {
    pub(crate) fn new(
        code: &str,
        description: impl Into<String>,
        status_code: Option<u16>,
    ) -> Self {
        Self {
            code: code.into(),
            description: description.into(),
            status_code,
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "globbookauth: {}", self.code)?;
        if !self.description.is_empty() {
            write!(f, " ({})", self.description)?;
        }
        Ok(())
    }
}

impl std::error::Error for AuthError {}

/// Invalid [`Config`](crate::Config), reported by [`Client::new`](crate::Client::new).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// `client_id` is empty.
    MissingClientId,
    /// `client_secret` is empty.
    MissingClientSecret,
    /// `redirect_url` is empty.
    MissingRedirectUrl,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingClientId => "globbookauth: client_id is required",
            Self::MissingClientSecret => "globbookauth: client_secret is required",
            Self::MissingRedirectUrl => "globbookauth: redirect_url is required",
        })
    }
}

impl std::error::Error for ConfigError {}
