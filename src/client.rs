//! The OAuth client.

use crate::errors::*;
use crate::types::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::fmt;

/// Extracts the authorization code and `state` from the callback Globbook
/// redirects the browser to. Accepts a full URL, a path with query string, or
/// a bare query string (with or without a leading `?`).
///
/// If you sent a `state`, compare the returned one against what you stored and
/// reject the callback on a mismatch.
pub fn parse_callback_params(url_or_query: &str) -> CallbackParams {
    let query = url_or_query
        .split_once('?')
        .map_or(url_or_query, |(_, q)| q);
    let query = query.split('#').next().unwrap_or("");
    let mut out = CallbackParams::default();
    for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
        if v.is_empty() {
            continue;
        }
        match &*k {
            "code" if out.code.is_none() => out.code = Some(v.into_owned()),
            "state" if out.state.is_none() => out.state = Some(v.into_owned()),
            _ => {}
        }
    }
    out
}

/// Server-side client for "Sign in with Globbook".
///
/// Stateless, cheap to clone, and safe to share across tasks.
///
/// **Security:** only construct this in server-side code. It holds
/// `client_secret`, which must never reach a browser or mobile app.
#[derive(Clone)]
pub struct Client {
    config: Config,
    base_url: String,
    http: reqwest::Client,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.config)
            .finish()
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
}

impl Client {
    /// Validates `config` and builds a client; no network I/O happens here.
    pub fn new(config: Config) -> Result<Self, ConfigError> {
        if config.client_id.trim().is_empty() {
            return Err(ConfigError::MissingClientId);
        }
        if config.client_secret.trim().is_empty() {
            return Err(ConfigError::MissingClientSecret);
        }
        if config.redirect_url.trim().is_empty() {
            return Err(ConfigError::MissingRedirectUrl);
        }
        let base = if config.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            &config.base_url
        };
        let base_url = base.trim_end_matches('/').to_owned();
        let http = config.http_client.clone().unwrap_or_default();
        Ok(Self {
            config,
            base_url,
            http,
        })
    }

    /// Step 1: the URL to redirect the user's browser to. Makes no request.
    pub fn authorization_url(&self, opts: &AuthorizationUrlOptions) -> String {
        let mut q = url::form_urlencoded::Serializer::new(String::new());
        q.append_pair("client_id", &self.config.client_id);
        if !opts.scopes.is_empty() {
            q.append_pair("scope", &opts.scopes.join(" "));
        }
        if let Some(state) = opts.state.as_deref().filter(|s| !s.is_empty()) {
            q.append_pair("state", state);
        }
        format!("{}/api/v2/oauth/authorize?{}", self.base_url, q.finish())
    }

    /// Step 3: exchanges the authorization code for an access token via
    /// `POST /api/v2/oauth/token` (form-encoded).
    pub async fn exchange_code_for_token(&self, code: &str) -> Result<Token, AuthError> {
        if code.trim().is_empty() {
            return Err(AuthError::new(
                ERROR_INVALID_REQUEST,
                "exchange_code_for_token: 'code' must not be empty",
                None,
            ));
        }
        let req = self
            .http
            .post(format!("{}/api/v2/oauth/token", self.base_url))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("code", code),
            ]);
        self.send(req).await
    }

    /// Step 4: fetches the user's profile via `GET /api/v2/oauth/userinfo`.
    pub async fn get_user_info(&self, access_token: &str) -> Result<UserInfo, AuthError> {
        if access_token.trim().is_empty() {
            return Err(AuthError::new(
                ERROR_INVALID_REQUEST,
                "get_user_info: 'access_token' must not be empty",
                None,
            ));
        }
        let req = self
            .http
            .get(format!("{}/api/v2/oauth/userinfo", self.base_url))
            .header("Accept", "application/json")
            .bearer_auth(access_token);
        self.send(req).await
    }

    async fn send<T: DeserializeOwned>(
        &self,
        mut req: reqwest::RequestBuilder,
    ) -> Result<T, AuthError> {
        if !self.config.timeout.is_zero() {
            req = req.timeout(self.config.timeout);
        }
        let resp = req.send().await.map_err(|e| self.transport_error(&e))?;
        let status = resp.status();
        let body = resp.bytes().await.map_err(|e| self.transport_error(&e))?;

        if status.is_success() {
            return serde_json::from_slice(&body).map_err(|_| {
                AuthError::new(
                    ERROR_INVALID_RESPONSE,
                    "Globbook API returned a non-JSON response.",
                    Some(status.as_u16()),
                )
            });
        }
        Err(match serde_json::from_slice::<ErrorBody>(&body) {
            Ok(e) if !e.error.is_empty() => {
                AuthError::new(&e.error, e.error_description, Some(status.as_u16()))
            }
            _ => AuthError::new(
                ERROR_INVALID_RESPONSE,
                format!(
                    "Globbook API returned a non-JSON error response (HTTP {}).",
                    status.as_u16()
                ),
                Some(status.as_u16()),
            ),
        })
    }

    fn transport_error(&self, e: &reqwest::Error) -> AuthError {
        if e.is_timeout() {
            AuthError::new(
                ERROR_TIMEOUT,
                format!(
                    "Request to Globbook timed out after {:?}",
                    self.config.timeout
                ),
                None,
            )
        } else {
            AuthError::new(
                ERROR_NETWORK_ERROR,
                format!("Request to Globbook failed: {}", strip(e)),
                None,
            )
        }
    }
}

fn strip(e: &reqwest::Error) -> String {
    // reqwest embeds the URL in its Display output; the callers already know it.
    let s = e.to_string();
    s.split(" for url").next().unwrap_or(&s).to_owned()
}
