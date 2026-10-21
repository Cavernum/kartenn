//! `kartenn-policy-import`: converts an existing policy into one changeset, and lists in
//! simulation where the converted policy answers otherwise than the tool of origin
//! (task 010-17; REQ-010-35, 53; S28). Offline: it reads files and writes one; the
//! changeset is then submitted (or simulated) by `POST /v1/changesets`.

use std::collections::BTreeSet;
use std::process::ExitCode;

use kartenn_policy::import::convert::{Inputs, Options, convert};
use kartenn_policy::import::simulate;
use kartenn_policy::import::{ImportError, source};
use kartenn_policy::model::AxisKey;

const USAGE: &str = "\
usage: kartenn-policy-import [options]
  --matrix <csv>          matrice source,flow_type,destination,state (allow, deny, partial)
  --ports <csv>           catalogue, première table service,transport,port
  --services <csv>        catalogue, seconde table service,allowed,description[,flow_type]
  --rules <répertoire>    règles d'inférence, fichiers .toml lus dans l'ordre de leurs noms
  --sites <csv>           sites name,id (contraintes sur le nom du site)
  --nodes <csv>           nœuds id,parent,kind,name,prefix,site (simulation de l'inférence)
  --axis <clé>            axe des règles d'inférence (policy.environment)
  --default-flow-type <n> type de flux d'un service que la seconde table ne type pas
  --existing-environments <a,b>  environnements déjà présents, non créés
  --motive <texte>        motif du changeset
  --out <fichier>         écrit le changeset (corps de POST /v1/changesets) ; sans lui,
                          simulation seule
Sortie standard : les avis de conversion, puis les différences de la simulation.";

struct Args {
    matrix: Option<String>,
    ports: Option<String>,
    services: Option<String>,
    rules: Option<String>,
    sites: Option<String>,
    nodes: Option<String>,
    axis: Option<String>,
    default_flow_type: Option<String>,
    existing: Vec<String>,
    motive: String,
    out: Option<String>,
}

fn args() -> Result<Args, String> {
    let mut a = Args {
        matrix: None,
        ports: None,
        services: None,
        rules: None,
        sites: None,
        nodes: None,
        axis: None,
        default_flow_type: None,
        existing: Vec::new(),
        motive: "Import de la politique existante (kartenn-policy-import)".into(),
        out: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        if flag == "--help" || flag == "-h" {
            return Err(String::new());
        }
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} : valeur manquante"))?;
        match flag.as_str() {
            "--matrix" => a.matrix = Some(value),
            "--ports" => a.ports = Some(value),
            "--services" => a.services = Some(value),
            "--rules" => a.rules = Some(value),
            "--sites" => a.sites = Some(value),
            "--nodes" => a.nodes = Some(value),
            "--axis" => a.axis = Some(value),
            "--default-flow-type" => a.default_flow_type = Some(value),
            "--existing-environments" => {
                a.existing = value.split(',').map(|s| s.trim().to_owned()).collect();
            }
            "--motive" => a.motive = value,
            "--out" => a.out = Some(value),
            other => return Err(format!("option inconnue {other}")),
        }
    }
    if a.ports.is_some() != a.services.is_some() {
        return Err("--ports et --services vont ensemble".into());
    }
    Ok(a)
}

fn read(path: &str) -> Result<String, ImportError> {
    std::fs::read_to_string(path).map_err(|e| ImportError(format!("{path} : {e}")))
}

/// The `.toml` files of a directory, in the order of their names.
fn rule_files(dir: &str) -> Result<Vec<(String, String)>, ImportError> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| ImportError(format!("{dir} : {e}")))?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|n| Ok((n.clone(), read(&format!("{dir}/{n}"))?)))
        .collect()
}

fn run(a: &Args) -> Result<(), ImportError> {
    let inputs = Inputs {
        matrix: a
            .matrix
            .as_deref()
            .map(read)
            .transpose()?
            .as_deref()
            .map(source::matrix)
            .transpose()?
            .unwrap_or_default(),
        ports: a
            .ports
            .as_deref()
            .map(read)
            .transpose()?
            .as_deref()
            .map(source::ports)
            .transpose()?
            .unwrap_or_default(),
        services: a
            .services
            .as_deref()
            .map(read)
            .transpose()?
            .as_deref()
            .map(source::services)
            .transpose()?
            .unwrap_or_default(),
        rules: a
            .rules
            .as_deref()
            .map(rule_files)
            .transpose()?
            .as_deref()
            .map(source::rules)
            .transpose()?
            .unwrap_or_default(),
        sites: a
            .sites
            .as_deref()
            .map(read)
            .transpose()?
            .as_deref()
            .map(source::sites)
            .transpose()?
            .unwrap_or_default(),
    };
    let mut options = Options {
        default_flow_type: a.default_flow_type.clone(),
        existing_environments: a.existing.iter().cloned().collect::<BTreeSet<_>>(),
        ..Options::default()
    };
    if let Some(axis) = &a.axis {
        options.axis = AxisKey::parse(axis)
            .map_err(|_| ImportError(format!("{axis} n'est pas une clé de propriété")))?;
    }
    let conversion = convert(&inputs, &options);
    println!("# Conversion : {} opérations", conversion.operations.len());
    for notice in &conversion.notices {
        println!("avis      {notice}");
    }
    let matrix = simulate::matrix(&inputs, &conversion);
    let nodes = match &a.nodes {
        Some(path) => {
            let nodes = source::nodes(&read(path)?)?;
            simulate::inference(&inputs, &nodes, &conversion, &options.axis)?
        }
        None => Vec::new(),
    };
    println!("# Simulation : {} différences", matrix.len() + nodes.len());
    for d in &matrix {
        println!("{d}");
    }
    for d in &nodes {
        println!("{d}");
    }
    if let Some(out) = &a.out {
        let body = serde_json::json!({ "motive": a.motive, "operations": conversion.operations });
        let text = serde_json::to_string_pretty(&body).expect("JSON");
        std::fs::write(out, text + "\n").map_err(|e| ImportError(format!("{out} : {e}")))?;
        println!("# Changeset écrit dans {out}");
    }
    Ok(())
}

fn main() -> ExitCode {
    let a = match args() {
        Ok(a) => a,
        Err(why) => {
            if !why.is_empty() {
                eprintln!("{why}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
