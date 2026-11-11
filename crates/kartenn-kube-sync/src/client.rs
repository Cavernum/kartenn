//! The client of an API server (Q-019-12): `kube` without its TLS stack, over a TLS
//! stack built here on rustls and ring, that trusts the configured CA alone (no system
//! roots, no switch to skip verification). The token is read again from its file when it
//! changes, and sent in a header marked sensitive; the client certificate is read again
//! by the handshake that follows a change.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use http::{HeaderValue, Request, header::AUTHORIZATION};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use kube::client::middleware::BaseUriLayer;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use tower::{BoxError, Layer, Service, ServiceBuilder};

use crate::config::{Cluster, Credentials};
use crate::credentials::{ClientCertFiles, CredentialError, CredentialFile, provider};

/// The time allowed to open a connection to the API server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Why the client of a cluster cannot be built; the connector does not start.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConnectError {
    /// The configuration was not checked ([`crate::config::load`]).
    #[error("cluster {0}: configuration not checked")]
    Unchecked(String),
    /// The CA file is unreadable or holds no certificate.
    #[error("cluster {slug}: CA file {}: {reason}", file.display())]
    Ca {
        /// The slug of the cluster.
        slug: String,
        /// The file.
        file: PathBuf,
        /// What is wrong, without the content.
        reason: String,
    },
    /// A credential file cannot be used.
    #[error("cluster {slug}: {source}")]
    Credential {
        /// The slug of the cluster.
        slug: String,
        /// Why.
        #[source]
        source: CredentialError,
    },
    /// The TLS configuration is refused by rustls.
    #[error("cluster {slug}: TLS: {source}")]
    Tls {
        /// The slug of the cluster.
        slug: String,
        /// Why.
        #[source]
        source: rustls::Error,
    },
}

/// The client of the API server of `cluster`, over TLS. Called inside a Tokio runtime.
///
/// # Errors
///
/// [`ConnectError`] for an unchecked configuration, an unusable CA or credential.
pub fn connect(cluster: &Cluster) -> Result<kube::Client, ConnectError> {
    let slug = || cluster.slug.clone();
    let (Some(ca), Some(credentials)) = (cluster.ca(), cluster.credentials()) else {
        return Err(ConnectError::Unchecked(slug()));
    };
    let roots = roots(ca).map_err(|reason| ConnectError::Ca {
        slug: slug(),
        file: ca.to_owned(),
        reason,
    })?;
    let builder = rustls::ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|source| ConnectError::Tls {
            slug: slug(),
            source,
        })?
        .with_root_certificates(roots);
    let tls = match credentials {
        Credentials::Token(_) => builder.with_no_client_auth(),
        Credentials::Certificate {
            cert_file,
            key_file,
        } => {
            let files = ClientCertFiles::new(cert_file, key_file, provider());
            // Read once now, so that an unusable certificate stops the start.
            files.current().map_err(|source| ConnectError::Credential {
                slug: slug(),
                source,
            })?;
            builder.with_client_cert_resolver(Arc::new(files))
        }
    };
    let mut http = HttpConnector::new();
    http.enforce_http(false);
    http.set_connect_timeout(Some(CONNECT_TIMEOUT));
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls)
        .https_only()
        .enable_http1()
        .wrap_connector(http);
    let inner = hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(https);
    connect_with(cluster, inner)
}

/// The client of `cluster` over `inner`, a service that sends the requests (the TLS
/// client of [`connect`], or a fake API server in tests). Called inside a Tokio runtime.
///
/// # Errors
///
/// [`ConnectError::Unchecked`] for an unchecked configuration.
pub fn connect_with<S, B>(cluster: &Cluster, inner: S) -> Result<kube::Client, ConnectError>
where
    S: Service<Request<kube::client::Body>, Response = http::Response<B>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    let unchecked = || ConnectError::Unchecked(cluster.slug.clone());
    let base = cluster
        .api_server
        .parse::<http::Uri>()
        .map_err(|_| unchecked())?;
    let bearer = match cluster.credentials().ok_or_else(unchecked)? {
        Credentials::Token(file) => Some(BearerLayer::new(file)),
        Credentials::Certificate { .. } => None,
    };
    let service = ServiceBuilder::new()
        .layer(BaseUriLayer::new(base))
        .option_layer(bearer)
        .map_err(Into::<BoxError>::into)
        .service(inner);
    Ok(kube::Client::new(service, "default"))
}

/// The certificates of `file`, at least one.
fn roots(file: &Path) -> Result<rustls::RootCertStore, String> {
    let certificates = CertificateDer::pem_file_iter(file)
        .map_err(|_| "unreadable PEM file".to_owned())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "malformed PEM certificate".to_owned())?;
    if certificates.is_empty() {
        return Err("no certificate".into());
    }
    let mut roots = rustls::RootCertStore::empty();
    for certificate in certificates {
        roots
            .add(certificate)
            .map_err(|_| "an unusable certificate".to_owned())?;
    }
    Ok(roots)
}

/// Sets `Authorization: Bearer` from a token file read again when it changes.
#[derive(Debug, Clone)]
struct BearerLayer {
    token: Arc<CredentialFile>,
}

impl BearerLayer {
    fn new(file: &Path) -> Self {
        Self {
            token: Arc::new(CredentialFile::new(file)),
        }
    }
}

impl<S> Layer<S> for BearerLayer {
    type Service = Bearer<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Bearer {
            inner,
            token: Arc::clone(&self.token),
        }
    }
}

#[derive(Debug, Clone)]
struct Bearer<S> {
    inner: S,
    token: Arc<CredentialFile>,
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = Result<T, BoxError>> + Send>>;

impl<S, B> Service<Request<B>> for Bearer<S>
where
    S: Service<Request<B>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = BoxFuture<S::Response>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        // The service made ready is the one that sends; a clone takes its place.
        let ready = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, ready);
        let token = Arc::clone(&self.token);
        Box::pin(async move {
            let secret = token.read().await?;
            let mut value =
                HeaderValue::try_from(format!("Bearer {}", secret.expose())).map_err(|_| {
                    CredentialError::NotAHeader {
                        file: token.path().to_owned(),
                    }
                })?;
            value.set_sensitive(true);
            request.headers_mut().insert(AUTHORIZATION, value);
            inner.call(request).await.map_err(Into::into)
        })
    }
}
