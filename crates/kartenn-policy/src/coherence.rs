//! The coherence of the machines (REQ-010-17, S10, D22; design « Cohérence des machines »):
//! a machine whose addresses resolve another environment than the one it declares, or,
//! without a declared environment, whose addresses resolve different environments, carries
//! the anomaly `policy.machine.environment_mismatch`. The addresses of an interface of an
//! exempt role are left out. Evaluated on the machines the changeset touches only (written,
//! their interfaces or attachments written, their declared environment, the role of an
//! interface or the resolved environment of one of their addresses changed, an exempt role
//! written), so that the anomaly closes once corrected there and an unrelated changeset
//! raises nothing.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::rules::Violation;
use kartenn_machines::module::{INTERFACE, INTERFACE_ROLE, MACHINE};
use kartenn_service::ObjectKind;
use kartenn_service::module::{CheckContext, Findings};
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::module::{DECLARED_ENVIRONMENT, ENVIRONMENT};
use crate::pg::store;
use crate::rules::MACHINE_ENVIRONMENT_MISMATCH;

/// The exempt roles while none was ever administered (REQ-010-17): administration,
/// backup, monitoring.
pub(crate) const DEFAULT_EXEMPT_ROLES: &[&str] = &["administration", "backup", "monitoring"];

/// The warnings of REQ-010-17 on the machines the changeset touches, each machine evaluated
/// anew so that its anomaly closes once corrected.
pub(crate) async fn check(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
) -> Result<Findings, StoreError> {
    let machines = touched_machines(conn, context).await?;
    if machines.is_empty() {
        return Ok(Findings::default());
    }
    let exempt = exempt_roles(conn).await?;
    let declared: BTreeMap<Uuid, BTreeSet<String>> =
        resolved_keys(conn, &machines, DECLARED_ENVIRONMENT)
            .await?
            .into_iter()
            .collect();
    let held = held_addresses(conn, &machines).await?;
    let interfaces: Vec<Uuid> = held
        .iter()
        .map(|h| h.interface)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let roles: BTreeMap<Uuid, BTreeSet<String>> = resolved_keys(conn, &interfaces, INTERFACE_ROLE)
        .await?
        .into_iter()
        .collect();
    let addresses: Vec<Uuid> = held
        .iter()
        .map(|h| h.address)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let environments: BTreeMap<Uuid, BTreeSet<String>> =
        resolved_keys(conn, &addresses, ENVIRONMENT)
            .await?
            .into_iter()
            .collect();
    let mut findings = Findings::default();
    for machine in &machines {
        findings
            .reevaluated
            .push((*machine, MACHINE_ENVIRONMENT_MISMATCH.id));
        let considered: Vec<(Uuid, &BTreeSet<String>)> = held
            .iter()
            .filter(|h| h.machine == *machine)
            .filter(|h| {
                roles
                    .get(&h.interface)
                    .is_none_or(|r| r.iter().all(|role| !exempt.contains(role)))
            })
            .filter_map(|h| {
                environments
                    .get(&h.address)
                    .filter(|e| !e.is_empty())
                    .map(|e| (h.address, e))
            })
            .collect();
        let declared = declared.get(machine).filter(|d| !d.is_empty());
        if let Some(message) = mismatch(declared, &considered) {
            findings.violations.push(
                Violation::new(&MACHINE_ENVIRONMENT_MISMATCH, message).with_subjects([*machine]),
            );
        }
    }
    Ok(findings)
}

/// The message of the divergence of a machine, if any: an address resolving another set
/// than the declared one or, without a declaration, two addresses of different sets.
fn mismatch(
    declared: Option<&BTreeSet<String>>,
    addresses: &[(Uuid, &BTreeSet<String>)],
) -> Option<String> {
    let render = |set: &BTreeSet<String>| {
        format!("{{{}}}", set.iter().cloned().collect::<Vec<_>>().join(", "))
    };
    if let Some(declared) = declared {
        let (address, set) = addresses.iter().find(|(_, set)| *set != declared)?;
        return Some(format!(
            "the address {address} resolves {} where the machine declares {}",
            render(set),
            render(declared)
        ));
    }
    let (first, first_set) = addresses.first()?;
    let (other, other_set) = addresses.iter().find(|(_, set)| *set != *first_set)?;
    Some(format!(
        "the addresses {first} and {other} resolve {} and {}",
        render(first_set),
        render(other_set)
    ))
}

/// The living machines the changeset touches (see the module documentation).
async fn touched_machines(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
) -> Result<Vec<Uuid>, StoreError> {
    let mut machines = Vec::new();
    let mut others = Vec::new();
    for (kind, id) in context.objects {
        match kind {
            ObjectKind::Module(k) if k.as_str() == MACHINE => machines.push(*id),
            ObjectKind::Module(k) if k.as_str() == INTERFACE => others.push(*id),
            ObjectKind::Address => others.push(*id),
            _ => {}
        }
    }
    others.extend(diff_objects(conn, &[ENVIRONMENT, DECLARED_ENVIRONMENT, INTERFACE_ROLE]).await?);
    let mut roles = written_exempt_roles(conn, context.changeset).await?;
    if !roles.is_empty() {
        // The first exempt role administered replaces the default list: its roles may
        // leave the exemption too.
        roles.extend(DEFAULT_EXEMPT_ROLES.iter().map(|&r| r.to_owned()));
    }
    // Nothing that could move a machine: no read at all (the changeset unit budget, D14).
    if machines.is_empty() && others.is_empty() && roles.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "WITH r AS (
             SELECT i.id AS interface, i.machine_id
               FROM machines.interface i WHERE i.valid_period @> now())
         SELECT DISTINCT m.id FROM machines.machine m
          WHERE m.valid_period @> now()
            AND (m.id = ANY($1)
                 OR m.id IN (SELECT machine_id FROM r WHERE interface = ANY($2))
                 OR m.id IN (SELECT r.machine_id FROM r
                               JOIN machines.attachment a
                                 ON a.interface_id = r.interface AND a.valid_period @> now()
                              WHERE a.address_id = ANY($2))
                 OR (cardinality($3::text[]) > 0 AND m.id IN (
                     SELECT r.machine_id FROM r
                       JOIN public.property_assignment pa
                         ON pa.object_id = r.interface AND pa.kind = 'value'
                        AND pa.valid_period @> now()
                       JOIN public.property_declaration d
                         ON d.id = pa.property_id AND d.key = $4 AND d.valid_period @> now()
                       JOIN public.property_enum_value e
                         ON e.id = pa.v_uuid OR e.id = ANY(pa.v_uuids)
                      WHERE e.key = ANY($3))))
          ORDER BY 1",
    )
    .bind(&machines)
    .bind(&others)
    .bind(&roles)
    .bind(INTERFACE_ROLE)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The objects whose resolved value of one of `keys` the changeset changes, read from the
/// detail of the differential of the properties (ADR 0026, clause 13).
async fn diff_objects(conn: &mut PgConnection, keys: &[&str]) -> Result<Vec<Uuid>, StoreError> {
    // The detail exists once the differential was computed; reading a missing table would
    // abort the transaction.
    let recorded: bool =
        sqlx::query_scalar("SELECT to_regclass('pg_temp.diff_detail') IS NOT NULL")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
    if !recorded {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT DISTINCT object_id FROM pg_temp.diff_detail WHERE property_key = ANY($1)",
    )
    .bind(keys)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The keys of the exempt roles the changeset writes, created, changed or closed.
async fn written_exempt_roles(
    conn: &mut PgConnection,
    changeset: Uuid,
) -> Result<Vec<String>, StoreError> {
    sqlx::query_scalar(
        "SELECT role_key FROM policy.policy_exempt_role WHERE changeset_id = $1
         UNION
         SELECT role_key FROM policy.policy_exempt_role_history WHERE sys_end = now()",
    )
    .bind(changeset)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The living exempt roles, or [`DEFAULT_EXEMPT_ROLES`] while none was ever administered.
async fn exempt_roles(conn: &mut PgConnection) -> Result<BTreeSet<String>, StoreError> {
    let (living, ever): (Vec<String>, bool) = sqlx::query_as(
        "SELECT coalesce(array_agg(role_key) FILTER (WHERE valid_period @> now()), '{}'),
                EXISTS (SELECT 1 FROM policy.policy_exempt_role)
                OR EXISTS (SELECT 1 FROM policy.policy_exempt_role_history)
           FROM policy.policy_exempt_role",
    )
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(if ever {
        living.into_iter().collect()
    } else {
        DEFAULT_EXEMPT_ROLES.iter().map(|&r| r.to_owned()).collect()
    })
}

/// An address held now by an interface of a machine.
struct Held {
    machine: Uuid,
    interface: Uuid,
    address: Uuid,
}

/// The addresses held now by the living interfaces of `machines`.
async fn held_addresses(
    conn: &mut PgConnection,
    machines: &[Uuid],
) -> Result<Vec<Held>, StoreError> {
    let rows: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT i.machine_id, i.id, a.address_id
           FROM machines.interface i
           JOIN machines.attachment a ON a.interface_id = i.id AND a.valid_period @> now()
          WHERE i.machine_id = ANY($1) AND i.valid_period @> now()
          ORDER BY 1, 2, 3",
    )
    .bind(machines)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(machine, interface, address)| Held {
            machine,
            interface,
            address,
        })
        .collect())
}

/// The keys of the closed-list values that `key` resolves now on each of `objects`, read in
/// the transaction (an inherited property from its origin, ADR 0016); objects without a
/// value are left out.
async fn resolved_keys(
    conn: &mut PgConnection,
    objects: &[Uuid],
    key: &str,
) -> Result<Vec<(Uuid, BTreeSet<String>)>, StoreError> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(Uuid, Vec<String>)> = sqlx::query_as(
        "WITH d AS (
             SELECT id, inherit_tree FROM public.property_declaration
              WHERE key = $2 AND valid_period @> now() LIMIT 1)
         SELECT x.id, array_agg(DISTINCT e.key ORDER BY e.key)
           FROM unnest($1::uuid[]) x(id)
          CROSS JOIN d
          CROSS JOIN LATERAL (
              SELECT CASE WHEN d.inherit_tree IS NULL THEN x.id
                          ELSE (SELECT o.origin_object_id FROM public.property_origin o
                                 WHERE o.object_id = x.id AND o.property_id = d.id
                                   AND o.valid_period @> now() LIMIT 1) END AS origin) s
           JOIN public.property_assignment a
             ON a.object_id = s.origin AND a.property_id = d.id AND a.kind = 'value'
            AND a.valid_period @> now()
           JOIN public.property_enum_value e ON e.id = a.v_uuid OR e.id = ANY(a.v_uuids)
          GROUP BY x.id",
    )
    .bind(objects)
    .bind(key)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(id, keys)| (id, keys.into_iter().collect()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(keys: &[&str]) -> BTreeSet<String> {
        keys.iter().map(|&k| k.to_owned()).collect()
    }

    #[test]
    fn a_declared_machine_diverges_on_one_other_address() {
        let (prod, admin) = (set(&["prod"]), set(&["admin"]));
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        assert!(mismatch(Some(&prod), &[(a, &prod)]).is_none());
        let found = mismatch(Some(&prod), &[(a, &prod), (b, &admin)]).unwrap();
        assert!(found.contains(&b.to_string()), "{found}");
    }

    #[test]
    fn an_undeclared_machine_diverges_on_two_addresses_of_different_sets() {
        let (prod, dev) = (set(&["prod"]), set(&["dev"]));
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        assert!(mismatch(None, &[(a, &prod), (b, &prod)]).is_none());
        assert!(mismatch(None, &[(a, &prod)]).is_none());
        assert!(mismatch(None, &[(a, &prod), (b, &dev)]).is_some());
    }
}
