//! The flows of the issuer, by its router without TLS: the confidential client, the public
//! client of the native applications (loopback, PKCE, device flow) and the rotation of
//! the refresh tokens (spec 023, task 023-7).

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

use super::*;

const CONFIG: &str = r#"
    issuer = "https://127.0.0.1:8444"
    listen = "127.0.0.1:8444"
    client_id = "kartenn"
    client_secret_file = "unused"
    redirect_uris = ["https://127.0.0.1:9443/v1/auth/callback"]
    cert_file = "unused"
    key_file = "unused"

    [native]
    client_id = "kartenn-cli"
    audience = "kartenn"
    device = true
    slow_down = 1

    [[users]]
    sub = "alice"
    name = "Alice Exemple"
    groups = ["kartenn-admins"]

    [[users]]
    sub = "refuse"
    name = "Refus"
    error = "access_denied"
"#;

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const LOOPBACK: &str = "http://127.0.0.1:49152/callback";

fn issuer() -> (Router, Shared) {
    let config: Config = toml::from_str(CONFIG).unwrap();
    config.check().unwrap();
    let idp = Arc::new(Idp::new(config, "secret".into()).unwrap());
    (router(idp.clone()), idp)
}

async fn call(app: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let (status, headers) = (response.status(), response.headers().clone());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, headers, body)
}

fn form(path: &str, fields: &[(&str, &str)], basic: bool) -> Request<Body> {
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields)
        .finish();
    let mut request =
        Request::post(path).header("content-type", "application/x-www-form-urlencoded");
    if basic {
        request = request.header(
            "authorization",
            format!("Basic {}", STANDARD.encode("kartenn:secret")),
        );
    }
    request.body(Body::from(body)).unwrap()
}

/// The parameters of the redirection `location`.
fn parameters(location: &str) -> HashMap<String, String> {
    let query = location.split_once('?').map_or("", |(_, q)| q);
    form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect()
}

/// The header and the claims of a JWT, unverified.
fn decoded(token: &str) -> (Value, Value) {
    let mut parts = token.split('.');
    let mut part = || {
        let bytes = URL_SAFE_NO_PAD.decode(parts.next().unwrap()).unwrap();
        serde_json::from_slice::<Value>(&bytes).unwrap()
    };
    (part(), part())
}

/// The fields of an authorization request of `client` to `redirect_uri`.
fn authorization<'a>(client: &'a str, redirect_uri: &'a str, pkce: bool) -> Vec<(&'a str, String)> {
    let mut fields = vec![
        ("response_type", "code".to_owned()),
        ("client_id", client.to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
        ("scope", "openid offline_access".to_owned()),
        ("state", "st".to_owned()),
        ("nonce", "no".to_owned()),
        ("audience", "kartenn".to_owned()),
    ];
    if pkce {
        fields.push((
            "code_challenge",
            URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes())),
        ));
        fields.push(("code_challenge_method", "S256".to_owned()));
    }
    fields
}

/// A code of `user` for `client`, through the page of the issuer.
async fn code(
    app: &Router,
    client: &str,
    redirect_uri: &str,
    user: &str,
) -> HashMap<String, String> {
    let mut fields = authorization(client, redirect_uri, true);
    fields.push(("user", user.to_owned()));
    let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, headers, _) = call(app, form("/authorize", &pairs, false)).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = headers["location"].to_str().unwrap();
    assert!(location.starts_with(redirect_uri), "{location}");
    parameters(location)
}

/// The exchange of `code` of the public client to the loopback, by `verifier`.
fn exchange(code: &str, verifier: &str, basic: bool) -> Request<Body> {
    form(
        "/token",
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", LOOPBACK),
            ("client_id", "kartenn-cli"),
        ],
        basic,
    )
}

#[tokio::test]
async fn the_discovery_announces_the_public_client_and_the_device_flow() {
    let (app, _) = issuer();
    let request = Request::get("/.well-known/openid-configuration")
        .body(Body::empty())
        .unwrap();
    let (_, _, document) = call(&app, request).await;
    assert_eq!(
        document["device_authorization_endpoint"],
        "https://127.0.0.1:8444/device_authorization"
    );
    assert_eq!(
        document["grant_types_supported"],
        json!(["authorization_code", "refresh_token", DEVICE_GRANT])
    );
    assert_eq!(
        document["token_endpoint_auth_methods_supported"],
        json!(["client_secret_basic", "none"])
    );
    // Without [native], neither.
    let mut config: Config = toml::from_str(CONFIG).unwrap();
    config.native = None;
    let app = router(Arc::new(Idp::new(config, "s".into()).unwrap()));
    let request = Request::get("/.well-known/openid-configuration")
        .body(Body::empty())
        .unwrap();
    let (_, _, document) = call(&app, request).await;
    assert!(document.get("device_authorization_endpoint").is_none());
    assert_eq!(
        document["token_endpoint_auth_methods_supported"],
        json!(["client_secret_basic"])
    );
}

#[tokio::test]
async fn the_public_client_logs_in_by_loopback_with_pkce() {
    let (app, _) = issuer();
    // Any port of the loopback; the page offers the users.
    let query = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(authorization("kartenn-cli", LOOPBACK, true))
        .finish();
    let request = Request::get(format!("/authorize?{query}"))
        .body(Body::empty())
        .unwrap();
    let (status, _, page) = call(&app, request).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    // Another path, `localhost`: refused by a page, never a redirection.
    for refused in [
        "http://127.0.0.1:49152/other",
        "http://localhost:49152/callback",
    ] {
        let query = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(authorization("kartenn-cli", refused, true))
            .finish();
        let request = Request::get(format!("/authorize?{query}"))
            .body(Body::empty())
            .unwrap();
        let (status, _, _) = call(&app, request).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    }
    // PKCE is required.
    let query = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(authorization("kartenn-cli", LOOPBACK, false))
        .finish();
    let request = Request::get(format!("/authorize?{query}"))
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = call(&app, request).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        parameters(headers["location"].to_str().unwrap())["error"],
        "invalid_request"
    );
    // The code, then its exchange without secret, its verifier checked.
    let back = code(&app, "kartenn-cli", LOOPBACK, "alice").await;
    assert_eq!(back["state"], "st");
    assert_eq!(back["iss"], "https://127.0.0.1:8444");
    // The confidential client cannot take the code of the public one; a wrong verifier
    // is refused.
    for (verifier, basic) in [(VERIFIER, true), ("wrong-verifier", false)] {
        let back = code(&app, "kartenn-cli", LOOPBACK, "alice").await;
        let request = exchange(&back["code"], verifier, basic);
        let (status, _, body) = call(&app, request).await;
        assert_eq!(
            (status, &body["error"]),
            (StatusCode::BAD_REQUEST, &json!("invalid_grant"))
        );
    }
    let (status, headers, tokens) = call(&app, exchange(&back["code"], VERIFIER, false)).await;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    assert_eq!(headers["cache-control"], "no-store");
    let (header, claims) = decoded(tokens["access_token"].as_str().unwrap());
    assert_eq!(header["typ"], "at+jwt");
    assert_eq!(claims["aud"], "kartenn");
    assert_eq!(claims["client_id"], "kartenn-cli");
    assert_eq!(claims["sub"], "alice");
    assert_eq!(claims["groups"], json!(["kartenn-admins"]));
    assert_eq!(claims["scope"], "openid offline_access");
    let (header, claims) = decoded(tokens["id_token"].as_str().unwrap());
    assert!(header.get("typ").is_none_or(|t| t == "JWT"), "{header}");
    assert_eq!(
        (&claims["aud"], &claims["nonce"]),
        (&json!("kartenn-cli"), &json!("no"))
    );
    // An unknown public client, or one sending a secret, is not authenticated.
    let unknown = [("grant_type", "refresh_token"), ("client_id", "other")];
    let secret = [
        ("grant_type", "refresh_token"),
        ("client_id", "kartenn-cli"),
        ("client_secret", "x"),
    ];
    for fields in [&unknown[..], &secret[..]] {
        let (status, _, body) = call(&app, form("/token", fields, false)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{fields:?}");
        assert_eq!(body["error"], "invalid_client");
    }
}

#[tokio::test]
async fn the_refresh_tokens_rotate() {
    let (app, _) = issuer();
    let back = code(&app, "kartenn-cli", LOOPBACK, "alice").await;
    let (_, _, tokens) = call(&app, exchange(&back["code"], VERIFIER, false)).await;
    let first = tokens["refresh_token"].as_str().unwrap().to_owned();
    let refresh = |token: &str, basic: bool| {
        form(
            "/token",
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", token),
                ("client_id", "kartenn-cli"),
            ],
            basic,
        )
    };
    let (status, _, renewed) = call(&app, refresh(&first, false)).await;
    assert_eq!(status, StatusCode::OK, "{renewed}");
    let second = renewed["refresh_token"].as_str().unwrap();
    assert_ne!(second, first);
    assert_eq!(
        decoded(renewed["access_token"].as_str().unwrap()).0["typ"],
        "at+jwt"
    );
    // The old one is refused once used.
    let (status, _, body) = call(&app, refresh(&first, false)).await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_REQUEST, &json!("invalid_grant"))
    );
    // Bound to its client: the confidential one cannot use it.
    let (status, _, body) = call(&app, refresh(second, true)).await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_REQUEST, &json!("invalid_grant"))
    );
    // The confidential client keeps its flow, and its rotation.
    let back = code(
        &app,
        "kartenn",
        "https://127.0.0.1:9443/v1/auth/callback",
        "alice",
    )
    .await;
    let request = form(
        "/token",
        &[
            ("grant_type", "authorization_code"),
            ("code", &back["code"]),
            ("code_verifier", VERIFIER),
            ("redirect_uri", "https://127.0.0.1:9443/v1/auth/callback"),
        ],
        true,
    );
    let (status, _, tokens) = call(&app, request).await;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    assert_eq!(
        decoded(tokens["id_token"].as_str().unwrap()).1["aud"],
        "kartenn"
    );
    let first = tokens["refresh_token"].as_str().unwrap().to_owned();
    let (status, _, _) = call(&app, refresh(&first, true)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(&app, refresh(&first, true)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Asks for a device code.
async fn device(app: &Router) -> Value {
    let request = form(
        "/device_authorization",
        &[
            ("client_id", "kartenn-cli"),
            ("scope", "openid offline_access"),
            ("audience", "kartenn"),
        ],
        false,
    );
    let (status, _, body) = call(app, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

fn poll(device_code: &str) -> Request<Body> {
    form(
        "/token",
        &[
            ("grant_type", DEVICE_GRANT),
            ("device_code", device_code),
            ("client_id", "kartenn-cli"),
        ],
        false,
    )
}

#[tokio::test]
async fn the_public_client_logs_in_by_the_device_flow() {
    let (app, idp) = issuer();
    let answer = device(&app).await;
    let code = answer["user_code"].as_str().unwrap();
    assert_eq!(code.len(), 9, "XXXX-XXXX");
    assert_eq!(answer["verification_uri"], "https://127.0.0.1:8444/device");
    assert_eq!(answer["interval"], 5);
    assert_eq!(answer["expires_in"], 600);
    let device_code = answer["device_code"].as_str().unwrap();
    // The first poll: slow_down on demand; the second, too early: slow_down again.
    for _ in 0..2 {
        let (status, _, body) = call(&app, poll(device_code)).await;
        assert_eq!(
            (status, &body["error"]),
            (StatusCode::BAD_REQUEST, &json!("slow_down"))
        );
    }
    let interval = locked(&idp.devices)[device_code].interval;
    assert_eq!(
        interval,
        Duration::from_secs(15),
        "5 s more at each slow_down"
    );
    // Paced: pending.
    locked(&idp.devices).get_mut(device_code).unwrap().last_poll = None;
    let (_, _, body) = call(&app, poll(device_code)).await;
    assert_eq!(body["error"], "authorization_pending");
    // The page knows the code typed in lower case, without dash.
    let typed = code.replace('-', "").to_lowercase();
    let request = Request::get(format!("/device?user_code={typed}"))
        .body(Body::empty())
        .unwrap();
    let (status, _, page) = call(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.as_str().unwrap().contains("alice"));
    let request = form(
        "/device",
        &[("user_code", &typed), ("user", "alice")],
        false,
    );
    let (status, _, _) = call(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, tokens) = call(&app, poll(device_code)).await;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    let (header, claims) = decoded(tokens["access_token"].as_str().unwrap());
    assert_eq!(
        (&header["typ"], &claims["aud"]),
        (&json!("at+jwt"), &json!("kartenn"))
    );
    assert!(tokens["refresh_token"].is_string());
    // Used once.
    let (_, _, body) = call(&app, poll(device_code)).await;
    assert_eq!(body["error"], "invalid_grant");
}

#[tokio::test]
async fn a_device_code_is_denied_or_expires() {
    let (app, idp) = issuer();
    let answer = device(&app).await;
    let request = form(
        "/device",
        &[
            ("user_code", answer["user_code"].as_str().unwrap()),
            ("user", "refuse"),
        ],
        false,
    );
    let (status, _, _) = call(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    let (_, _, body) = call(&app, poll(answer["device_code"].as_str().unwrap())).await;
    assert_eq!(body["error"], "access_denied");
    let answer = device(&app).await;
    let device_code = answer["device_code"].as_str().unwrap();
    locked(&idp.devices).get_mut(device_code).unwrap().expires = Instant::now();
    let (_, _, body) = call(&app, poll(device_code)).await;
    assert_eq!(body["error"], "expired_token");
    // The confidential client has no device flow; a wrong audience is refused.
    let (status, _, body) = call(&app, {
        let mut request = poll("x");
        request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {}", STANDARD.encode("kartenn:secret"))).unwrap(),
        );
        request
    })
    .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_REQUEST, &json!("unauthorized_client"))
    );
    let request = form(
        "/device_authorization",
        &[
            ("client_id", "kartenn-cli"),
            ("scope", "openid"),
            ("resource", "https://other.invalid"),
        ],
        false,
    );
    let (_, _, body) = call(&app, request).await;
    assert_eq!(body["error"], "invalid_target");
}

#[test]
fn origins_and_escapes() {
    assert_eq!(
        origin("http://127.0.0.1:5173/v1/auth/callback"),
        Some("http://127.0.0.1:5173")
    );
    assert_eq!(origin("https://h"), Some("https://h"));
    assert_eq!(origin("nope"), None);
    assert_eq!(
        escape(r#"<a href="x">'&"#),
        "&lt;a href=&quot;x&quot;&gt;&#39;&amp;"
    );
}
