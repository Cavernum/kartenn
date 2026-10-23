//! Access tokens (spec 005, REQ-005-9, 10; task 005-11): every refusal of RFC 9068 §4 and
//! RFC 8725, and the keys of an issuer reloaded on rotation, at most once a minute.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use kartenn_auth::issuer::IssuerConfig;
use kartenn_auth::tokens::{KeySource, TokenError, TokenVerifier};
use p256::pkcs8::EncodePrivateKey;
use serde_json::{Value, json};

const ISSUER: &str = "https://idp.example.invalid";

/// A signing key and its public JWK under `kid`.
fn key(kid: &str) -> (EncodingKey, Jwk) {
    let secret = p256::SecretKey::random(&mut rand_core::OsRng);
    let pem = secret
        .to_pkcs8_pem(p256::pkcs8::LineEnding::default())
        .unwrap();
    let encoding = EncodingKey::from_ec_pem(pem.as_bytes()).unwrap();
    let mut jwk = Jwk::from_encoding_key(&encoding, Algorithm::ES256).unwrap();
    jwk.common.key_id = Some(kid.into());
    (encoding, jwk)
}

/// The keys published by the issuer, and how often they are fetched.
#[derive(Default)]
struct Published {
    keys: Mutex<Vec<Jwk>>,
    fetched: AtomicUsize,
}

impl KeySource for &Published {
    fn keys(
        &self,
        _: &IssuerConfig,
    ) -> impl std::future::Future<Output = Result<JwkSet, TokenError>> + Send {
        self.fetched.fetch_add(1, Ordering::Relaxed);
        std::future::ready(Ok(JwkSet {
            keys: self.keys.lock().unwrap().clone(),
        }))
    }
}

fn issuer() -> IssuerConfig {
    IssuerConfig {
        id: "main".into(),
        issuer: ISSUER.into(),
        client_id: "kartenn".into(),
        client_secret: "s".into(),
        audience: "kartenn".into(),
        groups_claim: "groups".into(),
        algorithms: vec![Algorithm::ES256],
        native: None,
    }
}

fn claims() -> Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    json!({
        "iss": ISSUER, "aud": "kartenn", "sub": "u-1", "client_id": "cli",
        "iat": now, "exp": now + 300, "jti": "j-1",
        "name": "A. Person", "groups": ["netops", "readers"]
    })
}

fn token(key: &EncodingKey, kid: &str, typ: &str, claims: &Value) -> String {
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(kid.into());
    header.typ = Some(typ.into());
    encode(&header, claims, key).unwrap()
}

#[tokio::test]
async fn a_valid_access_token_gives_its_subject_and_groups() {
    let (signing, jwk) = key("k1");
    let published = Published::default();
    published.keys.lock().unwrap().push(jwk);
    let tokens = TokenVerifier::new(vec![issuer()], &published);
    let verified = tokens
        .verify(&token(&signing, "k1", "at+jwt", &claims()))
        .await
        .unwrap();
    assert_eq!(verified.subject, "u-1");
    assert_eq!(verified.issuer, ISSUER);
    assert_eq!(verified.name.as_deref(), Some("A. Person"));
    assert_eq!(verified.groups, ["netops", "readers"]);
}

#[tokio::test]
async fn every_refusal_of_rfc_9068() {
    let (signing, jwk) = key("k1");
    let published = Published::default();
    published.keys.lock().unwrap().push(jwk);
    let verifier = TokenVerifier::new(vec![issuer()], &published);
    let with = |change: &dyn Fn(&mut Value)| {
        let mut c = claims();
        change(&mut c);
        c
    };
    // An ID token presented as an access token (RFC 8725 §3.12).
    assert_eq!(
        verifier
            .verify(&token(&signing, "k1", "JWT", &claims()))
            .await,
        Err(TokenError::NotAccessToken)
    );
    // An issuer kartenn does not know, even with a valid key of its own.
    let foreign = with(&|c| c["iss"] = "https://other.example.invalid".into());
    assert_eq!(
        verifier
            .verify(&token(&signing, "k1", "at+jwt", &foreign))
            .await,
        Err(TokenError::UnknownIssuer)
    );
    // An algorithm outside the issuer's list (here a MAC with a guessed key).
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("k1".into());
    header.typ = Some("at+jwt".into());
    let mac = encode(&header, &claims(), &EncodingKey::from_secret(b"guess")).unwrap();
    assert_eq!(verifier.verify(&mac).await, Err(TokenError::Algorithm));
    // Another audience, an expired token, a signature by an unpublished key, no subject:
    // each with its coded reason for the audit.
    let (rogue, _) = key("k1");
    let anonymous = with(&|c| {
        c.as_object_mut().unwrap().remove("sub");
    });
    for (signing, claims, refusal, reason) in [
        (
            &signing,
            with(&|c| c["aud"] = "another-service".into()),
            TokenError::Audience,
            "audience",
        ),
        (
            &signing,
            with(&|c| c["exp"] = (c["iat"].as_u64().unwrap() - 600).into()),
            TokenError::Expired,
            "expired",
        ),
        (&rogue, claims(), TokenError::Signature, "signature"),
    ] {
        let error = verifier
            .verify(&token(signing, "k1", "at+jwt", &claims))
            .await
            .unwrap_err();
        assert_eq!(error, refusal);
        assert_eq!(error.reason(), reason);
    }
    let error = verifier
        .verify(&token(&signing, "k1", "at+jwt", &anonymous))
        .await
        .unwrap_err();
    assert_eq!(error.reason(), "claims", "{error}");
}

#[tokio::test]
async fn a_rotation_is_followed_and_unknown_keys_reload_once() {
    let (old, old_jwk) = key("k1");
    let published = Published::default();
    published.keys.lock().unwrap().push(old_jwk);
    let verifier = TokenVerifier::new(vec![issuer()], &published);
    assert!(
        verifier
            .verify(&token(&old, "k1", "at+jwt", &claims()))
            .await
            .is_ok()
    );
    assert_eq!(published.fetched.load(Ordering::Relaxed), 1);
    // The issuer rotates: a token of the new key is accepted without restart.
    let (new, new_jwk) = key("k2");
    published.keys.lock().unwrap().push(new_jwk);
    assert!(
        verifier
            .verify(&token(&new, "k2", "at+jwt", &claims()))
            .await
            .is_ok()
    );
    assert_eq!(published.fetched.load(Ordering::Relaxed), 2);
    // A thousand tokens of an unknown key cause no more reload within the minute: the
    // rotation used the one reload a minute that unknown keys may cause.
    let (stray, _) = key("k3");
    for _ in 0..1_000 {
        assert_eq!(
            verifier
                .verify(&token(&stray, "k3", "at+jwt", &claims()))
                .await,
            Err(TokenError::UnknownKey)
        );
    }
    assert_eq!(
        published.fetched.load(Ordering::Relaxed),
        2,
        "no reload for k3"
    );
}
