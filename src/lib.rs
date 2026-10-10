//! Official Rust SDK for **"Sign in with Globbook"** — the OAuth 2.0
//! authorization-code flow exposed at `/api/v2/oauth/*`.
//!
//! ```no_run
//! use globbook_auth::{parse_callback_params, AuthorizationUrlOptions, Client, Config};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::new(Config::new("client_id", "client_secret", "https://yourapp.com/callback"))?;
//!
//! // Step 1: redirect the user's browser here.
//! let url = client.authorization_url(&AuthorizationUrlOptions {
//!     state: Some("csrf-token".into()),
//!     ..Default::default()
//! });
//!
//! // Step 2: in your callback route.
//! let params = parse_callback_params("/callback?code=abc&state=csrf-token");
//! let token = client.exchange_code_for_token(&params.code.unwrap()).await?;
//! let user = client.get_user_info(&token.access_token).await?;
//! # Ok(()) }
//! ```

mod client;
mod errors;
mod types;

pub use client::{Client, parse_callback_params};
pub use errors::*;
pub use types::*;
