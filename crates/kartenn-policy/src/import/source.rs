//! The inputs of the converter, in a neutral format of its own (REQ-010-35, 53):
//!
//! - the matrix: a CSV `source,flow_type,destination,state`, a state among `allow`, `deny`
//!   and `partial` (« partiel », Q11);
//! - the catalogue in two tables: a CSV `service,transport,port` (a port `22` or a range
//!   `8000-8080`, none for ICMP) and a CSV `service,allowed,description[,flow_type]`;
//! - the rules of inference: TOML files read in the order of their names, each table a
//!   rule with `labels`, `constraints` (`name`, `cidr` with `subnet_of` or `equals`,
//!   `site`) and nested `exception` tables;
//! - the sites, a CSV `name,id`, to turn a constraint on the site name into identifiers;
//! - for the simulation of the inference, the nodes: a CSV
//!   `id,parent,kind,name,prefix,site`.
//!
//! A CSV has a header line; its separator is `;` when the header holds one, else `,`; the
//! last column takes the rest of the line (a description may hold the separator).

use std::collections::BTreeMap;

use toml_edit::{DocumentMut, Item, TableLike};

use super::ImportError;
use crate::inference::NodeKind;

/// One row of a CSV, by column name.
pub type Row = BTreeMap<String, String>;

/// Reads a CSV with a header line (see the module documentation).
///
/// # Errors
/// A missing header, or a required column absent.
pub fn csv(text: &str, what: &str, required: &[&str]) -> Result<Vec<Row>, ImportError> {
    let mut lines = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    let header = lines
        .next()
        .ok_or_else(|| ImportError::new(format!("{what} : fichier vide")))?;
    let separator = if header.contains(';') { ';' } else { ',' };
    let columns: Vec<String> = header
        .split(separator)
        .map(|c| c.trim().to_lowercase())
        .collect();
    for name in required {
        if !columns.iter().any(|c| c == name) {
            return Err(ImportError::new(format!("{what} : colonne {name} absente")));
        }
    }
    Ok(lines
        .map(|line| {
            columns
                .iter()
                .cloned()
                .zip(
                    line.splitn(columns.len(), separator)
                        .map(|v| v.trim().to_owned()),
                )
                .filter(|(_, v)| !v.is_empty())
                .collect()
        })
        .collect())
}

fn field<'a>(row: &'a Row, name: &str, what: &str, line: usize) -> Result<&'a str, ImportError> {
    row.get(name)
        .map(String::as_str)
        .ok_or_else(|| ImportError::new(format!("{what}, ligne {line} : {name} vide")))
}

/// State of a cell of the matrix of origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellState {
    /// Allowed.
    Allow,
    /// Denied.
    Deny,
    /// « Partiel »: to be reviewed by hand (Q11).
    Partial,
}

/// A cell of the matrix of origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Environment of the source, as written.
    pub source: String,
    /// Flow type, as written.
    pub flow_type: String,
    /// Environment of the destination, as written.
    pub destination: String,
    /// State.
    pub state: CellState,
}

/// Reads the matrix.
///
/// # Errors
/// A column missing or a state unknown.
pub fn matrix(text: &str) -> Result<Vec<Cell>, ImportError> {
    const WHAT: &str = "matrice";
    let rows = csv(text, WHAT, &["source", "flow_type", "destination", "state"])?;
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let line = i + 2;
            let state = match field(row, "state", WHAT, line)?.to_lowercase().as_str() {
                "allow" | "autorise" | "autorisé" => CellState::Allow,
                "deny" | "interdit" => CellState::Deny,
                "partial" | "partiel" => CellState::Partial,
                other => {
                    return Err(ImportError::new(format!(
                        "{WHAT}, ligne {line} : état {other:?} (allow, deny ou partial)"
                    )));
                }
            };
            Ok(Cell {
                source: field(row, "source", WHAT, line)?.to_owned(),
                flow_type: field(row, "flow_type", WHAT, line)?.to_owned(),
                destination: field(row, "destination", WHAT, line)?.to_owned(),
                state,
            })
        })
        .collect()
}

/// A row of the first table of the catalogue: one port (or range) of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortRow {
    /// The named service.
    pub service: String,
    /// Transport (`tcp`, `udp`, `icmp`…).
    pub transport: String,
    /// A port or a range `first-last`, none for a transport without ports.
    pub ports: Option<(i64, i64)>,
}

/// Reads the first table of the catalogue.
///
/// # Errors
/// A column missing or a port unreadable.
pub fn ports(text: &str) -> Result<Vec<PortRow>, ImportError> {
    const WHAT: &str = "catalogue (ports)";
    let rows = csv(text, WHAT, &["service", "transport", "port"])?;
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let line = i + 2;
            let ports = row
                .get("port")
                .map(|p| {
                    let (first, last) = p.split_once('-').unwrap_or((p, p));
                    match (first.trim().parse(), last.trim().parse()) {
                        (Ok(f), Ok(l)) => Ok((f, l)),
                        _ => Err(ImportError::new(format!(
                            "{WHAT}, ligne {line} : port {p:?} illisible"
                        ))),
                    }
                })
                .transpose()?;
            Ok(PortRow {
                service: field(row, "service", WHAT, line)?.to_owned(),
                transport: field(row, "transport", WHAT, line)?.to_lowercase(),
                ports,
            })
        })
        .collect()
}

/// A row of the second table of the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRow {
    /// The named service.
    pub service: String,
    /// Whether it is allowed (« autorisé = faux » is a guard-rail without a pair, S6).
    pub allowed: bool,
    /// Description.
    pub description: Option<String>,
    /// Its flow type, when the table gives one.
    pub flow_type: Option<String>,
}

/// Reads the second table of the catalogue.
///
/// # Errors
/// A column missing or a boolean unreadable.
pub fn services(text: &str) -> Result<Vec<ServiceRow>, ImportError> {
    const WHAT: &str = "catalogue (services)";
    let rows = csv(text, WHAT, &["service", "allowed"])?;
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let line = i + 2;
            let allowed = match field(row, "allowed", WHAT, line)?.to_lowercase().as_str() {
                "true" | "vrai" | "oui" | "yes" | "1" => true,
                "false" | "faux" | "non" | "no" | "0" => false,
                other => {
                    return Err(ImportError::new(format!(
                        "{WHAT}, ligne {line} : autorisé {other:?} (vrai ou faux)"
                    )));
                }
            };
            Ok(ServiceRow {
                service: field(row, "service", WHAT, line)?.to_owned(),
                allowed,
                description: row.get("description").cloned(),
                flow_type: row.get("flow_type").cloned(),
            })
        })
        .collect()
}

/// A site: its name in the rules of origin, its identifier in kartenn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// Name.
    pub name: String,
    /// Identifier.
    pub id: uuid::Uuid,
}

/// Reads the sites.
///
/// # Errors
/// A column missing or an identifier unreadable.
pub fn sites(text: &str) -> Result<Vec<Site>, ImportError> {
    const WHAT: &str = "sites";
    let rows = csv(text, WHAT, &["name", "id"])?;
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let line = i + 2;
            let id = field(row, "id", WHAT, line)?;
            Ok(Site {
                name: field(row, "name", WHAT, line)?.to_owned(),
                id: id.parse().map_err(|_| {
                    ImportError::new(format!("{WHAT}, ligne {line} : {id:?} n'est pas un UUID"))
                })?,
            })
        })
        .collect()
}

/// A node of the addressing tree, for the simulation of the inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRow {
    /// Its name in the file.
    pub id: String,
    /// Its parent, by name.
    pub parent: Option<String>,
    /// Kind.
    pub kind: NodeKind,
    /// Name, as the rules read it.
    pub name: Option<String>,
    /// Prefix (an address: its host prefix).
    pub prefix: Option<String>,
    /// Name of its site.
    pub site: Option<String>,
}

/// Reads the nodes.
///
/// # Errors
/// A column missing or a kind unknown.
pub fn nodes(text: &str) -> Result<Vec<NodeRow>, ImportError> {
    const WHAT: &str = "nœuds";
    let rows = csv(text, WHAT, &["id", "kind"])?;
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let line = i + 2;
            let kind = match field(row, "kind", WHAT, line)? {
                "domain" => NodeKind::Domain,
                "container" => NodeKind::Container,
                "network" => NodeKind::Network,
                "address" => NodeKind::Address,
                other => {
                    return Err(ImportError::new(format!(
                        "{WHAT}, ligne {line} : genre {other:?} (domain, container, network, address)"
                    )));
                }
            };
            Ok(NodeRow {
                id: field(row, "id", WHAT, line)?.to_owned(),
                parent: row.get("parent").cloned(),
                kind,
                name: row.get("name").cloned(),
                prefix: row.get("prefix").cloned(),
                site: row.get("site").cloned(),
            })
        })
        .collect()
}

/// A constraint of a rule of origin (Q10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    /// A regular expression searched in the name.
    Name(String),
    /// The prefix lies inside this one (`subnet_of`).
    Within(String),
    /// The prefix is this one.
    Equals(String),
    /// A regular expression searched in the name of the site.
    Site(String),
    /// A constraint this converter does not know.
    Unknown(String),
}

/// A rule of origin: constraints (AND), labels, nested exceptions (Q10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRule {
    /// Name of its table.
    pub name: String,
    /// The file it was read from.
    pub file: String,
    /// Labels; an exception replaces those of its parent (D26).
    pub labels: Vec<String>,
    /// Constraints.
    pub constraints: Vec<Constraint>,
    /// Exceptions, in written order.
    pub exceptions: Vec<SourceRule>,
}

/// The tables an item holds: a table, an array of tables or an array of inline tables.
fn tables(item: &Item) -> Vec<&dyn TableLike> {
    if let Some(array) = item.as_array_of_tables() {
        array.iter().map(|t| t as &dyn TableLike).collect()
    } else if let Some(array) = item.as_array() {
        array
            .iter()
            .filter_map(|v| v.as_inline_table().map(|t| t as &dyn TableLike))
            .collect()
    } else {
        item.as_table_like().into_iter().collect()
    }
}

fn constraint(table: &dyn TableLike) -> Constraint {
    let mut members = table.iter();
    let (Some((key, item)), None) = (members.next(), members.next()) else {
        return Constraint::Unknown(format!("{} membres", table.len()));
    };
    let text = |i: &Item| i.as_str().map(str::to_owned);
    match key {
        "name" => text(item).map_or_else(|| Constraint::Unknown(key.into()), Constraint::Name),
        "site" => text(item).map_or_else(|| Constraint::Unknown(key.into()), Constraint::Site),
        "cidr" => {
            if let Some(prefix) = text(item) {
                return Constraint::Within(prefix);
            }
            let inner = item.as_table_like();
            match (
                inner.and_then(|t| t.get("subnet_of")).and_then(text),
                inner.and_then(|t| t.get("equals")).and_then(text),
            ) {
                (Some(p), None) => Constraint::Within(p),
                (None, Some(p)) => Constraint::Equals(p),
                _ => Constraint::Unknown(key.into()),
            }
        }
        other => Constraint::Unknown(other.into()),
    }
}

fn rule(name: &str, file: &str, table: &dyn TableLike) -> Result<SourceRule, ImportError> {
    let mut rule = SourceRule {
        name: name.to_owned(),
        file: file.to_owned(),
        labels: Vec::new(),
        constraints: Vec::new(),
        exceptions: Vec::new(),
    };
    for (key, item) in table.iter() {
        match key {
            "labels" => {
                let array = item.as_array().ok_or_else(|| {
                    ImportError::new(format!("{file}, {name} : labels est une liste"))
                })?;
                for value in array {
                    let label = value.as_str().ok_or_else(|| {
                        ImportError::new(format!("{file}, {name} : une étiquette est un texte"))
                    })?;
                    rule.labels.push(label.to_owned());
                }
            }
            "constraints" => rule
                .constraints
                .extend(tables(item).into_iter().map(constraint)),
            "exception" => {
                for (i, exception) in tables(item).into_iter().enumerate() {
                    let child = exception
                        .get("name")
                        .and_then(Item::as_str)
                        .map_or_else(|| format!("{name}-exception-{}", i + 1), str::to_owned);
                    rule.exceptions.push(self::rule(&child, file, exception)?);
                }
            }
            "name" => {}
            other => rule.constraints.push(Constraint::Unknown(other.to_owned())),
        }
    }
    Ok(rule)
}

/// Reads the rule files, given in their reading order (the tool of origin read them by
/// name, general first): each top-level table is a rule, in written order.
///
/// # Errors
/// A file that is not TOML, or a rule written with the wrong types.
pub fn rules(files: &[(String, String)]) -> Result<Vec<SourceRule>, ImportError> {
    let mut out = Vec::new();
    for (file, text) in files {
        let document: DocumentMut = text
            .parse()
            .map_err(|e| ImportError::new(format!("{file} : {e}")))?;
        for (name, item) in document.iter() {
            let table = item
                .as_table_like()
                .ok_or_else(|| ImportError::new(format!("{file} : {name} n'est pas une table")))?;
            out.push(rule(name, file, table)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csv_reads_its_header_and_keeps_the_rest_in_the_last_column() {
        let rows = csv(
            "service;allowed;description\nssh;vrai;Accès distant; chiffré\n",
            "t",
            &["service"],
        )
        .unwrap();
        assert_eq!(rows[0]["description"], "Accès distant; chiffré");
        assert!(csv("a,b\n", "t", &["c"]).is_err());
    }

    #[test]
    fn rules_keep_their_written_order_and_nest_exceptions() {
        let rules = rules(&[(
            "10-x.toml".into(),
            r#"
[zeta]
labels = ["prod"]
constraints = [{ name = "^prd" }, { cidr = { subnet_of = "10.0.0.0/8" } }]
[zeta.exception]
labels = ["rec"]
constraints = [{ site = "^lab" }]

[alpha]
labels = ["dev"]
constraints = [{ owner = "x" }]
"#
            .into(),
        )])
        .unwrap();
        assert_eq!(rules[0].name, "zeta");
        assert_eq!(rules[1].name, "alpha");
        assert_eq!(rules[0].exceptions[0].labels, ["rec"]);
        assert_eq!(
            rules[0].constraints[1],
            Constraint::Within("10.0.0.0/8".into())
        );
        assert_eq!(rules[1].constraints, [Constraint::Unknown("owner".into())]);
    }
}
