//! 802.1Q mode of an interface (009/M9, REQ-009-12): `none` carries no VLAN, `access`
//! exactly one untagged VLAN, `trunk` a non-empty set of tagged VLANs and an optional
//! untagged native VLAN outside that set. Whether the VLANs live over the period is
//! checked by the module handler, which reads them.

use std::collections::BTreeSet;

use kartenn_core::rules::Violation;
use uuid::Uuid;

use crate::rules::VLAN_MODE;

/// A consistent 802.1Q mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VlanMode {
    /// No VLAN.
    #[default]
    None,
    /// One untagged VLAN.
    Access(Uuid),
    /// Tagged VLANs and an optional native one.
    Trunk {
        /// Tagged VLANs, never empty.
        tagged: BTreeSet<Uuid>,
        /// Untagged VLAN, outside `tagged`.
        native: Option<Uuid>,
    },
}

impl VlanMode {
    /// Builds a mode from its stored or submitted columns: `mode` is `none`, `access` or
    /// `trunk`.
    ///
    /// # Errors
    /// `machines.vlan.mode` when the columns do not fit the mode.
    pub fn from_parts(
        mode: &str,
        access: Option<Uuid>,
        native: Option<Uuid>,
        tagged: &[Uuid],
    ) -> Result<Self, Violation> {
        let invalid = |why: &str| Violation::new(&VLAN_MODE, format!("mode {mode} : {why}"));
        match mode {
            "none" => {
                if access.is_some() || native.is_some() || !tagged.is_empty() {
                    return Err(invalid(
                        "une interface sans mode 802.1Q ne porte aucun VLAN",
                    ));
                }
                Ok(Self::None)
            }
            "access" => {
                if native.is_some() || !tagged.is_empty() {
                    return Err(invalid("un port access ne porte qu'un VLAN non étiqueté"));
                }
                access
                    .map(Self::Access)
                    .ok_or_else(|| invalid("un port access porte exactement un VLAN"))
            }
            "trunk" => {
                if access.is_some() {
                    return Err(invalid(
                        "un port trunk n'a pas de VLAN access, mais un VLAN natif",
                    ));
                }
                let set: BTreeSet<Uuid> = tagged.iter().copied().collect();
                if set.len() != tagged.len() {
                    return Err(invalid("un VLAN étiqueté figure deux fois"));
                }
                if set.is_empty() {
                    return Err(invalid("un port trunk porte au moins un VLAN étiqueté"));
                }
                if native.is_some_and(|native| set.contains(&native)) {
                    return Err(invalid("le VLAN natif n'est pas aussi étiqueté"));
                }
                Ok(Self::Trunk {
                    tagged: set,
                    native,
                })
            }
            _ => Err(invalid("mode attendu : none, access ou trunk")),
        }
    }

    /// The stored name of the mode.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Access(_) => "access",
            Self::Trunk { .. } => "trunk",
        }
    }

    /// Every VLAN the interface carries, tagged or not.
    #[must_use]
    pub fn vlans(&self) -> BTreeSet<Uuid> {
        match self {
            Self::None => BTreeSet::new(),
            Self::Access(vlan) => BTreeSet::from([*vlan]),
            Self::Trunk { tagged, native } => tagged.iter().chain(native).copied().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(mode: &str, access: Option<Uuid>, native: Option<Uuid>, tagged: &[Uuid]) -> bool {
        VlanMode::from_parts(mode, access, native, tagged)
            .is_err_and(|v| v.rule.id == "machines.vlan.mode")
    }

    #[test]
    fn vlan_modes() {
        let [a, b, c] = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
        assert_eq!(
            VlanMode::from_parts("none", None, None, &[]).unwrap(),
            VlanMode::None
        );
        assert_eq!(
            VlanMode::from_parts("access", Some(a), None, &[]).unwrap(),
            VlanMode::Access(a)
        );
        let trunk = VlanMode::from_parts("trunk", None, Some(c), &[a, b]).unwrap();
        assert_eq!(trunk.name(), "trunk");
        assert_eq!(trunk.vlans(), BTreeSet::from([a, b, c]));
        assert!(
            VlanMode::from_parts("trunk", None, None, &[a]).is_ok(),
            "native is optional"
        );

        assert!(refused("none", Some(a), None, &[]));
        assert!(refused("none", None, None, &[a]));
        assert!(refused("access", None, None, &[]), "access without VLAN");
        assert!(refused("access", Some(a), None, &[b]));
        assert!(refused("access", Some(a), Some(b), &[]));
        assert!(refused("trunk", None, None, &[]), "empty trunk");
        assert!(
            refused("trunk", None, Some(a), &[a, b]),
            "native also tagged"
        );
        assert!(refused("trunk", Some(a), None, &[b]));
        assert!(refused("trunk", None, None, &[a, a]));
        assert!(refused("hybrid", None, None, &[]));
    }
}
