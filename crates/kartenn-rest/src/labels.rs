//! The labels of the dictionary (spec 018, R19 to R24; ADR 0040). This part: the labels
//! compiled with the codes of the build (core, modules, problems, former permission
//! names) and their fingerprint, computed once at start; it moves only with the version
//! of the server or its modules.

use kartenn_core::labels::CodeLabel;
use kartenn_service::module::ModuleRegistry;
use sha2::{Digest, Sha256};

use crate::problem::ProblemKind;

/// The labels compiled with the build, sorted by space and key, and their fingerprint.
#[derive(Debug, Clone)]
pub struct CodeCatalogue {
    labels: Vec<CodeLabel>,
    digest: String,
}

impl CodeCatalogue {
    /// The labels of the core, of the modules of `registry`, of the problems, and of the
    /// former names of the renamed permissions (R6: the history keeps them).
    #[must_use]
    pub fn of(registry: &ModuleRegistry) -> Self {
        let mut labels = kartenn_core::code_labels::core();
        labels.extend(registry.labels());
        labels.extend(ProblemKind::ALL.iter().map(|kind| {
            let compiled = kind.labels();
            CodeLabel::new("problem", kind.slug(), compiled.fr, compiled.en)
        }));
        let former: Vec<CodeLabel> = kartenn_core::authz::RENAMED_PERMISSIONS
            .iter()
            .filter_map(|(old, new)| {
                let current = labels
                    .iter()
                    .find(|l| l.space == "permission" && l.key == *new)?;
                Some(CodeLabel {
                    space: "permission",
                    key: (*old).to_owned(),
                    fr: format!("{} (ancien nom de {new})", current.fr),
                    en: format!("{} (former name of {new})", current.en),
                })
            })
            .collect();
        labels.extend(former);
        labels.sort();
        labels.dedup_by(|a, b| a.space == b.space && a.key == b.key);
        let mut hasher = Sha256::new();
        for l in &labels {
            for part in [l.space, &l.key, &l.fr, &l.en] {
                hasher.update(part.as_bytes());
                hasher.update([0]);
            }
        }
        let digest = crate::dated::hex(&hasher.finalize()[..8]);
        Self { labels, digest }
    }

    /// The labels, sorted by space and key.
    #[must_use]
    pub fn labels(&self) -> &[CodeLabel] {
        &self.labels
    }

    /// The fingerprint of the labels (16 hexadecimal characters), part of the revision of
    /// the dictionary.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_problems_and_former_names_are_labelled() {
        let catalogue = CodeCatalogue::of(ModuleRegistry::empty());
        let find = |space: &str, key: &str| {
            catalogue
                .labels()
                .iter()
                .find(|l| l.space == space && l.key == key)
                .cloned()
        };
        assert_eq!(
            find("problem", "changeset-refused").map(|l| l.en),
            Some("The changeset is refused".to_owned())
        );
        assert!(
            ProblemKind::ALL
                .iter()
                .all(|k| find("problem", k.slug()).is_some())
        );
        assert_eq!(catalogue.digest().len(), 16);
        assert_eq!(
            catalogue.digest(),
            CodeCatalogue::of(ModuleRegistry::empty()).digest(),
            "the fingerprint is stable"
        );
    }
}
