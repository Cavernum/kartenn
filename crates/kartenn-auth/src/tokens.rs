//! Access tokens of the clients other than browsers (spec 005, REQ-005-9, 10): JWTs of an
//! issuer validated per RFC 9068 §4 (`typ` `at+jwt`, exact `iss`, audience of kartenn,
//! signature by a key of the issuer and an algorithm of its closed list, `exp` with at
//! most a minute of leeway); keys cached per issuer and reloaded on an unknown `kid` at
//! most once a minute (RFC 8725 §3.8).

use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::issuer::IssuerConfig;

/// Leeway on `exp` (RFC 9068 §4: a few minutes at most).
const LEEWAY: u64 = 60;
/// Least time between two reloads of the keys of an issuer.
const RELOAD_EVERY: Duration = Duration::from_mins(1);

/// Why a token is refused; the caller answers `invalid_token` (RFC 6750 §3.1) whatever
/// the reason.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TokenError {
    /// Not a JWT, or no `kid`.
    #[error("malformed token")]
    Malformed,
    /// `typ` is not `at+jwt`: an ID token or another kind (RFC 8725 §3.12).
    #[error("not an access token")]
    NotAccessToken,
    /// No configured issuer of that name.
    #[error("unknown issuer")]
    UnknownIssuer,
    /// An algorithm outside the issuer's list.
    #[error("algorithm not accepted")]
    Algorithm,
    /// No key of the issuer with that `kid`, after a reload.
    #[error("unknown key")]
    UnknownKey,
    /// A signature that no key of the issuer verifies.
    #[error("signature")]
    Signature,
    /// Expired, or not valid yet.
    #[error("expired")]
    Expired,
    /// An audience other than kartenn (or than its client, for an ID token).
    #[error("audience")]
    Audience,
    /// The nonce of another login (OIDC Core §3.1.3.7, point 11).
    #[error("nonce")]
    Nonce,
    /// Another refused claim: issuer, missing subject, authorised party.
    #[error("{0}")]
    Claims(String),
    /// The keys cannot be fetched.
    #[error("keys: {0}")]
    Keys(String),
}

impl TokenError {
    /// The coded reason of the audit (spec 005, design « Audit »).
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::NotAccessToken => "token_type",
            Self::UnknownIssuer => "issuer_unknown",
            Self::Algorithm => "algorithm",
            Self::UnknownKey | Self::Signature => "signature",
            Self::Expired => "expired",
            Self::Audience => "audience",
            Self::Nonce => "nonce",
            Self::Claims(_) => "claims",
            Self::Keys(_) => "issuer_unavailable",
        }
    }
}

/// A refusal of `jsonwebtoken`, by its kind.
fn refused(error: &jsonwebtoken::errors::Error) -> TokenError {
    use jsonwebtoken::errors::ErrorKind;
    match error.kind() {
        ErrorKind::InvalidSignature => TokenError::Signature,
        ErrorKind::ExpiredSignature | ErrorKind::ImmatureSignature => TokenError::Expired,
        ErrorKind::InvalidAudience => TokenError::Audience,
        _ => TokenError::Claims(error.to_string()),
    }
}

/// A valid access token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedToken {
    /// Issuer.
    pub issuer: String,
    /// Subject at the issuer.
    pub subject: String,
    /// Display name, if the token carries `name`.
    pub name: Option<String>,
    /// Groups, from the issuer's groups claim.
    pub groups: Vec<String>,
}

/// Where the keys of an issuer come from.
pub trait KeySource: Send + Sync {
    /// The current key set of `issuer`.
    fn keys(
        &self,
        issuer: &IssuerConfig,
    ) -> impl Future<Output = Result<JwkSet, TokenError>> + Send;
}

/// Keys from the issuer's discovery document and `jwks_uri`.
#[derive(Debug, Clone)]
pub struct HttpKeys(pub crate::http::HttpClient);

impl KeySource for HttpKeys {
    async fn keys(&self, issuer: &IssuerConfig) -> Result<JwkSet, TokenError> {
        let discovery: crate::issuer::Discovery = self
            .0
            .get_json(&issuer.discovery_url())
            .await
            .map_err(|e| TokenError::Keys(e.to_string()))?;
        if discovery.issuer != issuer.issuer {
            return Err(TokenError::Keys(
                "the discovery names another issuer".into(),
            ));
        }
        self.0
            .get_json(&discovery.jwks_uri)
            .await
            .map_err(|e| TokenError::Keys(e.to_string()))
    }
}

/// The keys of one issuer.
struct Keys {
    by_kid: HashMap<String, DecodingKey>,
    /// Loaded at least once.
    loaded: bool,
    /// The last reload caused by an unknown `kid`: at most one a minute, so that tokens
    /// with invented `kid`s cannot make kartenn hammer the issuer.
    reloaded_for_miss: Option<Instant>,
}

/// Validates access tokens of the configured issuers.
pub struct TokenVerifier<S> {
    issuers: Vec<IssuerConfig>,
    source: S,
    keys: Mutex<HashMap<String, Keys>>,
}

impl<S> std::fmt::Debug for TokenVerifier<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenVerifier")
            .field("issuers", &self.issuers)
            .finish_non_exhaustive()
    }
}

/// The decoding keys of `set` by `kid`; a key without `kid` or unusable is left out.
fn by_kid(set: &JwkSet) -> HashMap<String, DecodingKey> {
    set.keys
        .iter()
        .filter_map(|jwk| {
            let kid = jwk.common.key_id.clone()?;
            DecodingKey::from_jwk(jwk).ok().map(|key| (kid, key))
        })
        .collect()
}

fn is_access_token(typ: Option<&str>) -> bool {
    typ.is_some_and(|t| {
        t.eq_ignore_ascii_case("at+jwt") || t.eq_ignore_ascii_case("application/at+jwt")
    })
}

impl<S: KeySource> TokenVerifier<S> {
    /// For `issuers`, keys from `source`.
    #[must_use]
    pub fn new(issuers: Vec<IssuerConfig>, source: S) -> Self {
        Self {
            issuers,
            source,
            keys: Mutex::new(HashMap::new()),
        }
    }

    /// The decoding key `kid` of `issuer`, reloading its keys once if unknown and the
    /// last reload is older than a minute.
    async fn key(&self, issuer: &IssuerConfig, kid: &str) -> Result<DecodingKey, TokenError> {
        let mut keys = self.keys.lock().await;
        let entry = keys.entry(issuer.issuer.clone()).or_insert_with(|| Keys {
            by_kid: HashMap::new(),
            loaded: false,
            reloaded_for_miss: None,
        });
        if let Some(key) = entry.by_kid.get(kid) {
            return Ok(key.clone());
        }
        if entry.loaded {
            if entry
                .reloaded_for_miss
                .is_some_and(|at| at.elapsed() < RELOAD_EVERY)
            {
                return Err(TokenError::UnknownKey);
            }
            entry.reloaded_for_miss = Some(Instant::now());
        }
        let set = self.source.keys(issuer).await?;
        entry.loaded = true;
        entry.by_kid = by_kid(&set);
        entry.by_kid.get(kid).cloned().ok_or(TokenError::UnknownKey)
    }

    /// Reloads the keys of every issuer loaded so far (the local timer of REQ-006-38, every
    /// 5 min), outside the lock; an issuer that does not answer keeps its keys. Returns the
    /// failures, by issuer.
    pub async fn reload(&self) -> Vec<(String, TokenError)> {
        let loaded: Vec<String> = self
            .keys
            .lock()
            .await
            .iter()
            .filter(|(_, keys)| keys.loaded)
            .map(|(issuer, _)| issuer.clone())
            .collect();
        let mut failures = Vec::new();
        for issuer in self.issuers.iter().filter(|i| loaded.contains(&i.issuer)) {
            match self.source.keys(issuer).await {
                Ok(set) => {
                    if let Some(entry) = self.keys.lock().await.get_mut(&issuer.issuer) {
                        entry.by_kid = by_kid(&set);
                    }
                }
                Err(e) => failures.push((issuer.issuer.clone(), e)),
            }
        }
        failures
    }

    /// Validates `token`.
    ///
    /// # Errors
    ///
    /// [`TokenError`] for any refusal.
    pub async fn verify(&self, token: &str) -> Result<VerifiedToken, TokenError> {
        let header = decode_header(token).map_err(|_| TokenError::Malformed)?;
        if !is_access_token(header.typ.as_deref()) {
            return Err(TokenError::NotAccessToken);
        }
        let kid = header.kid.clone().ok_or(TokenError::Malformed)?;
        // The issuer named by the token chooses the configuration; the validation below
        // checks it again, signed.
        let claimed: Value = jsonwebtoken::dangerous::insecure_decode::<Value>(token)
            .map(|data| data.claims)
            .map_err(|_| TokenError::Malformed)?;
        let iss = claimed["iss"].as_str().ok_or(TokenError::Malformed)?;
        let issuer = self
            .issuers
            .iter()
            .find(|i| i.issuer == iss)
            .ok_or(TokenError::UnknownIssuer)?;
        if !issuer.algorithms.contains(&header.alg) {
            return Err(TokenError::Algorithm);
        }
        let key = self.key(issuer, &kid).await?;
        let mut validation = Validation::new(header.alg);
        validation.algorithms = vec![header.alg];
        validation.set_issuer(&[&issuer.issuer]);
        validation.set_audience(&[&issuer.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = LEEWAY;
        let data = decode::<Value>(token, &key, &validation).map_err(|e| refused(&e))?;
        let claims = data.claims;
        let groups = claims[issuer.groups_claim.as_str()]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|g| g.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok(VerifiedToken {
            issuer: issuer.issuer.clone(),
            subject: claims["sub"].as_str().unwrap_or_default().to_owned(),
            name: claims["name"].as_str().map(str::to_owned),
            groups,
        })
    }

    /// The issuer configured as `id`.
    #[must_use]
    pub fn issuer(&self, id: &str) -> Option<&IssuerConfig> {
        self.issuers.iter().find(|i| i.id == id)
    }

    /// The issuers, in the order of the configuration.
    #[must_use]
    pub fn issuers(&self) -> &[IssuerConfig] {
        &self.issuers
    }

    /// The local identifiers of the issuers, in the order of the configuration.
    pub fn issuer_ids(&self) -> impl Iterator<Item = &str> {
        self.issuers.iter().map(|i| i.id.as_str())
    }

    /// Validates the ID token of a login at `issuer` (OIDC Core §3.1.3.7): exact `iss`,
    /// `aud` holding the client and no audience else, `azp` the client if present, a key of
    /// the issuer and an algorithm of its list, unexpired, and the `nonce` of the login
    /// (`None` for the ID token of a refresh, which carries none, §12.2).
    ///
    /// # Errors
    ///
    /// [`TokenError`] for any refusal; a wrong nonce is [`TokenError::Nonce`].
    pub async fn verify_id_token(
        &self,
        issuer: &IssuerConfig,
        token: &str,
        nonce: Option<&str>,
    ) -> Result<VerifiedToken, TokenError> {
        let header = decode_header(token).map_err(|_| TokenError::Malformed)?;
        if is_access_token(header.typ.as_deref()) {
            return Err(TokenError::Malformed);
        }
        if !issuer.algorithms.contains(&header.alg) {
            return Err(TokenError::Algorithm);
        }
        let kid = header.kid.clone().ok_or(TokenError::Malformed)?;
        let key = self.key(issuer, &kid).await?;
        let mut validation = Validation::new(header.alg);
        validation.algorithms = vec![header.alg];
        validation.set_issuer(&[&issuer.issuer]);
        validation.set_audience(&[&issuer.client_id]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = LEEWAY;
        let claims = decode::<Value>(token, &key, &validation)
            .map_err(|e| refused(&e))?
            .claims;
        // No audience but the client (§3.1.3.7, point 3), and `azp` the client if present.
        let audiences: Vec<&str> = match &claims["aud"] {
            Value::String(a) => vec![a.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        if audiences.iter().any(|a| *a != issuer.client_id) {
            return Err(TokenError::Audience);
        }
        if claims["azp"]
            .as_str()
            .is_some_and(|azp| azp != issuer.client_id)
        {
            return Err(TokenError::Claims("another authorised party".into()));
        }
        if let Some(nonce) = nonce {
            let matches = claims["nonce"].as_str().is_some_and(|n| {
                subtle::ConstantTimeEq::ct_eq(n.as_bytes(), nonce.as_bytes()).into()
            });
            if !matches {
                return Err(TokenError::Nonce);
            }
        }
        Ok(VerifiedToken {
            issuer: issuer.issuer.clone(),
            subject: claims["sub"].as_str().unwrap_or_default().to_owned(),
            name: claims["name"].as_str().map(str::to_owned),
            groups: claims[issuer.groups_claim.as_str()]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|g| g.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}
