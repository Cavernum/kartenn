//! Errors of the service as problems (design 006, « Erreurs »). The detail carries rule
//! messages and identifiers only; an infrastructure error goes to the log, never to the
//! client.

use kartenn_core::rules;
use kartenn_service::import::ImportError;
use kartenn_service::{RequestError, StoreError, WriteError};

use crate::problem::{Problem, ProblemKind};

/// A failure of the database, logged here and answered without detail.
fn unavailable(error: &StoreError) -> Problem {
    tracing::error!(name: "http.store.failed", error = %error.0, "the database failed during a request");
    Problem::new(ProblemKind::Unavailable)
}

impl From<StoreError> for Problem {
    fn from(error: StoreError) -> Self {
        unavailable(&error)
    }
}

impl From<RequestError> for Problem {
    fn from(error: RequestError) -> Self {
        match error {
            RequestError::Empty
            | RequestError::InvalidPeriod
            | RequestError::UnknownReference { .. }
            | RequestError::DuplicateLocal(_)
            | RequestError::WrongReferenceKind { .. } => {
                Self::new(ProblemKind::BadRequest).detail(error.to_string())
            }
            RequestError::InvalidOverride => {
                Self::new(ProblemKind::InvalidOverride).detail(error.to_string())
            }
            RequestError::MotiveRequired => Self::new(ProblemKind::MotiveRequired),
            RequestError::TooManyOperations { .. } => {
                Self::new(ProblemKind::TooManyOperations).detail(error.to_string())
            }
            RequestError::IdempotencyConflict => Self::new(ProblemKind::IdempotencyKeyReused),
            RequestError::IdempotencyInProgress => Self::new(ProblemKind::IdempotencyInProgress),
            // A decision refused to its author or to whom lacks the rights → 403; a
            // changeset no longer pending → 409 (design « Cycle d'un changeset »).
            RequestError::Review(ref v) if v.rule.id == rules::CHANGESET_NOT_PENDING.id => {
                Self::new(ProblemKind::Conflict).detail(error.to_string())
            }
            RequestError::Review(_) => Self::new(ProblemKind::Forbidden).detail(error.to_string()),
            RequestError::Forbidden(_) => {
                Self::new(ProblemKind::Forbidden).detail(error.to_string())
            }
            RequestError::Audit(e) => {
                tracing::error!(name: "audit.write.failed", error = %e.0, "an audit event could not be recorded");
                Self::new(ProblemKind::Unavailable)
            }
            RequestError::Store(e) => unavailable(&e),
            other => {
                tracing::error!(name: "http.error.unmapped", error = %other, "an error has no problem class");
                Self::new(ProblemKind::Internal)
            }
        }
    }
}

impl From<WriteError> for Problem {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Rule(_) => Self::new(ProblemKind::Conflict).detail(error.to_string()),
            WriteError::NoVersion(_) => Self::new(ProblemKind::NotFound),
            WriteError::Store(e) => unavailable(&e),
        }
    }
}

impl From<ImportError> for Problem {
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::Store(e) => unavailable(&e),
            other => Self::new(ProblemKind::BadRequest).detail(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use kartenn_core::rules::{self, Violation};
    use uuid::Uuid;

    use super::*;

    fn kind(problem: impl Into<Problem>) -> ProblemKind {
        problem.into().kind()
    }

    #[test]
    fn each_request_error_has_its_line_of_the_table() {
        let violation = Violation::new(&rules::CHANGESET_NOT_AUTHOR, "not the author");
        let cases = [
            (RequestError::Empty, ProblemKind::BadRequest),
            (RequestError::InvalidPeriod, ProblemKind::BadRequest),
            (
                RequestError::UnknownReference {
                    operation: 1,
                    local: 2,
                },
                ProblemKind::BadRequest,
            ),
            (RequestError::DuplicateLocal(1), ProblemKind::BadRequest),
            (RequestError::InvalidOverride, ProblemKind::InvalidOverride),
            (RequestError::MotiveRequired, ProblemKind::MotiveRequired),
            (
                RequestError::TooManyOperations { count: 2, max: 1 },
                ProblemKind::TooManyOperations,
            ),
            (
                RequestError::IdempotencyConflict,
                ProblemKind::IdempotencyKeyReused,
            ),
            (
                RequestError::IdempotencyInProgress,
                ProblemKind::IdempotencyInProgress,
            ),
            (RequestError::Review(violation), ProblemKind::Forbidden),
            (
                RequestError::Review(Violation::new(&rules::CHANGESET_NOT_PENDING, "not pending")),
                ProblemKind::Conflict,
            ),
            (
                RequestError::Forbidden("prefix.write".into()),
                ProblemKind::Forbidden,
            ),
            (
                RequestError::Audit(StoreError("down".into())),
                ProblemKind::Unavailable,
            ),
            (
                RequestError::Store(StoreError("down".into())),
                ProblemKind::Unavailable,
            ),
        ];
        for (error, expected) in cases {
            let name = format!("{error:?}");
            assert_eq!(kind(error), expected, "{name}");
        }
    }

    #[test]
    fn write_and_import_errors_have_their_line() {
        let violation = Violation::new(&rules::CHANGESET_NOT_AUTHOR, "not the author");
        assert_eq!(kind(WriteError::Rule(violation)), ProblemKind::Conflict);
        assert_eq!(
            kind(WriteError::NoVersion(Uuid::nil())),
            ProblemKind::NotFound
        );
        assert_eq!(
            kind(ImportError::MissingExternalId(0)),
            ProblemKind::BadRequest
        );
        assert_eq!(
            kind(ImportError::Store(StoreError("down".into()))),
            ProblemKind::Unavailable
        );
    }

    #[test]
    fn an_infrastructure_error_never_reaches_the_client() {
        let problem: Problem = RequestError::Store(StoreError(
            "relation \"prefix\" does not exist: SELECT secret".into(),
        ))
        .into();
        let body = String::from_utf8(problem.body()).unwrap();
        assert!(
            !body.contains("SELECT") && !body.contains("relation"),
            "{body}"
        );
    }
}
