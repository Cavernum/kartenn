//! Problem details (RFC 9457, REQ-006-20): one `application/problem+json` body per
//! failed request, with a stable `type` per class of error.

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// Media type of a problem (RFC 9457 §3).
pub const PROBLEM_JSON: &str = "application/problem+json";

/// Prefix of every problem `type`: a URN, an URI reference that needs no domain name
/// (RFC 9457 §3.1.1).
pub const TYPE_PREFIX: &str = "urn:kartenn:problem:";

/// A class of error, its status and its `type` (design 006, « Erreurs »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProblemKind {
    /// Malformed request: invalid JSON, unknown parameter, date without offset.
    BadRequest,
    /// A cursor that does not verify.
    InvalidCursor,
    /// No or invalid credentials.
    Unauthenticated,
    /// A right is missing on a visible object.
    Forbidden,
    /// An unsafe method from another site.
    CrossSiteRequest,
    /// The object is absent or invisible.
    NotFound,
    /// The route does not take the method.
    MethodNotAllowed,
    /// No representation the client accepts (`Accept`).
    NotAcceptable,
    /// The state of the target does not allow the action.
    Conflict,
    /// An undo that conflicts with later changes.
    UndoConflict,
    /// The same idempotency key is being processed.
    IdempotencyInProgress,
    /// `If-Match` does not match.
    PreconditionFailed,
    /// The body is over the limit.
    PayloadTooLarge,
    /// The body has a media type the route does not accept.
    UnsupportedMediaType,
    /// The changeset is refused by the rules.
    ChangesetRefused,
    /// The changeset or the acknowledgement has no motive.
    MotiveRequired,
    /// The changeset has too many operations.
    TooManyOperations,
    /// The idempotency key was used for another content.
    IdempotencyKeyReused,
    /// A member of a write the service does not change, or an undeclared property.
    InvalidMember,
    /// A filter of an output that does not parse, or a duration out of bounds.
    InvalidOverride,
    /// The elements of an import cannot be sorted.
    InvalidImport,
    /// `If-Match` is missing.
    PreconditionRequired,
    /// The quota of the identity is spent.
    RateLimited,
    /// Overload, timeout, database or audit journal unavailable.
    Unavailable,
    /// Panic or unexpected error.
    Internal,
    /// Nothing is free from there in the space asked (REQ-008-20).
    Exhausted,
    /// A query of compliance beyond its bounds (spec 010, design « API »).
    PolicyQueryTooLarge,
    /// Beyond the drafts of an identity or the operations of a draft (spec 024,
    /// REQ-024-7).
    DraftLimit,
    /// `?simulate=true` on a write that submits no changeset (spec 024, REQ-024-17).
    SimulateUnsupported,
}

impl ProblemKind {
    /// Every class, for the contract and the tests.
    pub const ALL: [Self; 29] = [
        Self::BadRequest,
        Self::InvalidCursor,
        Self::Unauthenticated,
        Self::Forbidden,
        Self::CrossSiteRequest,
        Self::NotFound,
        Self::MethodNotAllowed,
        Self::NotAcceptable,
        Self::Conflict,
        Self::UndoConflict,
        Self::IdempotencyInProgress,
        Self::PreconditionFailed,
        Self::PayloadTooLarge,
        Self::UnsupportedMediaType,
        Self::ChangesetRefused,
        Self::MotiveRequired,
        Self::TooManyOperations,
        Self::IdempotencyKeyReused,
        Self::InvalidMember,
        Self::InvalidOverride,
        Self::InvalidImport,
        Self::PreconditionRequired,
        Self::RateLimited,
        Self::Unavailable,
        Self::Internal,
        Self::Exhausted,
        Self::PolicyQueryTooLarge,
        Self::DraftLimit,
        Self::SimulateUnsupported,
    ];

    /// The last segment of the `type`.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::BadRequest => "bad-request",
            Self::InvalidCursor => "invalid-cursor",
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden => "forbidden",
            Self::CrossSiteRequest => "cross-site-request",
            Self::NotFound => "not-found",
            Self::MethodNotAllowed => "method-not-allowed",
            Self::NotAcceptable => "not-acceptable",
            Self::Conflict => "conflict",
            Self::UndoConflict => "undo-conflict",
            Self::IdempotencyInProgress => "idempotency-in-progress",
            Self::PreconditionFailed => "precondition-failed",
            Self::PayloadTooLarge => "payload-too-large",
            Self::UnsupportedMediaType => "unsupported-media-type",
            Self::ChangesetRefused => "changeset-refused",
            Self::MotiveRequired => "motive-required",
            Self::TooManyOperations => "too-many-operations",
            Self::IdempotencyKeyReused => "idempotency-key-reused",
            Self::InvalidMember => "invalid-member",
            Self::InvalidOverride => "invalid-override",
            Self::InvalidImport => "invalid-import",
            Self::PreconditionRequired => "precondition-required",
            Self::RateLimited => "rate-limited",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
            Self::Exhausted => "exhausted",
            Self::PolicyQueryTooLarge => "policy-query-too-large",
            Self::DraftLimit => "draft-limit",
            Self::SimulateUnsupported => "simulate-unsupported",
        }
    }

    /// The HTTP status.
    #[must_use]
    pub const fn status(self) -> StatusCode {
        match self {
            Self::BadRequest | Self::InvalidCursor | Self::SimulateUnsupported => {
                StatusCode::BAD_REQUEST
            }
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Forbidden | Self::CrossSiteRequest => StatusCode::FORBIDDEN,
            Self::NotFound | Self::Exhausted => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::NotAcceptable => StatusCode::NOT_ACCEPTABLE,
            Self::Conflict | Self::UndoConflict | Self::IdempotencyInProgress => {
                StatusCode::CONFLICT
            }
            Self::PreconditionFailed => StatusCode::PRECONDITION_FAILED,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::ChangesetRefused
            | Self::MotiveRequired
            | Self::TooManyOperations
            | Self::IdempotencyKeyReused
            | Self::InvalidMember
            | Self::InvalidOverride
            | Self::InvalidImport
            | Self::PolicyQueryTooLarge
            | Self::DraftLimit => StatusCode::UNPROCESSABLE_ENTITY,
            Self::PreconditionRequired => StatusCode::PRECONDITION_REQUIRED,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Its French and English labels (spec 018, R19; REQ-021-20): what happened, in plain
    /// words, as a screen says it — not the `title`, which stays the neutral summary of the
    /// class. A client shows the label of its language from the dictionary, space `problem`.
    #[must_use]
    pub const fn labels(self) -> kartenn_core::labels::Compiled {
        kartenn_core::labels::Compiled::new(self.label_fr(), self.label_en())
    }

    /// The French label, a sentence and its stop.
    const fn label_fr(self) -> &'static str {
        match self {
            Self::BadRequest => "La requête est mal formée.",
            Self::InvalidCursor => "Cette page a expiré ; la liste repart du début.",
            Self::Unauthenticated => "Vous n'êtes pas, ou plus, connecté.",
            Self::Forbidden => "Vous n'avez pas le droit de faire cela.",
            Self::CrossSiteRequest => "Requête refusée : elle ne vient pas de kartenn.",
            Self::NotFound => "Cet objet n'existe pas à ces dates, ou plus.",
            Self::MethodNotAllowed => "Cette action n'est pas possible ici.",
            Self::NotAcceptable => "Le serveur ne sait pas répondre dans ce format.",
            Self::Conflict => "L'état actuel ne permet pas cette action.",
            Self::UndoConflict => {
                "Ce changeset ne peut plus être annulé : un changement plus récent le recouvre."
            }
            Self::IdempotencyInProgress => "La même demande est déjà en cours.",
            Self::PreconditionFailed => "L'objet a changé depuis votre lecture.",
            Self::PayloadTooLarge => "La demande est trop volumineuse.",
            Self::UnsupportedMediaType => "Le format de la demande n'est pas pris en charge.",
            Self::ChangesetRefused => "Le changement est refusé par les règles de cohérence.",
            Self::MotiveRequired => "Un motif est obligatoire.",
            Self::TooManyOperations => "Trop d'opérations dans un seul changeset.",
            Self::IdempotencyKeyReused => {
                "Cette clé d'idempotence a déjà servi pour une autre demande."
            }
            Self::InvalidMember => "Un champ ne peut pas être écrit.",
            Self::InvalidOverride => "Le filtre ou sa durée est invalide.",
            Self::InvalidImport => "L'import ne peut pas être ordonné.",
            Self::PreconditionRequired => {
                "La version lue de l'objet doit accompagner la modification."
            }
            Self::RateLimited => "Trop de requêtes : patientez un instant.",
            Self::Unavailable => "Le service est momentanément indisponible.",
            Self::Internal => "Erreur interne du serveur.",
            Self::Exhausted => "Plus aucune valeur libre dans cet ensemble.",
            Self::PolicyQueryTooLarge => "La requête de conformité est trop étendue.",
            Self::DraftLimit => "Vous avez atteint le nombre maximal de brouillons.",
            Self::SimulateUnsupported => "Cette écriture ne se simule pas.",
        }
    }

    /// The English label, a sentence and its stop (not the `title`).
    const fn label_en(self) -> &'static str {
        match self {
            Self::BadRequest => "The request is malformed.",
            Self::InvalidCursor => "This page has expired; the list starts again from the top.",
            Self::Unauthenticated => "You are not, or no longer, signed in.",
            Self::Forbidden => "You are not allowed to do this.",
            Self::CrossSiteRequest => "Request refused: it did not come from kartenn.",
            Self::NotFound => "This object does not exist at these dates, or no longer.",
            Self::MethodNotAllowed => "This action is not possible here.",
            Self::NotAcceptable => "The server cannot answer in this format.",
            Self::Conflict => "The current state does not allow this action.",
            Self::UndoConflict => {
                "This changeset can no longer be undone: a later change covers it."
            }
            Self::IdempotencyInProgress => "The same request is already under way.",
            Self::PreconditionFailed => "The object changed since you read it.",
            Self::PayloadTooLarge => "The request is too large.",
            Self::UnsupportedMediaType => "The format of the request is not supported.",
            Self::ChangesetRefused => "The change is refused by the consistency rules.",
            Self::MotiveRequired => "A motive is required.",
            Self::TooManyOperations => "Too many operations in one changeset.",
            Self::IdempotencyKeyReused => {
                "This idempotency key was already used for another request."
            }
            Self::InvalidMember => "A field cannot be written.",
            Self::InvalidOverride => "The filter or its duration is invalid.",
            Self::InvalidImport => "The import cannot be ordered.",
            Self::PreconditionRequired => {
                "The version read of the object must come with the change."
            }
            Self::RateLimited => "Too many requests: wait a moment.",
            Self::Unavailable => "The service is unavailable for the moment.",
            Self::Internal => "Internal server error.",
            Self::Exhausted => "No free value is left in this set.",
            Self::PolicyQueryTooLarge => "The compliance query is too wide.",
            Self::DraftLimit => "You have reached the most drafts allowed.",
            Self::SimulateUnsupported => "This write cannot be simulated.",
        }
    }

    /// A short summary, the same for every occurrence (RFC 9457 §3.1.3).
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::BadRequest => "The request is malformed",
            Self::InvalidCursor => "The cursor is invalid",
            Self::Unauthenticated => "Authentication is required",
            Self::Forbidden => "The action is not permitted",
            Self::CrossSiteRequest => "Cross-site request refused",
            Self::NotFound => "Not found",
            Self::MethodNotAllowed => "Method not allowed",
            Self::NotAcceptable => "No acceptable representation",
            Self::Conflict => "The current state does not allow the action",
            Self::UndoConflict => "The undo conflicts with later changes",
            Self::IdempotencyInProgress => "A request with this idempotency key is in progress",
            Self::PreconditionFailed => "The resource has changed",
            Self::PayloadTooLarge => "The body is too large",
            Self::UnsupportedMediaType => "Unsupported media type",
            Self::ChangesetRefused => "The changeset is refused",
            Self::MotiveRequired => "A motive is required",
            Self::TooManyOperations => "The changeset has too many operations",
            Self::IdempotencyKeyReused => "The idempotency key was used for another request",
            Self::InvalidMember => "A member cannot be written",
            Self::InvalidOverride => "The filter or its duration is invalid",
            Self::InvalidImport => "The elements of the import cannot be sorted",
            Self::PreconditionRequired => "If-Match is required",
            Self::RateLimited => "Too many requests",
            Self::Unavailable => "The service is unavailable",
            Self::Internal => "Internal error",
            Self::Exhausted => "Nothing is free",
            Self::PolicyQueryTooLarge => "The query of compliance is too large",
            Self::DraftLimit => "A limit of the drafts is reached",
            Self::SimulateUnsupported => "This write cannot be simulated",
        }
    }
}

/// A problem (RFC 9457 §3.1). `instance` is the request identifier, set by the request
/// layer on the way out.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Problem {
    #[serde(skip)]
    kind: ProblemKind,
    /// The class of the problem, an URN `urn:kartenn:problem:<slug>`.
    #[serde(rename = "type")]
    type_: String,
    /// The title of the class.
    title: &'static str,
    /// The HTTP status.
    status: u16,
    /// This occurrence, explained.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// The identifier of the request, `urn:uuid:…`.
    #[serde(skip_serializing_if = "Option::is_none")]
    instance: Option<String>,
    /// Extension members, such as `violations` (RFC 9457 §3.2).
    #[serde(flatten)]
    #[schema(additional_properties)]
    extensions: serde_json::Map<String, serde_json::Value>,
}

impl Problem {
    /// A problem of class `kind`, without detail.
    #[must_use]
    pub fn new(kind: ProblemKind) -> Self {
        Self {
            kind,
            type_: format!("{TYPE_PREFIX}{}", kind.slug()),
            title: kind.title(),
            status: kind.status().as_u16(),
            detail: None,
            instance: None,
            extensions: serde_json::Map::new(),
        }
    }

    /// With an explanation of this occurrence: identifiers and rule messages only, never
    /// an internal trace, a query or a sensitive value.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// With an extension member (RFC 9457 §3.2), such as `violations`.
    #[must_use]
    pub fn extension(mut self, name: &str, value: serde_json::Value) -> Self {
        self.extensions.insert(name.to_owned(), value);
        self
    }

    /// With the identifier of the request.
    #[must_use]
    pub fn instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Its class.
    #[must_use]
    pub const fn kind(&self) -> ProblemKind {
        self.kind
    }

    /// The body, as JSON.
    #[must_use]
    pub fn body(&self) -> Vec<u8> {
        // A map of strings, numbers and JSON values always serialises.
        serde_json::to_vec(self).unwrap_or_default()
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let mut response = (
            self.kind.status(),
            [(header::CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON))],
            self.body(),
        )
            .into_response();
        // Kept for the request layer, which adds `instance` and writes the body again.
        response.extensions_mut().insert(self);
        response
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_class_has_its_own_type_and_an_error_status() {
        let slugs: BTreeSet<_> = ProblemKind::ALL.iter().map(|k| k.slug()).collect();
        assert_eq!(slugs.len(), ProblemKind::ALL.len());
        for kind in ProblemKind::ALL {
            assert!(
                kind.status().is_client_error() || kind.status().is_server_error(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn a_problem_has_the_members_of_rfc_9457() {
        let problem = Problem::new(ProblemKind::NotFound)
            .detail("prefix 0190 absent")
            .instance("urn:uuid:x")
            .extension("violations", serde_json::json!([]));
        let json: serde_json::Value = serde_json::from_slice(&problem.body()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "urn:kartenn:problem:not-found",
                "title": "Not found",
                "status": 404,
                "detail": "prefix 0190 absent",
                "instance": "urn:uuid:x",
                "violations": [],
            })
        );
        let bare: serde_json::Value =
            serde_json::from_slice(&Problem::new(ProblemKind::Internal).body()).unwrap();
        assert!(bare.get("detail").is_none() && bare.get("instance").is_none());
    }
}
