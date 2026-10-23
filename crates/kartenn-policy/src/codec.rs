//! The JSON forms of the objects of the policy (feature `pg`): what an operation receives
//! and what a row stores, decoded into the pure domain (`model`) and back. A form is
//! checked once, by the operation; the row keeps the checked, canonical form.

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::properties::PropertyKey;
use kartenn_core::rules::Violation;
use serde_json::{Map, Value, json};

use crate::model::{
    AxisKey, AxisValue, ConditionKind, DossierField, Key, Locale, RawEntry, Selector, ServiceEntry,
    SignalKind, Text, Texts,
};
use crate::rules::{
    CONDITION_PARAMS_INVALID, RULE_UNKNOWN_AXIS, SERVICE_ENTRY_INVALID, SIGNAL_PARAMS_INVALID,
    TEXT_INVALID,
};

/// The language every pedagogical text needs (REQ-010-33): the default language of the
/// instance (`[i18n] default_locale`, REQ-018-12).
pub(crate) fn default_locale() -> Locale {
    kartenn_core::labels::default_locale().clone()
}

/// Texts `{locale: {body, url?}}` (S29): checked, normalised (NFC), and their row form.
pub(crate) fn texts(value: &Value) -> Result<(Texts, Value), Violation> {
    let object = value
        .as_object()
        .ok_or_else(|| Violation::new(&TEXT_INVALID, "texts are an object of languages"))?;
    let mut texts = BTreeMap::new();
    let mut row = Map::new();
    for (tag, text) in object {
        let locale = crate::model::locale(tag)?;
        let body = text.get("body").and_then(Value::as_str).ok_or_else(|| {
            Violation::new(&TEXT_INVALID, format!("the text in {tag} has no body"))
        })?;
        let url = text.get("url").and_then(Value::as_str);
        let text = Text::new(body, url)?;
        row.insert(
            locale.as_str().to_owned(),
            json!({ "body": text.body, "url": text.url }),
        );
        texts.insert(locale, text);
    }
    Ok((Texts(texts), Value::Object(row)))
}

/// The texts of a row, as stored (checked when written).
pub(crate) fn stored_texts(value: &Value) -> Texts {
    texts(value).map(|(t, _)| t).unwrap_or_default()
}

/// A port or a range of ports: `22`, `"22"`, `"8000-8080"` or `[8000, 8080]`.
fn ports(value: &Value) -> Option<(i64, i64)> {
    match value {
        Value::Number(n) => n.as_i64().map(|p| (p, p)),
        Value::String(s) => match s.split_once('-') {
            Some((a, b)) => Some((a.trim().parse().ok()?, b.trim().parse().ok()?)),
            None => s.trim().parse().ok().map(|p| (p, p)),
        },
        Value::Array(a) if a.len() == 2 => Some((a[0].as_i64()?, a[1].as_i64()?)),
        _ => None,
    }
}

/// The row form of a checked entry.
fn entry_row(entry: ServiceEntry) -> Value {
    match entry {
        ServiceEntry::Ports {
            transport,
            first,
            last,
        } => json!({ "transport": transport.to_string(), "ports": [first, last] }),
        ServiceEntry::Icmp {
            v6,
            icmp_type,
            code,
        } => json!({
            "transport": if v6 { "icmpv6" } else { "icmp" },
            "icmp_type": icmp_type,
            "icmp_code": code,
        }),
        ServiceEntry::Protocol(protocol) => json!({ "transport": "ip", "protocol": protocol }),
    }
}

/// The entries of a service (REQ-010-5): checked, and their row form.
pub(crate) fn entries(value: &Value) -> Result<(Vec<ServiceEntry>, Value), Violation> {
    let list = value.as_array().ok_or_else(|| {
        Violation::new(
            &SERVICE_ENTRY_INVALID,
            "the entries of a service are a list",
        )
    })?;
    let mut checked = Vec::new();
    for raw in list {
        let int = |key: &str| raw.get(key).and_then(Value::as_i64);
        let ports = match raw.get("ports") {
            None | Some(Value::Null) => None,
            Some(p) => Some(ports(p).ok_or_else(|| {
                Violation::new(&SERVICE_ENTRY_INVALID, format!("ports {p} unreadable"))
            })?),
        };
        checked.push(ServiceEntry::new(&RawEntry {
            transport: raw
                .get("transport")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            ports,
            icmp_type: int("icmp_type"),
            icmp_code: int("icmp_code"),
            protocol: int("protocol"),
        })?);
    }
    let row = checked.iter().copied().map(entry_row).collect();
    Ok((checked, Value::Array(row)))
}

/// An axis key, checked.
fn axis(value: &str) -> Result<AxisKey, Violation> {
    PropertyKey::parse(value)
        .map_err(|_| Violation::new(&RULE_UNKNOWN_AXIS, format!("{value}: not a property key")))
}

/// A selector `{axis: [values]}` (REQ-010-8): its terms, and its row form.
pub(crate) fn selector(value: &Value) -> Result<(Selector, Value), Violation> {
    let object = value
        .as_object()
        .ok_or_else(|| Violation::new(&RULE_UNKNOWN_AXIS, "a selector is an object of axes"))?;
    let mut selector = Selector::new();
    for (key, values) in object {
        let admitted: BTreeSet<AxisValue> = values
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(AxisValue::from)
                    .collect()
            })
            .unwrap_or_default();
        selector.insert(axis(key)?, admitted);
    }
    let row = selector
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                Value::Array(v.iter().map(|x| x.as_str().into()).collect()),
            )
        })
        .collect::<Map<_, _>>();
    Ok((selector, Value::Object(row)))
}

/// The axes of « same value », checked.
pub(crate) fn axes(value: &Value) -> Result<BTreeSet<AxisKey>, Violation> {
    value
        .as_array()
        .ok_or_else(|| Violation::new(&RULE_UNKNOWN_AXIS, "same is a list of axes"))?
        .iter()
        .map(|a| axis(a.as_str().unwrap_or_default()))
        .collect()
}

fn keys(value: Option<&Value>) -> Option<BTreeSet<Key>> {
    value?
        .as_array()?
        .iter()
        .map(|k| Key::new(k.as_str()?).ok())
        .collect()
}

fn condition_invalid(why: impl Into<String>) -> Violation {
    Violation::new(&CONDITION_PARAMS_INVALID, why.into())
}

fn signal_invalid(why: impl Into<String>) -> Violation {
    Violation::new(&SIGNAL_PARAMS_INVALID, why.into())
}

/// The kind of an admissibility condition from its name and parameters (REQ-010-36).
pub(crate) fn condition(kind: &str, params: &Value) -> Result<ConditionKind, Violation> {
    let text = |key: &str| params.get(key).and_then(Value::as_str);
    Ok(match kind {
        "dossier_field" => ConditionKind::DossierField(match text("field") {
            Some("external_refs") => DossierField::ExternalRefs,
            Some("requester") => DossierField::Requester,
            Some("owners") => DossierField::Owners,
            Some("data_nature") => DossierField::DataNature,
            Some("period") => DossierField::Period,
            other => return Err(condition_invalid(format!("unknown field {other:?}"))),
        }),
        "max_width" => ConditionKind::MaxWidth(
            params
                .get("addresses")
                .and_then(Value::as_u64)
                .map(u128::from)
                .ok_or_else(|| condition_invalid("max_width needs addresses"))?,
        ),
        "not_whole_domain" => ConditionKind::NotWholeDomain,
        "named_service" => ConditionKind::NamedService,
        "dossier_rank_at_most" => ConditionKind::DossierRankAtMost(
            axis(text("axis").unwrap_or_default()).map_err(|_| condition_invalid("an axis"))?,
        ),
        other => return Err(condition_invalid(format!("unknown kind {other}"))),
    })
}

/// The kind of a signal from its name and parameters (REQ-010-37).
pub(crate) fn signal(kind: &str, params: &Value) -> Result<SignalKind, Violation> {
    let axis_of = || {
        axis(
            params
                .get("axis")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .map_err(|_| signal_invalid("an axis"))
    };
    Ok(match kind {
        "wide_endpoint" => SignalKind::WideEndpoint(
            params
                .get("addresses")
                .and_then(Value::as_u64)
                .map(u128::from)
                .ok_or_else(|| signal_invalid("wide_endpoint needs addresses"))?,
        ),
        "flow_type" => SignalKind::FlowType(
            keys(params.get("flow_types")).ok_or_else(|| signal_invalid("flow_types"))?,
        ),
        "service" => SignalKind::Service(
            keys(params.get("services")).ok_or_else(|| signal_invalid("services"))?,
        ),
        "axis_differs" => SignalKind::AxisDiffers(axis_of()?),
        "destination_higher_rank" => SignalKind::DestinationHigherRank(axis_of()?),
        "endpoint_value" => SignalKind::EndpointValue(
            axis_of()?,
            params
                .get("values")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(AxisValue::from)
                        .collect()
                })
                .unwrap_or_default(),
        ),
        "transverse_only" => SignalKind::TransverseOnly,
        "rank_at_least" => SignalKind::RankAtLeast(
            axis_of()?,
            params
                .get("rank")
                .and_then(Value::as_i64)
                .and_then(|r| i32::try_from(r).ok())
                .ok_or_else(|| signal_invalid("rank_at_least needs a rank"))?,
        ),
        other => return Err(signal_invalid(format!("unknown kind {other}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_checked_and_canonical() {
        let (checked, row) =
            entries(&json!([{"transport": "tcp", "ports": "8000-8080"}, {"transport": "icmp"}]))
                .unwrap();
        assert_eq!(checked.len(), 2);
        assert_eq!(
            row,
            json!([{"transport": "tcp", "ports": [8000, 8080]},
                   {"transport": "icmp", "icmp_type": null, "icmp_code": null}])
        );
        assert_eq!(
            entries(&json!([{"transport": "tcp", "ports": 0}]))
                .unwrap_err()
                .rule
                .id,
            "policy.service.entry_invalid"
        );
    }

    #[test]
    fn kinds_parse_their_parameters() {
        assert!(condition("max_width", &json!({"addresses": 256})).is_ok());
        assert_eq!(
            condition("dossier_field", &json!({"field": "colour"}))
                .unwrap_err()
                .rule
                .id,
            "policy.condition.params_invalid"
        );
        assert!(
            signal(
                "rank_at_least",
                &json!({"axis": "policy.sensitivity", "rank": 2})
            )
            .is_ok()
        );
        assert_eq!(
            signal("weather", &json!({})).unwrap_err().rule.id,
            "policy.signal.params_invalid"
        );
    }

    #[test]
    fn texts_need_a_body_and_a_language() {
        assert!(texts(&json!({"fr": {"body": "Interdit."}})).is_ok());
        for bad in [json!({"fr": {}}), json!({"F": {"body": "x"}}), json!([])] {
            assert_eq!(texts(&bad).unwrap_err().rule.id, "policy.text.invalid");
        }
    }
}
