//! Configuration and response types.

use serde::Deserialize;
use std::fmt;
use std::time::Duration;

/// Default Globbook origin.
pub const DEFAULT_BASE_URL: &str = "https://globbook.com";
/// Default per-request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Restricted scope: the user's birthdate.
pub const SCOPE_BIRTHDATE: &str = "birthdate";
/// Restricted scope: the user's gender.
pub const SCOPE_GENDER: &str = "gender";
/// Restricted scope: the user's phone number.
pub const SCOPE_PHONE: &str = "phone";
/// Restricted scope: the user's address.
pub const SCOPE_ADDRESS: &str = "address";

/// Settings passed to [`Client::new`](crate::Client::new).
#[derive(Clone)]
pub struct Config {
    /// App ID from Globbook's developer console.
    pub client_id: String,
    /// Confidential secret from the developer console. Server-side only.
    pub client_secret: String,
    /// Your callback URL, exactly as registered with Globbook.
    pub redirect_url: String,
    /// Globbook API origin. Defaults to [`DEFAULT_BASE_URL`].
    pub base_url: String,
    /// Per-request timeout. Defaults to [`DEFAULT_TIMEOUT`] (10s); zero disables it.
    pub timeout: Duration,
    /// `reqwest::Client` to use for requests. Defaults to a fresh client.
    pub http_client: Option<reqwest::Client>,
}

impl Config {
    /// A config with the three required fields and defaults for the rest.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_url: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_url: redirect_url.into(),
            base_url: DEFAULT_BASE_URL.into(),
            timeout: DEFAULT_TIMEOUT,
            http_client: None,
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("redirect_url", &self.redirect_url)
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Options for [`Client::authorization_url`](crate::Client::authorization_url).
#[derive(Debug, Default, Clone)]
pub struct AuthorizationUrlOptions {
    /// Restricted scopes to request (see the `SCOPE_*` constants).
    pub scopes: Vec<String>,
    /// Opaque CSRF-protection value, echoed back on the callback (RFC 6749 §10.12).
    pub state: Option<String>,
}

/// Access token issued by [`Client::exchange_code_for_token`](crate::Client::exchange_code_for_token).
#[derive(Clone, Deserialize)]
pub struct Token {
    /// Bearer credential — treat like a password.
    pub access_token: String,
    /// Always `Bearer`.
    pub token_type: String,
    /// Lifetime in seconds (3600).
    pub expires_in: u64,
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Token")
            .field("access_token", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// The authenticated user's profile.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserInfo {
    /// OIDC `sub` — an md5 hash, not the numeric user id.
    pub sub: String,
    /// Username.
    pub preferred_username: String,
    /// Whether the profile is verified.
    pub profile_verified: bool,
    /// Email address.
    pub email: String,
    /// Full name.
    pub name: String,
    /// Given name.
    pub given_name: String,
    /// Family name.
    pub family_name: String,
    /// Bio.
    pub bio: String,
    /// Signed CDN URL, if any.
    pub picture: Option<String>,
    /// Signed CDN URL, if any.
    pub cover_image: Option<String>,
    /// Website.
    pub website: String,
    /// `YYYY-MM-DD`. Restricted: `None` unless the client is verified and the user granted the scope.
    pub birthdate: Option<String>,
    /// Restricted, see `birthdate`.
    pub gender: Option<String>,
    /// Restricted, see `birthdate`.
    pub phone_number: Option<String>,
    /// `"city country"`. Restricted, see `birthdate`.
    pub address: Option<String>,
}

/// Values from the callback request. Both are `None` if absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallbackParams {
    /// Single-use authorization code.
    pub code: Option<String>,
    /// The `state` you sent, if any; compare it against what you stored.
    pub state: Option<String>,
}
