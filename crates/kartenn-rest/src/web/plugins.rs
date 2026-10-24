//! The plugins of the shell (spec 007, REQ-007-4 to 8, 11; W3, W5; design « Plugins côté
//! serveur »): `<plugins_dir>/<name>/<version>/`, each manifest checked, every file
//! digested (`sha384`, SRI), the highest valid version published, read again every 10 s
//! and on `SIGHUP`, without rebuild nor restart.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use arc_swap::ArcSwap;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha384};

/// How often the directory is read again (REQ-007-8).
pub const RELOAD_EVERY: Duration = Duration::from_secs(10);
/// The most files of a version.
pub const MAX_FILES: usize = 1_000;
/// The most bytes of a version.
pub const MAX_BYTES: u64 = 20 * 1024 * 1024;

/// The points of extension of the SDK (W5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = UiExtensionPoint)]
pub enum Point {
    /// A route and its page, with an entry in the navigation.
    View,
    /// A tab on the page of a type of object.
    Tab,
    /// A panel on the page of a type of object.
    Panel,
    /// A column in the lists of a type of object.
    Column,
    /// An action of the menu of an object.
    Action,
}

/// The spaces of the sidebar, in their order (spec 021, REQ-021-2): a closed list, extended
/// by amendment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = UiSpace)]
pub enum Space {
    /// The referential: domains, prefixes, ranges, addresses, entities, sites.
    Referential,
    /// Changesets, reviews, drafts, imports.
    Changes,
    /// The network: VLAN and their groups.
    Network,
    /// The hosts.
    Hosts,
    /// The flow policy.
    Policy,
    /// The flows.
    Flows,
    /// The administration, last.
    Admin,
}

/// A point of extension a plugin fills.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = UiExtension)]
pub struct Extension {
    /// The point.
    pub point: Point,
    /// Unique in the plugin.
    pub id: String,
    /// The Custom Element, prefixed by the name of the plugin.
    pub element: String,
    /// The types of objects it applies to; empty for a view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_types: Vec<String>,
    /// The key of its label in the catalogues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The key of its entry of navigation in the catalogues (a view).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nav: Option<String>,
    /// The permission without which the shell hides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<String>,
    /// The letter of navigation it proposes after `g` (a view, `[a-z]`); the shell refuses
    /// a letter already taken (spec 021, design « API des plugins »).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub go: Option<String>,
}

/// `manifest.json` of a version.
#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    name: String,
    version: String,
    sdk: String,
    #[serde(default)]
    ui: Option<String>,
    #[serde(default)]
    framework: Option<String>,
    #[serde(default)]
    space: Option<Space>,
    entry: String,
    #[serde(default)]
    module: Option<String>,
    catalogs: BTreeMap<String, String>,
    #[serde(default)]
    extensions: Vec<Extension>,
}

/// A version published in `ui_plugins` (REQ-007-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Published {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// The range of the SDK it expects.
    pub sdk: String,
    /// The range of `@kartenn/ui` it expects, if it imports it (ADR 0038).
    pub ui: Option<String>,
    /// The range of the shared framework (`vue`) it expects, if it imports it.
    pub framework: Option<String>,
    /// The space of the sidebar its views go under.
    pub space: Option<Space>,
    /// The URL of its directory, `/plugins/<name>/<version>/`.
    pub base: String,
    /// The URL of its entry module.
    pub entry: String,
    /// The SRI digest of each module, style sheet and catalogue, by relative path.
    pub integrity: BTreeMap<String, String>,
    /// Its catalogues by language, relative paths.
    pub catalogs: BTreeMap<String, String>,
    /// The points it fills.
    pub extensions: Vec<Extension>,
}

/// A file of a valid version.
#[derive(Debug, Clone)]
pub(crate) struct File {
    pub(crate) disk: PathBuf,
    /// `sha384-…`: the SRI digest, and the validator of the file.
    pub(crate) sri: String,
}

/// What a reading of the directory found.
#[derive(Debug, Default)]
struct Snapshot {
    /// The highest valid version of each name.
    published: BTreeMap<String, Published>,
    /// The files of every valid version, by `(name, version, path)`.
    files: HashMap<(String, String, String), File>,
}

/// What the readings remember: digests by path, and what was logged.
#[derive(Debug, Default)]
struct Memory {
    digests: HashMap<PathBuf, (u64, Option<SystemTime>, String)>,
    rejected: HashMap<(String, String), String>,
    loaded: BTreeSet<(String, String)>,
}

/// The registry of the plugins.
#[derive(Debug)]
pub struct Plugins {
    dir: PathBuf,
    /// The modules of the build: a plugin of another module is not published.
    modules: BTreeSet<String>,
    snapshot: ArcSwap<Snapshot>,
    memory: Mutex<Memory>,
}

impl Plugins {
    /// The registry of `dir`, read once.
    ///
    /// # Errors
    ///
    /// `dir` cannot be read.
    pub fn open(dir: &Path, modules: BTreeSet<String>) -> std::io::Result<Self> {
        std::fs::read_dir(dir)?;
        let plugins = Self {
            dir: dir.to_owned(),
            modules,
            snapshot: ArcSwap::from_pointee(Snapshot::default()),
            memory: Mutex::new(Memory::default()),
        };
        plugins.reload();
        Ok(plugins)
    }

    /// The versions published now, by name.
    #[must_use]
    pub fn published(&self) -> Vec<Published> {
        self.snapshot.load().published.values().cloned().collect()
    }

    /// A file of a valid version.
    pub(crate) fn file(&self, name: &str, version: &str, path: &str) -> Option<File> {
        self.snapshot
            .load()
            .files
            .get(&(name.to_owned(), version.to_owned(), path.to_owned()))
            .cloned()
    }

    /// Reads the directory again and publishes what it holds at once; an unreadable
    /// directory keeps the previous snapshot.
    pub fn reload(&self) {
        let mut memory = self.memory.lock().unwrap_or_else(PoisonError::into_inner);
        let names = match std::fs::read_dir(&self.dir) {
            Ok(names) => names,
            Err(e) => {
                tracing::warn!(name: "web.plugins.unreadable", error = %e, dir = %self.dir.display(), "plugins directory unreadable, the previous plugins stay");
                return;
            }
        };
        let mut seen = HashMap::new();
        let mut valid: BTreeMap<String, Vec<(semver::Version, Published)>> = BTreeMap::new();
        let mut files = HashMap::new();
        let mut rejected = HashMap::new();
        for (name, version, path) in versions(names) {
            match self.version(&name, &version, &path, &memory.digests, &mut seen) {
                Ok((published, found)) => {
                    for (relative, file) in found {
                        files.insert((name.clone(), version.clone(), relative), file);
                    }
                    if let Ok(parsed) = semver::Version::parse(&version) {
                        valid.entry(name).or_default().push((parsed, published));
                    }
                }
                Err(reason) => {
                    rejected.insert((name, version), reason);
                }
            }
        }
        for ((name, version), reason) in &rejected {
            if memory.rejected.get(&(name.clone(), version.clone())) != Some(reason) {
                tracing::warn!(name: "web.plugin.rejected", plugin = %name, version = %version, reason = %reason, "plugin rejected");
            }
        }
        let mut loaded = BTreeSet::new();
        for (name, versions) in &valid {
            for (_, published) in versions {
                let key = (name.clone(), published.version.clone());
                if !memory.loaded.contains(&key) {
                    tracing::info!(name: "web.plugin.loaded", plugin = %name, version = %published.version, "plugin loaded");
                }
                loaded.insert(key);
            }
        }
        let published = valid
            .into_iter()
            .filter_map(|(name, mut versions)| {
                versions.sort_by(|a, b| a.0.cmp(&b.0));
                versions.pop().map(|(_, published)| (name, published))
            })
            .collect();
        memory.digests = seen;
        memory.rejected = rejected;
        memory.loaded = loaded;
        self.snapshot.store(Arc::new(Snapshot { published, files }));
    }

    /// Reads the directory again every [`RELOAD_EVERY`], off the runtime.
    pub fn watch(self: &Arc<Self>, every: Duration) -> tokio::task::JoinHandle<()> {
        let plugins = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + every, every);
            loop {
                tick.tick().await;
                let plugins = plugins.clone();
                let _ = tokio::task::spawn_blocking(move || plugins.reload()).await;
            }
        })
    }

    /// A version checked: its publication and its files, or why it is rejected.
    fn version(
        &self,
        name: &str,
        version: &str,
        path: &Path,
        known: &HashMap<PathBuf, (u64, Option<SystemTime>, String)>,
        seen: &mut HashMap<PathBuf, (u64, Option<SystemTime>, String)>,
    ) -> Result<(Published, Vec<(String, File)>), String> {
        let mut listed = Vec::new();
        list(path, "", &mut listed)?;
        if listed.len() > MAX_FILES {
            return Err(format!("more than {MAX_FILES} files"));
        }
        let bytes: u64 = listed.iter().map(|(_, _, meta)| meta.len()).sum();
        if bytes > MAX_BYTES {
            return Err(format!("more than {MAX_BYTES} bytes"));
        }
        let manifest =
            std::fs::read(path.join("manifest.json")).map_err(|e| format!("manifest.json: {e}"))?;
        let manifest: Manifest =
            serde_json::from_slice(&manifest).map_err(|e| format!("manifest.json: {e}"))?;
        let names: BTreeSet<&str> = listed.iter().map(|(r, _, _)| r.as_str()).collect();
        let catalogs = check(&manifest, name, version, &names, path)?;
        if let Some(module) = &manifest.module
            && !self.modules.contains(module)
        {
            return Err(format!("module {module} absent from the build"));
        }
        let mut files = Vec::new();
        let mut integrity = BTreeMap::new();
        for (relative, disk, meta) in listed {
            let stamp = (meta.len(), meta.modified().ok());
            let sri = match known.get(&disk) {
                Some((len, modified, sri)) if (*len, *modified) == stamp => sri.clone(),
                _ => sri(&disk).map_err(|e| format!("{relative}: {e}"))?,
            };
            seen.insert(disk.clone(), (stamp.0, stamp.1, sri.clone()));
            if ["js", "css", "json"]
                .iter()
                .any(|extension| has_extension(&relative, extension))
            {
                integrity.insert(relative.clone(), sri.clone());
            }
            files.push((relative, File { disk, sri }));
        }
        let base = format!("/plugins/{name}/{version}/");
        Ok((
            Published {
                name: name.to_owned(),
                version: version.to_owned(),
                sdk: manifest.sdk,
                ui: manifest.ui,
                framework: manifest.framework,
                space: manifest.space,
                entry: format!("{base}{}", manifest.entry),
                base,
                integrity,
                catalogs,
                extensions: manifest.extensions,
            },
            files,
        ))
    }
}

/// The `(name, version, path)` of every directory `<name>/<version>/`, hidden ones aside.
fn versions(names: std::fs::ReadDir) -> Vec<(String, String, PathBuf)> {
    let mut found = Vec::new();
    for entry in names.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Ok(versions) = std::fs::read_dir(entry.path()) else {
            continue;
        };
        for version in versions.flatten() {
            let Ok(number) = version.file_name().into_string() else {
                continue;
            };
            if !number.starts_with('.') && version.file_type().is_ok_and(|t| t.is_dir()) {
                found.push((name.clone(), number, version.path()));
            }
        }
    }
    found
}

/// The regular files under `dir`, by path relative to the version; a symbolic link or
/// anything but a file or a directory rejects the version.
fn list(
    dir: &Path,
    prefix: &str,
    found: &mut Vec<(String, PathBuf, std::fs::Metadata)>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{prefix}: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{prefix}: {e}"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("{prefix}: a name that is not UTF-8"))?;
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let meta =
            std::fs::symlink_metadata(entry.path()).map_err(|e| format!("{relative}: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!("{relative}: symbolic link"));
        } else if meta.is_dir() {
            list(&entry.path(), &relative, found)?;
        } else if meta.is_file() {
            found.push((relative, entry.path(), meta));
            if found.len() > MAX_FILES {
                return Err(format!("more than {MAX_FILES} files"));
            }
        } else {
            return Err(format!("{relative}: not a regular file"));
        }
    }
    Ok(())
}

/// `sha384-…` of a file, as `integrity` expects it.
fn sri(path: &Path) -> std::io::Result<String> {
    let body = std::fs::read(path)?;
    Ok(format!("sha384-{}", STANDARD.encode(Sha384::digest(body))))
}

/// The checks of a manifest (REQ-007-5, design « Règles de validation »); the catalogues
/// on success.
fn check(
    manifest: &Manifest,
    name: &str,
    version: &str,
    files: &BTreeSet<&str>,
    dir: &Path,
) -> Result<BTreeMap<String, String>, String> {
    if !plugin_name(&manifest.name) {
        return Err(format!("name {}: [a-z][a-z0-9-]{{0,39}}", manifest.name));
    }
    if manifest.name != name {
        return Err(format!(
            "name {} differs from its directory {name}",
            manifest.name
        ));
    }
    let parsed = semver::Version::parse(&manifest.version)
        .map_err(|e| format!("version {}: {e}", manifest.version))?;
    if !parsed.build.is_empty() {
        return Err(format!("version {}: build metadata", manifest.version));
    }
    if manifest.version != version {
        return Err(format!(
            "version {} differs from its directory {version}",
            manifest.version
        ));
    }
    for (field, value) in [
        ("sdk", Some(&manifest.sdk)),
        ("ui", manifest.ui.as_ref()),
        ("framework", manifest.framework.as_ref()),
    ] {
        if let Some(value) = value
            && !version_range(value)
        {
            return Err(format!("{field} {value}: ^x[.y[.z]], ~x.y[.z] or x.y.z"));
        }
    }
    if !has_extension(&manifest.entry, "js") || !files.contains(manifest.entry.as_str()) {
        return Err(format!("entry {}: no such module", manifest.entry));
    }
    if !manifest.catalogs.contains_key("en") {
        return Err("catalogs: en is missing".into());
    }
    let mut catalogs = Vec::new();
    for (language, path) in &manifest.catalogs {
        if !has_extension(path, "json") || !files.contains(path.as_str()) {
            return Err(format!("catalog {language}: {path} is missing"));
        }
        let text = std::fs::read(dir.join(path)).map_err(|e| format!("catalog {path}: {e}"))?;
        let catalog: serde_json::Value =
            serde_json::from_slice(&text).map_err(|e| format!("catalog {path}: {e}"))?;
        if !catalog.is_object() {
            return Err(format!("catalog {path}: not an object"));
        }
        catalogs.push((language, catalog));
    }
    let mut ids = BTreeSet::new();
    let mut letters = BTreeSet::new();
    for extension in &manifest.extensions {
        if !ids.insert(extension.id.as_str()) {
            return Err(format!("extension {}: id used twice", extension.id));
        }
        if let Some(go) = &extension.go {
            if extension.point != Point::View {
                return Err(format!(
                    "extension {}: go on a {:?}",
                    extension.id, extension.point
                ));
            }
            if go.len() != 1 || !go.chars().all(|c| c.is_ascii_lowercase()) {
                return Err(format!("extension {}: go {go}: [a-z]", extension.id));
            }
            if !letters.insert(go.as_str()) {
                return Err(format!("extension {}: go {go} used twice", extension.id));
            }
        }
        if !element_name(&extension.element, name) {
            return Err(format!(
                "extension {}: element {} is not a custom element prefixed by {name}-",
                extension.id, extension.element
            ));
        }
        if extension.point != Point::View && extension.object_types.is_empty() {
            return Err(format!("extension {}: no object_types", extension.id));
        }
        for key in [&extension.label, &extension.nav].into_iter().flatten() {
            for (language, catalog) in &catalogs {
                if !has_key(catalog, key) {
                    return Err(format!(
                        "extension {}: {key} missing from the catalog {language}",
                        extension.id
                    ));
                }
            }
        }
    }
    Ok(manifest.catalogs.clone())
}

/// The extension of a path, compared exactly: the browser asks for the file by its name.
fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path).extension().is_some_and(|e| e == extension)
}

fn plugin_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && name.len() <= 40
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A valid name of Custom Element (HTML, « valid custom element name », restricted to
/// ASCII), prefixed by `<plugin>-`.
fn element_name(element: &str, plugin: &str) -> bool {
    const RESERVED: [&str; 8] = [
        "annotation-xml",
        "color-profile",
        "font-face",
        "font-face-src",
        "font-face-uri",
        "font-face-format",
        "font-face-name",
        "missing-glyph",
    ];
    element
        .strip_prefix(plugin)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|rest| !rest.is_empty())
        && element
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.' | '_'))
        && !RESERVED.contains(&element)
}

/// `^x[.y[.z]]`, `~x.y[.z]` or `x.y.z` (design « Règles de validation »): the ranges `sdk`,
/// `ui` and `framework` of a manifest.
fn version_range(range: &str) -> bool {
    let numbers = |text: &str, min: usize| {
        let parts: Vec<&str> = text.split('.').collect();
        (min..=3).contains(&parts.len())
            && parts.iter().all(|p| {
                !p.is_empty()
                    && p.chars().all(|c| c.is_ascii_digit())
                    && (p.len() == 1 || !p.starts_with('0'))
            })
    };
    if let Some(rest) = range.strip_prefix('^') {
        numbers(rest, 1)
    } else if let Some(rest) = range.strip_prefix('~') {
        numbers(rest, 2)
    } else {
        numbers(range, 3)
    }
}

/// A key of a catalogue, flat (`"tab.notes"`) or nested (`{"tab": {"notes": …}}`).
fn has_key(catalog: &serde_json::Value, key: &str) -> bool {
    catalog.get(key).is_some()
        || key
            .split('.')
            .try_fold(catalog, |value, part| value.get(part))
            .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_of_versions() {
        for good in ["^1", "^1.2", "^1.2.3", "~1.2", "~1.2.3", "1.2.3", "^0.10"] {
            assert!(version_range(good), "{good}");
        }
        for bad in [
            "", "^", "1", "1.2", "~1", ">=1.0", "^01.2", "^1.x", "^1.2.3.4", "*",
        ] {
            assert!(!version_range(bad), "{bad}");
        }
    }

    #[test]
    fn names_of_plugins_and_elements() {
        assert!(plugin_name("example") && plugin_name("vlan-2"));
        assert!(!plugin_name("Example") && !plugin_name("2x") && !plugin_name(&"a".repeat(41)));
        assert!(element_name("example-notes", "example"));
        for bad in [
            "autre-onglet",
            "example",
            "example-",
            "example-Notes",
            "examplenotes",
        ] {
            assert!(!element_name(bad, "example"), "{bad}");
        }
        assert!(!element_name("font-face", "font"));
    }

    #[test]
    fn keys_of_catalogues_flat_or_nested() {
        let catalog = serde_json::json!({"tab.notes": "Notes", "nav": {"overview": "Vue"}});
        assert!(has_key(&catalog, "tab.notes"));
        assert!(has_key(&catalog, "nav.overview"));
        assert!(!has_key(&catalog, "nav.absent"));
    }
}
