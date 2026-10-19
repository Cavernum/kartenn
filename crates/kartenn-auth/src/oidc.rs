//! The OIDC flow of the BFF (spec 005, REQ-005-4 to 8; ADR 0020): kartenn is a
//! confidential client; the browser holds a sealed cookie only.
//!
//! - [`Oidc::begin`]: the authorization URL (code, PKCE S256, `state`, `nonce`) and a
//!   sealed cookie of the login in progress, valid ten minutes (RFC 9700 §2.1.1, §4.7.1).
//! - [`Oidc::complete`]: the callback checked (`state`, then `iss` of RFC 9207 when the
//!   issuer publishes it, §4.4.2), the code exchanged, the ID token validated with its
//!   `nonce` (OIDC Core §3.1.3.7).
//! - [`Session`]: the sealed session cookie, its idle and absolute lifetimes (K10), the
//!   refresh of its groups, the RP-Initiated Logout URL.

use std::collections::HashMap;
use std::fmt::Write as _;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::http::{HttpClient, encode};
use crate::issuer::{Discovery, IssuerConfig};
use crate::keys::{KeyRing, Purpose};
use crate::seal::{open, seal};
use crate::tokens::{KeySource, TokenError, TokenVerifier};

/// The cookie of a login in progress.
pub const LOGIN_COOKIE: &str = "__Host-kartenn-login";
/// The session cookie (its chunks are `__Host-kartenn-1`… when it is split).
pub const SESSION_COOKIE: &str = "__Host-kartenn";
/// How long a login may take.
pub const LOGIN_WITHIN: i64 = 600;
/// Most seconds a break-glass session lives (REQ-005-15, REQ-006-24).
pub const BREAKGLASS_LIFETIME: i64 = 3_600;

/// Why a login fails; [`LoginError::reason`] is recorded in the audit.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LoginError {
    /// No issuer of that name.
    #[error("unknown issuer")]
    UnknownIssuer,
    /// The issuer cannot be reached, or answers out of protocol.
    #[error("issuer unavailable: {0}")]
    Unavailable(String),
    /// No login cookie, a forged or expired one.
    #[error("no valid login in progress")]
    Transaction,
    /// The `state` differs (CSRF, RFC 9700 §4.7.1).
    #[error("state")]
    State,
    /// The response comes from another issuer (mix-up, RFC 9207 §2.4).
    #[error("issuer mix-up")]
    Issuer,
    /// The issuer refused (`error` in the callback).
    #[error("refused by the issuer: {0}")]
    Refused(String),
    /// The code exchange failed.
    #[error("code exchange: {0}")]
    Exchange(String),
    /// The ID token is refused (signature, audience, expiry, nonce).
    #[error("ID token: {0}")]
    IdToken(TokenError),
    /// A refreshed ID token names another subject (OIDC Core §12.2).
    #[error("another subject")]
    Subject,
}

impl LoginError {
    /// The coded reason of the audit.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::UnknownIssuer => "issuer_unknown",
            Self::Unavailable(_) => "issuer_unavailable",
            Self::Transaction => "transaction",
            Self::State => "state",
            Self::Issuer => "iss",
            Self::Refused(_) => "refused",
            Self::Exchange(_) => "exchange",
            Self::IdToken(e) => e.reason(),
            Self::Subject => "subject",
        }
    }
}

/// A login in progress, sealed in [`LOGIN_COOKIE`].
#[derive(Debug, Serialize, Deserialize)]
struct Transaction {
    issuer: String,
    state: String,
    nonce: String,
    verifier: String,
    started: i64,
}

/// What [`Oidc::begin`] gives the browser.
#[derive(Debug)]
pub struct LoginStart {
    /// Where to redirect it.
    pub url: String,
    /// The chunks of the login cookie.
    pub cookie: Vec<String>,
}

/// The parameters of the callback.
#[derive(Debug, Default, Clone)]
pub struct Callback {
    /// The code.
    pub code: Option<String>,
    /// The state.
    pub state: Option<String>,
    /// The issuer (RFC 9207).
    pub iss: Option<String>,
    /// An error of the issuer.
    pub error: Option<String>,
}

/// A login completed: who, and the tokens the session keeps.
#[derive(Debug, Clone)]
pub struct LoggedIn {
    /// Local name of the issuer.
    pub issuer_id: String,
    /// Issuer identifier.
    pub issuer: String,
    /// Subject.
    pub subject: String,
    /// Display name.
    pub name: Option<String>,
    /// Groups.
    pub groups: Vec<String>,
    /// For the refresh of the groups.
    pub refresh_token: Option<String>,
    /// For the logout (`id_token_hint`).
    pub id_token: String,
    /// Seconds before the access token of the issuer expires.
    pub expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

fn random_text() -> String {
    let mut bits = [0u8; 32];
    getrandom::fill(&mut bits).expect("the system provides randomness");
    URL_SAFE_NO_PAD.encode(bits)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The OIDC client of the BFF.
pub struct Oidc<S> {
    tokens: TokenVerifier<S>,
    http: HttpClient,
    ring: KeyRing,
    redirect_uri: String,
    discoveries: Mutex<HashMap<String, Discovery>>,
}

impl<S> std::fmt::Debug for Oidc<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Oidc")
            .field("redirect_uri", &self.redirect_uri)
            .finish_non_exhaustive()
    }
}

impl<S> Oidc<S> {
    /// The verifier of the tokens of its issuers, shared with the access tokens.
    #[must_use]
    pub const fn tokens(&self) -> &TokenVerifier<S> {
        &self.tokens
    }
}

impl<S: KeySource> Oidc<S> {
    /// For the issuers of `tokens`, redirecting to `redirect_uri` (compared exactly by the
    /// issuer, RFC 9700 §4.1.3).
    #[must_use]
    pub fn new(
        tokens: TokenVerifier<S>,
        http: HttpClient,
        ring: KeyRing,
        redirect_uri: String,
    ) -> Self {
        Self {
            tokens,
            http,
            ring,
            redirect_uri,
            discoveries: Mutex::new(HashMap::new()),
        }
    }

    async fn discovery(&self, issuer: &IssuerConfig) -> Result<Discovery, LoginError> {
        if let Some(found) = self.discoveries.lock().await.get(&issuer.id) {
            return Ok(found.clone());
        }
        let found: Discovery = self
            .http
            .get_json(&issuer.discovery_url())
            .await
            .map_err(|e| LoginError::Unavailable(e.to_string()))?;
        if found.issuer != issuer.issuer {
            return Err(LoginError::Unavailable(
                "the discovery names another issuer".into(),
            ));
        }
        self.discoveries
            .lock()
            .await
            .insert(issuer.id.clone(), found.clone());
        Ok(found)
    }

    /// Reads again the discovery of every issuer read so far, then the keys of the tokens
    /// (the local timer of REQ-006-38, every 5 min); an issuer that does not answer keeps
    /// what was read. Returns the failures, by issuer.
    pub async fn reload(&self) -> Vec<(String, String)> {
        let read: Vec<String> = self.discoveries.lock().await.keys().cloned().collect();
        let mut failures = Vec::new();
        for id in read {
            let Some(issuer) = self.tokens.issuer(&id) else {
                continue;
            };
            let found = self
                .http
                .get_json::<Discovery>(&issuer.discovery_url())
                .await
                .map_err(|e| e.to_string())
                .and_then(|found| {
                    if found.issuer == issuer.issuer {
                        Ok(found)
                    } else {
                        Err("the discovery names another issuer".to_owned())
                    }
                });
            match found {
                Ok(found) => {
                    self.discoveries.lock().await.insert(id, found);
                }
                Err(e) => failures.push((issuer.issuer.clone(), e)),
            }
        }
        failures.extend(
            self.tokens
                .reload()
                .await
                .into_iter()
                .map(|(issuer, e)| (issuer, e.to_string())),
        );
        failures
    }

    /// Starts a login at the issuer `issuer_id`.
    ///
    /// # Errors
    ///
    /// [`LoginError::UnknownIssuer`], [`LoginError::Unavailable`].
    pub async fn begin(&self, issuer_id: &str) -> Result<LoginStart, LoginError> {
        let issuer = self
            .tokens
            .issuer(issuer_id)
            .ok_or(LoginError::UnknownIssuer)?;
        let discovery = self.discovery(issuer).await?;
        let transaction = Transaction {
            issuer: issuer.id.clone(),
            state: random_text(),
            nonce: random_text(),
            verifier: random_text(),
            started: now(),
        };
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(transaction.verifier.as_bytes()));
        let url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
            discovery.authorization_endpoint,
            encode(&issuer.client_id),
            encode(&self.redirect_uri),
            encode("openid profile"),
            transaction.state,
            transaction.nonce,
            challenge
        );
        let plaintext =
            serde_json::to_vec(&transaction).map_err(|e| LoginError::Unavailable(e.to_string()))?;
        let cookie = seal(&self.ring, Purpose::Login, LOGIN_COOKIE, &plaintext)
            .map_err(|e| LoginError::Unavailable(e.to_string()))?;
        Ok(LoginStart { url, cookie })
    }

    /// Completes a login from its callback and its login cookie.
    ///
    /// # Errors
    ///
    /// [`LoginError`], whose [`LoginError::reason`] the audit records.
    pub async fn complete(
        &self,
        callback: &Callback,
        login_cookie: &[&str],
    ) -> Result<LoggedIn, LoginError> {
        let transaction: Transaction = open(&self.ring, Purpose::Login, LOGIN_COOKIE, login_cookie)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or(LoginError::Transaction)?;
        if now() - transaction.started > LOGIN_WITHIN {
            return Err(LoginError::Transaction);
        }
        let state_matches = callback
            .state
            .as_deref()
            .is_some_and(|s| s.as_bytes().ct_eq(transaction.state.as_bytes()).into());
        if !state_matches {
            return Err(LoginError::State);
        }
        let issuer = self
            .tokens
            .issuer(&transaction.issuer)
            .ok_or(LoginError::UnknownIssuer)?;
        let discovery = self.discovery(issuer).await?;
        // RFC 9207: the issuer the login went to, and no other.
        match (
            &callback.iss,
            discovery.authorization_response_iss_parameter_supported,
        ) {
            (Some(iss), _) if *iss != issuer.issuer => return Err(LoginError::Issuer),
            (None, true) => return Err(LoginError::Issuer),
            _ => {}
        }
        if let Some(error) = &callback.error {
            return Err(LoginError::Refused(error.clone()));
        }
        let code = callback.code.as_deref().ok_or(LoginError::Transaction)?;
        let response: TokenResponse = self
            .http
            .post_form(
                &discovery.token_endpoint,
                &[
                    ("grant_type", "authorization_code"),
                    ("code", code),
                    ("redirect_uri", &self.redirect_uri),
                    ("code_verifier", &transaction.verifier),
                ],
                Some((&issuer.client_id, &issuer.client_secret)),
            )
            .await
            .map_err(|e| LoginError::Exchange(e.to_string()))?;
        let id_token = response
            .id_token
            .ok_or_else(|| LoginError::Exchange("no ID token".into()))?;
        let verified = self
            .tokens
            .verify_id_token(issuer, &id_token, Some(&transaction.nonce))
            .await
            .map_err(LoginError::IdToken)?;
        Ok(LoggedIn {
            issuer_id: issuer.id.clone(),
            issuer: verified.issuer,
            subject: verified.subject,
            name: verified.name,
            groups: verified.groups,
            refresh_token: response.refresh_token,
            id_token,
            expires_in: response.expires_in.unwrap_or(300),
        })
    }

    /// Refreshes the groups of `session` at its issuer with its refresh token
    /// (REQ-005-7); the session keeps its groups when the issuer gives no new ID token.
    ///
    /// # Errors
    ///
    /// [`LoginError`]: the session is then closed.
    pub async fn refresh(&self, session: &mut Session) -> Result<(), LoginError> {
        let issuer = self
            .tokens
            .issuer(&session.issuer_id)
            .ok_or(LoginError::UnknownIssuer)?;
        let refresh_token = session
            .refresh_token
            .clone()
            .ok_or_else(|| LoginError::Exchange("no refresh token".into()))?;
        let discovery = self.discovery(issuer).await?;
        let response: TokenResponse = self
            .http
            .post_form(
                &discovery.token_endpoint,
                &[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", &refresh_token),
                ],
                Some((&issuer.client_id, &issuer.client_secret)),
            )
            .await
            .map_err(|e| LoginError::Exchange(e.to_string()))?;
        if let Some(id_token) = response.id_token {
            // A refreshed ID token carries no nonce (OIDC Core §12.2): checked without it.
            let verified = self
                .tokens
                .verify_id_token(issuer, &id_token, None)
                .await
                .map_err(LoginError::IdToken)?;
            if verified.subject != session.subject {
                return Err(LoginError::Subject);
            }
            session.groups = verified.groups;
            session.id_token = Some(id_token);
        }
        if response.refresh_token.is_some() {
            session.refresh_token = response.refresh_token;
        }
        session.refresh_at = now() + response.expires_in.unwrap_or(300);
        Ok(())
    }

    /// Where to send the browser to log out at the issuer too (RP-Initiated Logout §2),
    /// if it publishes an end-session endpoint.
    ///
    /// # Errors
    ///
    /// [`LoginError::Unavailable`] when the discovery cannot be read.
    pub async fn logout_url(
        &self,
        session: &Session,
        after: Option<&str>,
    ) -> Result<Option<String>, LoginError> {
        let issuer = self
            .tokens
            .issuer(&session.issuer_id)
            .ok_or(LoginError::UnknownIssuer)?;
        let discovery = self.discovery(issuer).await?;
        Ok(discovery.end_session_endpoint.map(|end| {
            let mut url = format!("{end}?client_id={}", encode(&issuer.client_id));
            if let Some(hint) = &session.id_token {
                let _ = write!(url, "&id_token_hint={}", encode(hint));
            }
            if let Some(after) = after {
                let _ = write!(url, "&post_logout_redirect_uri={}", encode(after));
            }
            url
        }))
    }
}

/// Why a session cookie is refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SessionError {
    /// Missing, forged, or sealed by a removed key.
    #[error("no valid session")]
    Invalid,
    /// Idle too long.
    #[error("idle")]
    Idle,
    /// Older than its absolute lifetime.
    #[error("expired")]
    Expired,
}

/// The session of a browser, sealed in [`SESSION_COOKIE`] (K10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The identity in kartenn.
    pub identity: Uuid,
    /// Local name of the issuer.
    pub issuer_id: String,
    /// Issuer identifier.
    pub issuer: String,
    /// Subject.
    pub subject: String,
    /// Groups.
    pub groups: Vec<String>,
    /// Opened at (Unix seconds).
    pub started: i64,
    /// Last activity (Unix seconds).
    pub seen: i64,
    /// Refresh token of the issuer.
    pub refresh_token: Option<String>,
    /// ID token, for the logout.
    pub id_token: Option<String>,
    /// When the groups are to be refreshed (Unix seconds).
    pub refresh_at: i64,
    /// A break-glass session: no issuer, at most [`BREAKGLASS_LIFETIME`].
    #[serde(default)]
    pub breakglass: bool,
}

impl Session {
    /// A new session of `identity` after `login`, now.
    #[must_use]
    pub fn new(identity: Uuid, login: &LoggedIn) -> Self {
        let at = now();
        Self {
            identity,
            issuer_id: login.issuer_id.clone(),
            issuer: login.issuer.clone(),
            subject: login.subject.clone(),
            groups: login.groups.clone(),
            started: at,
            seen: at,
            refresh_token: login.refresh_token.clone(),
            id_token: Some(login.id_token.clone()),
            refresh_at: at + login.expires_in.min(900),
            breakglass: false,
        }
    }

    /// A new break-glass session of `identity`, now: nothing to refresh.
    #[must_use]
    pub fn breakglass(identity: Uuid) -> Self {
        let at = now();
        Self {
            identity,
            issuer_id: String::new(),
            issuer: String::new(),
            subject: String::new(),
            groups: Vec::new(),
            started: at,
            seen: at,
            refresh_token: None,
            id_token: None,
            refresh_at: i64::MAX,
            breakglass: true,
        }
    }

    /// Seals it into its cookie chunks.
    ///
    /// # Errors
    ///
    /// [`crate::seal::SealError::TooLarge`] beyond three cookies.
    pub fn seal(&self, ring: &KeyRing) -> Result<Vec<String>, crate::seal::SealError> {
        let plaintext = serde_json::to_vec(self).map_err(|_| crate::seal::SealError::Malformed)?;
        seal(ring, Purpose::Session, SESSION_COOKIE, &plaintext)
    }

    /// Opens a session cookie at `at` (Unix seconds), within `idle` and `absolute`
    /// seconds; marks it seen.
    ///
    /// # Errors
    ///
    /// [`SessionError`].
    pub fn open(
        ring: &KeyRing,
        chunks: &[&str],
        at: i64,
        idle: i64,
        absolute: i64,
    ) -> Result<Self, SessionError> {
        let mut session = Self::unseal(ring, chunks)?;
        session.check(at, idle, absolute)?;
        Ok(session)
    }

    /// Opens a session cookie, without checking its lifetimes: [`Self::check`] then, so
    /// that the caller knows whose session ended.
    ///
    /// # Errors
    ///
    /// [`SessionError::Invalid`].
    pub fn unseal(ring: &KeyRing, chunks: &[&str]) -> Result<Self, SessionError> {
        open(ring, Purpose::Session, SESSION_COOKIE, chunks)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or(SessionError::Invalid)
    }

    /// Whether it lives at `at`, within `idle` and `absolute` seconds (a break-glass
    /// session at most [`BREAKGLASS_LIFETIME`]); marks it seen.
    ///
    /// # Errors
    ///
    /// [`SessionError::Idle`], [`SessionError::Expired`].
    pub fn check(&mut self, at: i64, idle: i64, absolute: i64) -> Result<(), SessionError> {
        let absolute = self.lifetime(absolute);
        if at - self.started >= absolute {
            return Err(SessionError::Expired);
        }
        if at - self.seen >= idle {
            return Err(SessionError::Idle);
        }
        self.seen = at;
        Ok(())
    }

    /// Its absolute lifetime under `absolute` seconds: at most [`BREAKGLASS_LIFETIME`]
    /// for a break-glass session.
    const fn lifetime(&self, absolute: i64) -> i64 {
        if self.breakglass && BREAKGLASS_LIFETIME < absolute {
            BREAKGLASS_LIFETIME
        } else {
            absolute
        }
    }

    /// When it ends (Unix seconds) unless seen again: by inactivity after `idle`, or at
    /// the end of its lifetime under `absolute`, whichever comes first.
    #[must_use]
    pub const fn ends_at(&self, idle: i64, absolute: i64) -> i64 {
        let by_idle = self.seen.saturating_add(idle);
        let by_age = self.started.saturating_add(self.lifetime(absolute));
        if by_idle < by_age { by_idle } else { by_age }
    }

    /// Whether its groups are due for a refresh at `at`.
    #[must_use]
    pub const fn refresh_due(&self, at: i64) -> bool {
        at >= self.refresh_at
    }
}
