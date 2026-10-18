//! A small HTTPS client for the issuers (discovery, keys, token endpoint): rustls with
//! `ring`, bounded in size and time; plain HTTP only towards the loopback (tests, a
//! development IdP).

use std::fmt::Write as _;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::Request;
use hyper::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use serde::de::DeserializeOwned;

/// Largest response read.
const MAX_BODY: usize = 1024 * 1024;
/// Longest exchange.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Why an exchange with an issuer failed.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// Plain HTTP outside the loopback, or a malformed URL.
    #[error("refused URL {0}: HTTPS is required outside the loopback")]
    Insecure(String),
    /// Transport.
    #[error("{0}")]
    Transport(String),
    /// A status other than 2xx.
    #[error("status {0}")]
    Status(u16),
    /// Not the JSON expected.
    #[error("response: {0}")]
    Decode(String),
}

/// The client.
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
}

fn allowed(url: &str) -> Result<hyper::Uri, HttpError> {
    let uri: hyper::Uri = url
        .parse()
        .map_err(|_| HttpError::Insecure(url.to_owned()))?;
    let loopback = matches!(
        uri.host(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    );
    match uri.scheme_str() {
        Some("https") => Ok(uri),
        Some("http") if loopback => Ok(uri),
        _ => Err(HttpError::Insecure(url.to_owned())),
    }
}

impl HttpClient {
    /// With the system's roots of trust.
    ///
    /// # Errors
    ///
    /// The roots cannot be loaded.
    pub fn new() -> Result<Self, HttpError> {
        let connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| HttpError::Transport(e.to_string()))?
            .https_or_http()
            .enable_http1()
            .build();
        Ok(Self {
            client: Client::builder(TokioExecutor::new()).build(connector),
        })
    }

    async fn send<T: DeserializeOwned>(
        &self,
        request: Request<Full<Bytes>>,
    ) -> Result<T, HttpError> {
        let exchange = async {
            let response = self
                .client
                .request(request)
                .await
                .map_err(|e| HttpError::Transport(e.to_string()))?;
            let status = response.status();
            let body = Limited::new(response.into_body(), MAX_BODY)
                .collect()
                .await
                .map_err(|e| HttpError::Transport(e.to_string()))?
                .to_bytes();
            if !status.is_success() {
                return Err(HttpError::Status(status.as_u16()));
            }
            serde_json::from_slice(&body).map_err(|e| HttpError::Decode(e.to_string()))
        };
        tokio::time::timeout(TIMEOUT, exchange)
            .await
            .map_err(|_| HttpError::Transport("timed out".into()))?
    }

    /// GETs a JSON document.
    ///
    /// # Errors
    ///
    /// [`HttpError`].
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, HttpError> {
        let request = Request::get(allowed(url)?)
            .header(ACCEPT, "application/json")
            .body(Full::new(Bytes::new()))
            .map_err(|e| HttpError::Transport(e.to_string()))?;
        self.send(request).await
    }

    /// POSTs a form, with HTTP Basic credentials if given (client authentication at a
    /// token endpoint), and reads a JSON answer.
    ///
    /// # Errors
    ///
    /// [`HttpError`].
    pub async fn post_form<T: DeserializeOwned>(
        &self,
        url: &str,
        form: &[(&str, &str)],
        basic: Option<(&str, &str)>,
    ) -> Result<T, HttpError> {
        let body = form
            .iter()
            .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let mut request = Request::post(allowed(url)?)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(ACCEPT, "application/json");
        if let Some((user, password)) = basic {
            use base64::Engine as _;
            let credentials = base64::engine::general_purpose::STANDARD.encode(format!(
                "{}:{}",
                encode(user),
                encode(password)
            ));
            request = request.header(AUTHORIZATION, format!("Basic {credentials}"));
        }
        let request = request
            .body(Full::new(Bytes::from(body)))
            .map_err(|e| HttpError::Transport(e.to_string()))?;
        self.send(request).await
    }
}

/// Percent-encodes a form value (`application/x-www-form-urlencoded`).
#[must_use]
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_only_towards_the_loopback() {
        assert!(allowed("https://idp.example.invalid/x").is_ok());
        assert!(allowed("http://127.0.0.1:8080/x").is_ok());
        assert!(allowed("http://localhost/x").is_ok());
        assert!(allowed("http://idp.example.invalid/x").is_err());
        assert!(allowed("ftp://idp.example.invalid/x").is_err());
    }

    #[test]
    fn form_values_are_encoded() {
        assert_eq!(encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    }
}
