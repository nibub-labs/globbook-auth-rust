# globbook-auth-rust

A Rust client SDK for **"Sign in with Globbook"** — the OAuth 2.0-style
authorization-code flow exposed by Globbook's backend at `/api/v2/oauth/*`.

Use this crate to let users of your Rust server-side application (axum,
actix-web, Rocket, or anything else) sign in with their Globbook account. It
wraps the three real HTTP calls the flow requires (authorization redirect,
token exchange, userinfo fetch) behind a small, idiomatic async Rust API.

> **Note**: this crate does not register applications with Globbook.
> Before using it you must create an app in Globbook's developer console
> to obtain a `client_id`, `client_secret`, and register your app's
> `redirect_url` — that is a one-time manual step, unrelated to this SDK.

## Installation

```sh
cargo add globbook-auth
```

Requires Rust 1.88 or later (edition 2024). HTTP is handled by `reqwest` with rustls (no
OpenSSL needed).

## Quickstart

The full flow has three steps: redirect the user to Globbook, receive the
callback and exchange the code for a token, then fetch the user's profile.

```rust
use globbook_auth::{parse_callback_params, AuthorizationUrlOptions, Client, Config};

let client = Client::new(Config::new(
    std::env::var("GLOBBOOK_CLIENT_ID")?,
    std::env::var("GLOBBOOK_CLIENT_SECRET")?,
    "https://yourapp.com/auth/globbook/callback",
))?;

// Step 1: send the user to Globbook's hosted consent page.
let url = client.authorization_url(&AuthorizationUrlOptions::default());
// ...respond with a 302 redirect to `url`

// Step 2 + 3: Globbook redirects back with ?code=... after the user approves.
let params = parse_callback_params(request_uri); // full URL, path+query, or bare query
let Some(code) = params.code else {
    return Err("sign-in was cancelled or failed".into());
};

let token = client.exchange_code_for_token(&code).await?;
let user = client.get_user_info(&token.access_token).await?;

// Look up or create a local account keyed on user.sub, then establish your
// own session. user.sub is a stable md5 hash identifying the Globbook user —
// not their raw numeric ID.
println!("signed in: {} <{}>", user.name, user.email);
```

A runnable version of this example lives in [`examples/server.rs`](./examples/server.rs).

## API reference

Full documentation is on [docs.rs](https://docs.rs/globbook-auth), and locally
via `cargo doc --open`. Summary of the public surface:

### `struct Config`

Settings passed to `Client::new`. Build one with `Config::new(client_id, client_secret, redirect_url)`:

| Field          | Required | Description                                                                  |
| -------------- | -------- | ----------------------------------------------------------------------------- |
| `client_id`    | yes      | App ID from Globbook's developer console.                                     |
| `client_secret`| yes      | Confidential secret from the developer console. Server-side only.             |
| `redirect_url` | yes      | Your app's callback URL, exactly as registered with Globbook.                 |
| `base_url`     | no       | Globbook API origin. Defaults to `https://globbook.com`.                      |
| `timeout`      | no       | Per-request timeout. Defaults to 10 seconds; `Duration::ZERO` disables it.    |
| `http_client`  | no       | A `reqwest::Client` to use for requests. Defaults to a fresh client.          |

### `Client::new(config: Config) -> Result<Client, ConfigError>`

Validates that `client_id`, `client_secret`, and `redirect_url` are all
non-empty, returning `ConfigError::MissingClientId`, `MissingClientSecret`, or
`MissingRedirectUrl` otherwise — validation happens at construction time, not
deferred to the first API call. A `Client` is stateless, cheap to `clone`, and
safe to share across tasks.

### `Client::authorization_url(&self, opts: &AuthorizationUrlOptions) -> String`

Builds the URL to redirect the user's browser to, to start the sign-in flow.
Makes no network request — your handler issues the actual redirect. Pass
`&AuthorizationUrlOptions::default()` for the base profile with no
CSRF-protection state:

```rust
pub struct AuthorizationUrlOptions {
    pub scopes: Vec<String>,   // restricted scopes — see "Restricted claims" below
    pub state: Option<String>, // opaque CSRF-protection value — see "CSRF protection (state)" below
}
```

```rust
use globbook_auth::{SCOPE_BIRTHDATE, SCOPE_GENDER};

let url = client.authorization_url(&AuthorizationUrlOptions {
    scopes: vec![SCOPE_BIRTHDATE.into(), SCOPE_GENDER.into()],
    state: Some(csrf_token),
});
```

### `parse_callback_params(url_or_query: &str) -> CallbackParams`

Extracts the authorization code (and CSRF-protection state, if present) from a
callback request. Framework-agnostic: pass a full URL, a path with query
string, or a bare query string (with or without a leading `?`).

```rust
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>, // None if you didn't send one
}
```

### `Client::exchange_code_for_token(&self, code: &str) -> Result<Token, AuthError>`

Exchanges an authorization code for an access token via
`POST /api/v2/oauth/token` (sent as `application/x-www-form-urlencoded`, the
only content type Globbook's token endpoint accepts).

```rust
pub struct Token {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64, // seconds
}
```

### `Client::get_user_info(&self, access_token: &str) -> Result<UserInfo, AuthError>`

Fetches the authenticated user's profile via `GET /api/v2/oauth/userinfo`.

```rust
pub struct UserInfo {
    pub sub: String,                // OIDC "sub" — an md5 hash, not the numeric user id
    pub preferred_username: String,
    pub profile_verified: bool,
    pub email: String,
    pub name: String,
    pub given_name: String,
    pub family_name: String,
    pub bio: String,
    pub picture: Option<String>,    // signed CDN URL, or None
    pub cover_image: Option<String>,// signed CDN URL, or None
    pub website: String,

    // Restricted claims — None unless your app is verified in the Globbook
    // Developer Console AND the user granted the matching scope at consent time.
    pub birthdate: Option<String>,  // YYYY-MM-DD, or None
    pub gender: Option<String>,
    pub phone_number: Option<String>,
    pub address: Option<String>,    // "city country" — this platform has no street address
}
```

### Restricted claims

`birthdate`, `gender`, `phone_number`, and `address` are gated separately from
the rest of the profile. Globbook only populates them — the field is `None`
otherwise — when **both** are true:

1. Your app has been verified in the Globbook Developer Console (see
   "Requesting verification" in the developer docs).
2. You requested the scope via `AuthorizationUrlOptions::scopes` (constants
   `SCOPE_BIRTHDATE`, `SCOPE_GENDER`, `SCOPE_PHONE`, `SCOPE_ADDRESS`) when
   starting the flow, **and** the signed-in user granted it on the consent
   screen — requesting a scope is not the same as receiving it; the user can
   uncheck any scope individually.

An unverified app never receives these fields, regardless of what scopes it
requests or what the user approves on consent. Always check before use:

```rust
if let Some(birthdate) = &user.birthdate {
    println!("birthdate: {birthdate}");
}
```

### CSRF protection (state)

Pass `state` in `AuthorizationUrlOptions` to protect against login CSRF
(RFC 6749 §10.12): an attacker who obtains their own valid authorization code
could otherwise trick a victim's browser into completing the attacker's login
on the victim's session.

```rust
// Before redirecting — generate an unguessable value and store it
// (session, signed cookie) tied to the current browser session.
let csrf_token = generate_random_token();
save_to_session(&req, "oauth_state", &csrf_token);

let url = client.authorization_url(&AuthorizationUrlOptions {
    state: Some(csrf_token),
    ..Default::default()
});

// In your callback handler — compare before exchanging the code.
let params = parse_callback_params(request_uri);
if params.code.is_none() || params.state != load_from_session(&req, "oauth_state") {
    return Err("invalid or missing state — possible CSRF");
}
```

`state` is entirely optional and Globbook never interprets it — it's echoed
back unchanged, per the RFC 6749 `state` parameter. Omitting it does not change
any other behavior; this is opt-in hardening, not a required step.

## Error handling

Every failed API call (`exchange_code_for_token`, `get_user_info`) returns an
`AuthError`:

```rust
use globbook_auth::{ERROR_INVALID_GRANT, ERROR_INVALID_REQUEST};

match client.exchange_code_for_token(&code).await {
    Ok(token) => { /* ... */ }
    Err(e) if e.code == ERROR_INVALID_GRANT => {
        // code was invalid, expired, or already used — restart the flow
    }
    Err(e) if e.code == ERROR_INVALID_REQUEST => {
        // a required field was missing — almost certainly a bug in your integration
    }
    Err(e) => eprintln!("globbook oauth error: {}: {}", e.code, e.description),
}
```

`AuthError` has `code`, `description`, and `status_code: Option<u16>`, and
implements `std::error::Error`.

Known error codes (all exported as constants):

| Code                       | Meaning                                                                 |
| --------------------------- | ------------------------------------------------------------------------ |
| `invalid_request`           | A required field was missing/malformed.                                  |
| `invalid_grant`             | `client_id`/`client_secret`/`code` combination rejected.                 |
| `invalid_token`             | The access token given to `get_user_info` is missing/malformed/expired.  |
| `unsupported_media_type`    | The request wasn't sent as `application/x-www-form-urlencoded`. Should never occur from this SDK itself. |
| `timeout`                   | The request exceeded `Config::timeout`. Raised by this SDK, not the API. |
| `network_error`             | The request failed before reaching the API (DNS, TLS, connection refused). Raised by this SDK. |
| `invalid_response`          | Globbook returned a non-JSON or unexpectedly-shaped body.                |

## Security notes

- **`client_secret` is a server-side secret.** Never embed it in a mobile app,
  browser bundle, or any client-side code — only call this SDK from your
  backend. `Config`, `Client`, and `Token` redact their secret from `Debug`
  output, but that is a safety net, not a substitute for keeping it out of
  client-side code in the first place.
- **`Token::access_token` is a bearer credential.** Treat it like a password:
  don't log it, don't put it in a URL, transmit it only over HTTPS.
- Every request made by this SDK is bounded by `Config::timeout` (default 10s)
  so a slow or unresponsive Globbook endpoint can't hang your request handler
  indefinitely.

## Testing

The test suite runs entirely offline against a local one-shot HTTP server.

```sh
cargo build
cargo clippy --all-targets
cargo test
```

## License

MIT — see [LICENSE](./LICENSE).
