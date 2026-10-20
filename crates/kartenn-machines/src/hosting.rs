//! Hosting of a machine (009/M5, REQ-009-5): a `virtual` machine runs on a `physical` one
//! or a cluster, a `container` on a `physical` or `virtual` one or a cluster; `physical`
//! and `appliance` have no host, and only a `physical` machine joins a cluster. Hosting
//! never loops.

use std::fmt;

use kartenn_core::rules::Violation;
use uuid::Uuid;

use crate::rules::HOST_INVALID;

/// Kind of a machine (009/M2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineKind {
    /// Bare-metal machine.
    Physical,
    /// Virtual machine.
    Virtual,
    /// Container.
    Container,
    /// Network or security appliance.
    Appliance,
}

impl MachineKind {
    /// Every kind.
    pub const ALL: [Self; 4] = [
        Self::Physical,
        Self::Virtual,
        Self::Container,
        Self::Appliance,
    ];

    /// The stored name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Physical => "physical",
            Self::Virtual => "virtual",
            Self::Container => "container",
            Self::Appliance => "appliance",
        }
    }

    /// Reads a stored name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }

    /// Whether a machine of this kind may run on `host`.
    #[must_use]
    pub fn may_run_on(self, host: Host) -> bool {
        matches!(
            (self, host),
            (Self::Virtual, Host::Machine(Self::Physical) | Host::Cluster)
                | (
                    Self::Container,
                    Host::Machine(Self::Physical | Self::Virtual) | Host::Cluster
                )
        )
    }

    /// Whether a machine of this kind may be a member of a cluster.
    #[must_use]
    pub fn may_join_cluster(self) -> bool {
        self == Self::Physical
    }
}

impl fmt::Display for MachineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What hosts a machine: another machine, of some kind, or a cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Host {
    /// A machine of this kind.
    Machine(MachineKind),
    /// A cluster of virtualisation.
    Cluster,
}

/// Checks the hosting of a machine of kind `guest` by `host`.
///
/// # Errors
/// `machines.host.invalid` when the pair is not allowed.
pub fn check_host(guest: MachineKind, host: Host) -> Result<(), Violation> {
    if guest.may_run_on(host) {
        return Ok(());
    }
    let host = match host {
        Host::Machine(kind) => format!("une machine {kind}"),
        Host::Cluster => "un cluster".to_owned(),
    };
    Err(Violation::new(
        &HOST_INVALID,
        format!("une machine {guest} ne peut pas être hébergée par {host}"),
    ))
}

/// Checks the membership of a machine of kind `kind` in a cluster.
///
/// # Errors
/// `machines.host.invalid` unless the machine is `physical`.
pub fn check_membership(kind: MachineKind) -> Result<(), Violation> {
    if kind.may_join_cluster() {
        Ok(())
    } else {
        Err(Violation::new(
            &HOST_INVALID,
            format!("seule une machine physical est membre d'un cluster, pas une machine {kind}"),
        ))
    }
}

/// Longest hosting chain the walk follows: M5 allows two levels (a container on a virtual
/// machine on a physical one); anything longer is already a loop or corrupted state.
const MAX_DEPTH: usize = 8;

/// Checks that making `host` the host machine of `guest` closes no loop, following
/// `host_of` (the current host machine of a machine, if any) from `host` upwards.
///
/// # Errors
/// `machines.host.invalid` when `guest` hosts itself, directly or through its hosts.
pub fn check_no_cycle(
    guest: Uuid,
    host: Uuid,
    mut host_of: impl FnMut(Uuid) -> Option<Uuid>,
) -> Result<(), Violation> {
    let cycle = || {
        Violation::new(
            &HOST_INVALID,
            format!("la machine {guest} s'hébergerait elle-même"),
        )
        .with_subjects([guest, host])
    };
    let mut current = host;
    for _ in 0..MAX_DEPTH {
        if current == guest {
            return Err(cycle());
        }
        match host_of(current) {
            Some(next) => current = next,
            None => return Ok(()),
        }
    }
    Err(cycle())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use MachineKind::{Appliance, Container, Physical, Virtual};

    #[test]
    fn hosting_truth_table() {
        let hosts = [
            Host::Machine(Physical),
            Host::Machine(Virtual),
            Host::Machine(Container),
            Host::Machine(Appliance),
            Host::Cluster,
        ];
        let expected: [(MachineKind, [bool; 5]); 4] = [
            (Physical, [false, false, false, false, false]),
            (Virtual, [true, false, false, false, true]),
            (Container, [true, true, false, false, true]),
            (Appliance, [false, false, false, false, false]),
        ];
        for (guest, row) in expected {
            for (host, allowed) in hosts.into_iter().zip(row) {
                assert_eq!(
                    check_host(guest, host).is_ok(),
                    allowed,
                    "{guest} on {host:?}"
                );
                if !allowed {
                    assert_eq!(
                        check_host(guest, host).unwrap_err().rule.id,
                        "machines.host.invalid"
                    );
                }
            }
        }
        assert!(check_membership(Physical).is_ok());
        for kind in [Virtual, Container, Appliance] {
            assert!(check_membership(kind).is_err(), "{kind}");
        }
        for kind in MachineKind::ALL {
            assert_eq!(MachineKind::from_name(kind.as_str()), Some(kind));
        }
        assert_eq!(MachineKind::from_name("vm"), None);
    }

    #[test]
    fn hosting_cycle() {
        let [a, b, c, d] = [
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        ];
        // b runs on c; c runs on nothing.
        let hosts = HashMap::from([(b, c)]);
        let lookup = |id: Uuid| hosts.get(&id).copied();
        assert!(check_no_cycle(a, b, lookup).is_ok());
        assert!(check_no_cycle(a, a, lookup).is_err(), "self-hosting");
        // a on b while b runs on a: loop of two.
        let hosts = HashMap::from([(b, a)]);
        let err = check_no_cycle(a, b, |id| hosts.get(&id).copied()).unwrap_err();
        assert_eq!(err.rule.id, "machines.host.invalid");
        assert_eq!(err.subjects, vec![a, b]);
        // a loop not through the guest, met on the way up: refused, never followed forever.
        let hosts = HashMap::from([(b, c), (c, d), (d, c)]);
        assert!(check_no_cycle(a, b, |id| hosts.get(&id).copied()).is_err());
    }
}
