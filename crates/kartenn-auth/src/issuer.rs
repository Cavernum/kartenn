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

/// The algorithm named `name` if kartenn accepts it for the tokens of an issuer: signatures
/// by a public key only, never `none` nor a MAC whose key the clients share (RFC 8725 §3.1,
/// §3.2; a choice of kartenn).
#[must_use]
pub fn accepted_algorithm(name: &str) -> Option<Algorithm> {
    use Algorithm::{ES256, ES384, EdDSA, PS256, PS384, PS512, RS256, RS384, RS512};
    let algorithm: Algorithm = name.parse().ok()?;
    matches!(
        algorithm,
        RS256 | RS384 | RS512 | PS256 | PS384 | PS512 | ES256 | ES384 | EdDSA
    )
    .then_some(algorithm)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_key_signatures_are_accepted() {
        assert_eq!(accepted_algorithm("RS256"), Some(Algorithm::RS256));
        assert_eq!(accepted_algorithm("EdDSA"), Some(Algorithm::EdDSA));
        for refused in ["none", "HS256", "HS512", "rs256", ""] {
            assert_eq!(accepted_algorithm(refused), None, "{refused}");
        }
    }
}
