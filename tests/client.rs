//! Integration tests against a local one-shot HTTP server.

use globbook_auth::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

fn client(base: &str) -> Client {
    let mut cfg = Config::new("cid", "sec", "https://app.com/cb");
    cfg.base_url = base.into();
    Client::new(cfg).unwrap()
}

/// One-shot HTTP server; resolves to the raw request it received.
async fn serve(status: &str, body: &str) -> (String, JoinHandle<String>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let h = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut buf = vec![0u8; 8192];
        let n = s.read(&mut buf).await.unwrap();
        s.write_all(resp.as_bytes()).await.unwrap();
        String::from_utf8_lossy(&buf[..n]).into_owned()
    });
    (base, h)
}

#[test]
fn config_validation() {
    assert_eq!(
        Client::new(Config::new("", "s", "r")).unwrap_err(),
        ConfigError::MissingClientId
    );
    assert_eq!(
        Client::new(Config::new("c", " ", "r")).unwrap_err(),
        ConfigError::MissingClientSecret
    );
    assert_eq!(
        Client::new(Config::new("c", "s", "")).unwrap_err(),
        ConfigError::MissingRedirectUrl
    );
}

#[test]
fn authorization_url() {
    let c = client("https://g.test/");
    assert_eq!(
        c.authorization_url(&Default::default()),
        "https://g.test/api/v2/oauth/authorize?client_id=cid"
    );
    let opts = AuthorizationUrlOptions {
        scopes: vec![SCOPE_BIRTHDATE.into(), SCOPE_GENDER.into()],
        state: Some("a b".into()),
    };
    assert_eq!(
        c.authorization_url(&opts),
        "https://g.test/api/v2/oauth/authorize?client_id=cid&scope=birthdate+gender&state=a+b"
    );
    let default = Client::new(Config::new("cid", "s", "r")).unwrap();
    assert!(
        default
            .authorization_url(&Default::default())
            .starts_with("https://globbook.com/api/v2/")
    );
}

#[test]
fn callback_params() {
    let want = CallbackParams {
        code: Some("abc".into()),
        state: Some("x y".into()),
    };
    for input in [
        "code=abc&state=x%20y",
        "?code=abc&state=x+y",
        "https://app.com/cb?code=abc&state=x%20y#frag",
        "/cb?code=abc&state=x+y",
    ] {
        assert_eq!(parse_callback_params(input), want, "{input}");
    }
    assert_eq!(
        parse_callback_params("error=access_denied"),
        CallbackParams::default()
    );
    assert_eq!(parse_callback_params(""), CallbackParams::default());
}

#[test]
fn secrets_are_redacted() {
    let c = client("https://g.test");
    assert!(!format!("{c:?}").contains("\"sec\""));
    let t = Token {
        access_token: "SECRETTOK".into(),
        token_type: "Bearer".into(),
        expires_in: 1,
    };
    assert!(!format!("{t:?}").contains("SECRETTOK"));
}

#[tokio::test]
async fn exchanges_code() {
    let (base, req) = serve(
        "200 OK",
        r#"{"access_token":"T","token_type":"Bearer","expires_in":3600}"#,
    )
    .await;
    let t = client(&base).exchange_code_for_token("c0de").await.unwrap();
    assert_eq!(
        (t.access_token.as_str(), t.token_type.as_str(), t.expires_in),
        ("T", "Bearer", 3600)
    );
    let raw = req.await.unwrap();
    assert!(raw.starts_with("POST /api/v2/oauth/token"));
    assert!(
        raw.to_lowercase()
            .contains("application/x-www-form-urlencoded")
    );
    assert!(raw.ends_with("client_id=cid&client_secret=sec&code=c0de"));
}

#[tokio::test]
async fn maps_oauth_error() {
    let (base, _) = serve(
        "401 Unauthorized",
        r#"{"error":"invalid_grant","error_description":"bad code"}"#,
    )
    .await;
    let e = client(&base)
        .exchange_code_for_token("x")
        .await
        .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.description.as_str(), e.status_code),
        (ERROR_INVALID_GRANT, "bad code", Some(401))
    );
}

#[tokio::test]
async fn non_json_responses() {
    let (base, _) = serve("502 Bad Gateway", "<html>").await;
    let e = client(&base).get_user_info("t").await.unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.status_code),
        (ERROR_INVALID_RESPONSE, Some(502))
    );
    let (base, _) = serve("200 OK", "nope").await;
    assert_eq!(
        client(&base).get_user_info("t").await.unwrap_err().code,
        ERROR_INVALID_RESPONSE
    );
}

#[tokio::test]
async fn rejects_empty_arguments() {
    let c = client("http://127.0.0.1:1");
    assert_eq!(
        c.exchange_code_for_token(" ").await.unwrap_err().code,
        ERROR_INVALID_REQUEST
    );
    assert_eq!(
        c.get_user_info("").await.unwrap_err().code,
        ERROR_INVALID_REQUEST
    );
}

#[tokio::test]
async fn network_error() {
    let e = client("http://127.0.0.1:1")
        .get_user_info("t")
        .await
        .unwrap_err();
    assert_eq!(e.code, ERROR_NETWORK_ERROR);
}

#[tokio::test]
async fn times_out() {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = Config::new("cid", "sec", "r");
    cfg.base_url = format!("http://{}", l.local_addr().unwrap());
    cfg.timeout = Duration::from_millis(100);
    let _hold = tokio::spawn(async move {
        let _conn = l.accept().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    });
    let e = Client::new(cfg)
        .unwrap()
        .get_user_info("t")
        .await
        .unwrap_err();
    assert_eq!(e.code, ERROR_TIMEOUT);
}

#[tokio::test]
async fn gets_userinfo_with_bearer_and_optional_claims() {
    let (base, req) = serve(
        "200 OK",
        r#"{"sub":"1","email":"a@b.c","picture":null,"gender":"female"}"#,
    )
    .await;
    let u = client(&base).get_user_info("TOK").await.unwrap();
    assert_eq!((u.sub.as_str(), u.email.as_str()), ("1", "a@b.c"));
    assert_eq!(
        (u.birthdate, u.gender, u.picture),
        (None, Some("female".into()), None)
    );
    let raw = req.await.unwrap();
    assert!(raw.starts_with("GET /api/v2/oauth/userinfo"));
    assert!(raw.to_lowercase().contains("authorization: bearer tok"));
}
