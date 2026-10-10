//! A complete "Sign in with Globbook" flow on a bare tokio TCP server:
//! `/login` redirects to Globbook's consent page, and
//! `/auth/globbook/callback` exchanges the returned code and prints the profile.
//!
//! A demonstration, not a production auth system: it has no session handling
//! and does not verify `state`. A real integration would generate and store a
//! `state` before redirecting, then compare it in the callback.
//!
//! ```sh
//! GLOBBOOK_CLIENT_ID=your-client-id \
//! GLOBBOOK_CLIENT_SECRET=your-client-secret \
//! GLOBBOOK_REDIRECT_URL=http://localhost:8080/auth/globbook/callback \
//! cargo run --example server
//! ```
//!
//! Then open <http://localhost:8080/login> in a browser.

use globbook_auth::{AuthorizationUrlOptions, Client, Config, parse_callback_params};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new(Config::new(
        env("GLOBBOOK_CLIENT_ID"),
        env("GLOBBOOK_CLIENT_SECRET"),
        env("GLOBBOOK_REDIRECT_URL"),
    ))?;

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("listening on http://localhost:8080/login");

    loop {
        let (mut sock, _) = listener.accept().await?;
        let client = client.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let target = String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1).map(str::to_owned))
                .unwrap_or_default();

            let resp = if target.starts_with("/login") {
                // Step 1: send the user to Globbook's hosted consent page.
                let url = client.authorization_url(&AuthorizationUrlOptions::default());
                format!("HTTP/1.1 302 Found\r\nLocation: {url}\r\nContent-Length: 0\r\n\r\n")
            } else if target.starts_with("/auth/globbook/callback") {
                // Steps 2 + 3: exchange the code, then fetch the profile.
                let (status, body) = match callback(&client, &target).await {
                    Ok(html) => ("200 OK", html),
                    Err(msg) => ("502 Bad Gateway", msg),
                };
                format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
            } else {
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".into()
            };
            let _ = sock.write_all(resp.as_bytes()).await;
        });
    }
}

async fn callback(client: &Client, target: &str) -> Result<String, String> {
    let code = parse_callback_params(target)
        .code
        .ok_or("sign-in was cancelled or failed")?;
    let token = client.exchange_code_for_token(&code).await.map_err(|e| {
        eprintln!("token exchange failed: {e}");
        "sign-in failed".to_string()
    })?;
    let user = client
        .get_user_info(&token.access_token)
        .await
        .map_err(|e| {
            eprintln!("fetching user info failed: {e}");
            "sign-in failed".to_string()
        })?;
    // A real app would look up or create a local account keyed on `user.sub`
    // and establish its own session here.
    Ok(format!(
        "<h1>Signed in with Globbook</h1><p>Welcome, {} (@{})</p><p>{}</p>",
        escape(&user.name),
        escape(&user.preferred_username),
        escape(&user.email)
    ))
}
