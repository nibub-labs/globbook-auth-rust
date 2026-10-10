# Changelog

All notable changes to this project are documented in this file.

## 1.1.0 - Initial release

Initial release of `globbook-auth`, the Rust client SDK for "Sign in with
Globbook".

- `Client::new(Config)` validating `client_id`, `client_secret`, and `redirect_url`.
- `Client::authorization_url()` with `AuthorizationUrlOptions` (`scopes`, `state`) —
  `GET /api/v2/oauth/authorize`.
- `parse_callback_params()` accepting a full URL, a path with query string, or a bare
  query string; returns `CallbackParams { code, state }`.
- `Client::exchange_code_for_token()` — `POST /api/v2/oauth/token`
  (`application/x-www-form-urlencoded`).
- `Client::get_user_info()` — `GET /api/v2/oauth/userinfo`, including the restricted
  `birthdate`, `gender`, `phone_number`, and `address` claims as `Option<String>`.
- A single `AuthError` (`code`, `description`, `status_code`) for every API failure, with
  SDK-raised `timeout`, `network_error`, and `invalid_response` codes.
- `Config::timeout` (default 10s; zero disables) and injectable `reqwest::Client`.
- Secret redaction: `Config`, `Client`, and `Token` never print the client secret or access
  token through `Debug`.
- Async, built on `reqwest` with rustls; no OpenSSL required.
