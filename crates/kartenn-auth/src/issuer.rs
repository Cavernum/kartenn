//! OIDC issuers (spec 005, K11): their technical declaration, from the configuration, and
//! their discovery document (OpenID Connect Discovery 1.0).

use std::fmt;

use jsonwebtoken::Algorithm;
use serde::Deserialize;

/// An issuer, as configured (its confinement lives in kartenn, K11).
#[derive(Clone)]
pub struct IssuerConfig {
    /// Local name.
    pub id: String,
    /// Issuer identifier (`iss`), compared exactly.
    pub issuer: String,
    /// Client of the BFF at this issuer.
    pub client_id: String,
    /// Its secret (confidential client); never printed.
    pub client_secret: String,
    /// Audience of the access tokens of the clients other than browsers.
    pub audience: String,
    /// Claim holding the groups (`groups` by default).
    pub groups_claim: String,
    /// Algorithms accepted for its tokens (a closed list, RFC 8725 §3.1).
    pub algorithms: Vec<Algorithm>,
}

impl fmt::Debug for IssuerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IssuerConfig")
            .field("id", &self.id)
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field("client_secret", &"***")
            .field("audience", &self.audience)
            .field("groups_claim", &self.groups_claim)
            .field("algorithms", &self.algorithms)
            .finish()
    }
}

/// The endpoints of an issuer.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Discovery {
    /// Issuer identifier: must equal the configured one.
    pub issuer: String,
    /// Authorization endpoint.
    pub authorization_endpoint: String,
    /// Token endpoint.
    pub token_endpoint: String,
    /// Keys.
    pub jwks_uri: String,
    /// RP-Initiated Logout, if supported.
    #[serde(default)]
    pub end_session_endpoint: Option<String>,
    /// RFC 9207 support: the `iss` parameter of authorization responses.
    #[serde(default)]
    pub authorization_response_iss_parameter_supported: bool,
}

impl IssuerConfig {
    /// Where its discovery document lies.
    #[must_use]
    pub fn discovery_url(&self) -> String {
        format!(
            "{}/.well-known/openid-configuration",
            self.issuer.trim_end_matches('/')
        )
    }
}
