use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use thiserror::Error;

const MAX_PACKAGES: usize = 128;
const MAX_ACTIONS: usize = 512;
const MAX_ACTIONS_PER_PACKAGE: usize = 64;
/// An editor commonly removes the destination immediately before atomically
/// renaming its temporary manifest into place.  Keep a live package through
/// that short gap; a subsequent demand scan after the debounce removes an
/// actual deletion.
const MISSING_SOURCE_DEBOUNCE: Duration = Duration::from_millis(750);

use star_contracts::{
    canonical::{Sha256Hash, canonical_sha256},
    manifest::{
        ActionDescriptor, ManifestError, ManifestSource, ToolPackageManifest, UpdatePolicy,
        parse_manifest_v1, version_requirement_matches,
    },
};

use crate::manifest_resources::validate_manifest_resources;

#[derive(Clone, Debug)]
pub struct RegistrySourceRoot {
    pub source: ManifestSource,
    pub directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ActivePackage {
    pub manifest: ToolPackageManifest,
    pub source: ManifestSource,
    pub source_hash: Sha256Hash,
    pub path: PathBuf,
    pub resolved_executable_hashes: BTreeMap<String, Sha256Hash>,
}

#[derive(Clone, Debug)]
pub struct SearchHit<'a> {
    pub package: &'a ActivePackage,
    pub action: &'a ActionDescriptor,
    pub score: i32,
    pub matched_fields: Vec<&'static str>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplacementResolution {
    /// target package -> trusted active replacer package
    pub replaced_by: BTreeMap<String, String>,
    pub conflicts: BTreeSet<String>,
}

#[derive(Debug, Error)]
pub enum RegistryCacheError {
    #[error("registry cache I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("registry cache is corrupt or from another format")]
    Corrupt,
    #[error("registry cache DACL failed")]
    Dacl,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCacheFile {
    format_version: u32,
    active: BTreeMap<String, CachedPackage>,
    revision: u64,
    diagnostic_revision: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedPackage {
    manifest: ToolPackageManifest,
    source: String,
    source_hash: Sha256Hash,
    path: PathBuf,
    #[serde(default)]
    resolved_executable_hashes: BTreeMap<String, Sha256Hash>,
}

#[derive(Default)]
pub struct RegistryRuntime {
    active: BTreeMap<String, ActivePackage>,
    pending_compatible: BTreeMap<String, ActivePackage>,
    pub diagnostics: BTreeMap<PathBuf, String>,
    pub revision: u64,
    pub diagnostic_revision: u64,
    missing_since: BTreeMap<PathBuf, Instant>,
    cache_loaded: bool,
}

impl RegistryRuntime {
    pub fn load_cache(path: &Path) -> Result<Self, RegistryCacheError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(RegistryCacheError::Io(error)),
        };
        let cache: RegistryCacheFile =
            serde_json::from_slice(&bytes).map_err(|_| RegistryCacheError::Corrupt)?;
        if cache.format_version != 1 {
            return Err(RegistryCacheError::Corrupt);
        }
        let mut active = BTreeMap::new();
        for (package_id, package) in cache.active {
            let source = match package.source.as_str() {
                "release" => ManifestSource::Release,
                "user" => ManifestSource::User,
                "project" => ManifestSource::Project,
                _ => return Err(RegistryCacheError::Corrupt),
            };
            active.insert(
                package_id,
                ActivePackage {
                    manifest: package.manifest,
                    source,
                    source_hash: package.source_hash,
                    path: package.path,
                    resolved_executable_hashes: package.resolved_executable_hashes,
                },
            );
        }
        Ok(Self {
            active,
            pending_compatible: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            revision: cache.revision,
            diagnostic_revision: cache.diagnostic_revision,
            missing_since: BTreeMap::new(),
            cache_loaded: true,
        })
    }

    pub fn persist_cache(&self, path: &Path) -> Result<(), RegistryCacheError> {
        let parent = path.parent().ok_or(RegistryCacheError::Corrupt)?;
        fs::create_dir_all(parent)?;
        let active = self
            .active
            .iter()
            .map(|(package_id, package)| {
                (
                    package_id.clone(),
                    CachedPackage {
                        manifest: package.manifest.clone(),
                        source: source_name(package.source).to_owned(),
                        source_hash: package.source_hash.clone(),
                        path: package.path.clone(),
                        resolved_executable_hashes: package.resolved_executable_hashes.clone(),
                    },
                )
            })
            .collect();
        let cache = RegistryCacheFile {
            format_version: 1,
            active,
            revision: self.revision,
            diagnostic_revision: self.diagnostic_revision,
        };
        let temporary = parent.join(format!(".registry-cache-{}.tmp", star_ipc::nonce()));
        fs::write(
            &temporary,
            serde_json::to_vec(&cache).map_err(|_| RegistryCacheError::Corrupt)?,
        )?;
        fs::OpenOptions::new()
            .write(true)
            .open(&temporary)?
            .sync_all()?;
        fs::rename(temporary, path)?;
        star_ipc::key_store::apply_owner_system_dacl(path).map_err(|_| RegistryCacheError::Dacl)
    }

    pub fn active(&self) -> &BTreeMap<String, ActivePackage> {
        &self.active
    }

    pub fn probe_candidate(&self, package_id: &str) -> Option<&ActivePackage> {
        self.pending_compatible
            .get(package_id)
            .or_else(|| self.active.get(package_id))
    }

    pub fn accept_compatible_probe(&mut self, package_id: &str) -> bool {
        let Some(package) = self.pending_compatible.remove(package_id) else {
            return false;
        };
        let changed = self.active.get(package_id).is_none_or(|active| {
            Self::package_semantic_hash(active) != Self::package_semantic_hash(&package)
        });
        self.diagnostics.remove(&package.path);
        self.active.insert(package_id.to_owned(), package);
        if changed {
            self.revision += 1;
        }
        self.diagnostic_revision += 1;
        true
    }

    pub fn reject_compatible_probe(&mut self, package_id: &str) -> bool {
        let Some(package) = self.pending_compatible.get(package_id) else {
            return false;
        };
        let changed = self.diagnostics.insert(
            package.path.clone(),
            "TOOL_PROBE_FAILED_LKG_RETAINED".to_owned(),
        ) != Some("TOOL_PROBE_FAILED_LKG_RETAINED".to_owned());
        if changed {
            self.diagnostic_revision += 1;
        }
        true
    }

    pub fn core_ready(&self) -> bool {
        self.active.get("star.control.core").is_some_and(|package| {
            package.source == ManifestSource::Release && package.manifest.required
        })
    }

    /// Package hashes are deterministic and intentionally exclude paths,
    /// timestamps and diagnostics. Descriptor hashes are added by the runtime
    /// normalization stage; this early snapshot still makes scanner changes
    /// observable without pinning a process path.
    pub fn snapshot_hash(&self) -> Sha256Hash {
        let packages: Vec<_> = self
            .active
            .iter()
            .map(|(package_id, package)| {
                serde_json::json!({"package_id": package_id, "package_hash": Self::package_semantic_hash(package)})
            })
            .collect();
        canonical_sha256(&serde_json::json!({"packages": packages}))
            .expect("registry snapshot value is canonical JSON")
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<&ActivePackage> {
        let mut seen = BTreeSet::new();
        self.search_actions(query)
            .into_iter()
            .filter_map(|hit| {
                seen.insert(hit.package.manifest.package_id.as_str())
                    .then_some(hit.package)
            })
            .take(limit)
            .collect()
    }

    /// Deterministic v1 lexical search.  Exact and prefix scores are based on
    /// the normalized whole query; summary/description scores count each
    /// distinct query token at most once per field.
    pub fn search_actions(&self, query: &str) -> Vec<SearchHit<'_>> {
        let trusted: BTreeSet<_> = self.active.keys().cloned().collect();
        self.search_actions_with_trust(query, &trusted)
    }

    pub fn search_actions_with_trust(
        &self,
        query: &str,
        trusted_packages: &BTreeSet<String>,
    ) -> Vec<SearchHit<'_>> {
        let normalized_query = normalize_search_text(query).trim().to_owned();
        let query_tokens = search_tokens(&normalized_query);
        let replacements = self.resolve_replacements(trusted_packages);
        let mut hits = Vec::new();
        for package in self.active.values() {
            if replacements
                .replaced_by
                .contains_key(&package.manifest.package_id)
                || replacements
                    .conflicts
                    .contains(&package.manifest.package_id)
            {
                continue;
            }
            for action in &package.manifest.actions {
                let tool_id = normalize_search_text(&action.tool_id);
                let aliases: Vec<_> = action
                    .aliases
                    .iter()
                    .map(|alias| normalize_search_text(alias))
                    .collect();
                let tags: BTreeSet<_> = action
                    .tags
                    .iter()
                    .chain(&action.task_kinds)
                    .map(|value| normalize_search_text(value))
                    .collect();
                let summary_tokens = search_tokens(&action.summary);
                let description_tokens = search_tokens(&action.description);
                let mut score = 0;
                let mut matched_fields = Vec::new();
                if normalized_query.is_empty() {
                    score = 1;
                } else {
                    if tool_id == normalized_query {
                        score += 1_000;
                        matched_fields.push("tool_id");
                    } else if tool_id.starts_with(&normalized_query) {
                        score += 600;
                        matched_fields.push("tool_id");
                    }
                    let alias_score = aliases.iter().fold(0, |best, alias| {
                        best.max(if alias == &normalized_query {
                            800
                        } else if alias.starts_with(&normalized_query) {
                            500
                        } else {
                            0
                        })
                    });
                    if alias_score > 0 {
                        score += alias_score;
                        matched_fields.push("alias");
                    }
                    if tags.contains(&normalized_query)
                        || query_tokens.iter().any(|token| tags.contains(token))
                    {
                        score += 300;
                        if action
                            .tags
                            .iter()
                            .map(|value| normalize_search_text(value))
                            .any(|value| value == normalized_query || query_tokens.contains(&value))
                        {
                            matched_fields.push("tag");
                        }
                        if action
                            .task_kinds
                            .iter()
                            .map(|value| normalize_search_text(value))
                            .any(|value| value == normalized_query || query_tokens.contains(&value))
                        {
                            matched_fields.push("task_kind");
                        }
                    }
                    let summary_matches = query_tokens.intersection(&summary_tokens).count() as i32;
                    if summary_matches > 0 {
                        score += summary_matches * 40;
                        matched_fields.push("summary");
                    }
                    let description_matches =
                        query_tokens.intersection(&description_tokens).count() as i32;
                    if description_matches > 0 {
                        score += description_matches * 10;
                        matched_fields.push("description");
                    }
                }
                if score > 0 {
                    matched_fields.sort_unstable();
                    matched_fields.dedup();
                    hits.push(SearchHit {
                        package,
                        action,
                        score,
                        matched_fields,
                    });
                }
            }
        }
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.action.tool_id.cmp(&right.action.tool_id))
        });
        hits
    }

    pub fn resolve_replacements(
        &self,
        trusted_packages: &BTreeSet<String>,
    ) -> ReplacementResolution {
        let mut candidates: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut conflicts = BTreeSet::new();
        for replacer in self.active.values() {
            let replacer_id = &replacer.manifest.package_id;
            if !trusted_packages.contains(replacer_id) {
                continue;
            }
            for replacement in &replacer.manifest.replaces {
                let Some(target) = self.active.get(&replacement.package_id) else {
                    conflicts.insert(replacer_id.clone());
                    continue;
                };
                if (target.source == ManifestSource::Release && target.manifest.required)
                    || !version_requirement_matches(
                        &replacement.version_req,
                        &target.manifest.package_version,
                    )
                {
                    conflicts.insert(replacer_id.clone());
                    continue;
                }
                candidates
                    .entry(replacement.package_id.clone())
                    .or_default()
                    .push(replacer_id.clone());
            }
        }
        let mut replaced_by = BTreeMap::new();
        for (target, mut replacers) in candidates {
            replacers.sort();
            replacers.dedup();
            if replacers.len() != 1 {
                conflicts.extend(replacers);
            } else {
                replaced_by.insert(target, replacers.remove(0));
            }
        }

        for start in replaced_by.keys().cloned().collect::<Vec<_>>() {
            let mut chain = Vec::new();
            let mut current = start;
            while let Some(next) = replaced_by.get(&current).cloned() {
                if chain.contains(&current) || chain.contains(&next) || chain.len() >= 8 {
                    conflicts.extend(chain.clone());
                    conflicts.insert(current);
                    conflicts.insert(next);
                    break;
                }
                chain.push(current);
                current = next;
            }
        }
        replaced_by.retain(|target, replacer| {
            !conflicts.contains(target) && !conflicts.contains(replacer)
        });
        ReplacementResolution {
            replaced_by,
            conflicts,
        }
    }

    pub fn find_action(&self, tool_id: &str) -> Option<(&ActivePackage, &ActionDescriptor)> {
        self.active.values().find_map(|package| {
            package
                .manifest
                .actions
                .iter()
                .find(|action| action.tool_id == tool_id)
                .map(|action| (package, action))
        })
    }

    pub fn descriptor_hash(package: &ActivePackage, action: &ActionDescriptor) -> Sha256Hash {
        let action = normalized_action(action);
        let executable_identity = package.resolved_executable_hashes.get(&action.backend_ref);
        canonical_sha256(&serde_json::json!({
            "package_id": package.manifest.package_id,
            "package_version": package.manifest.package_version,
            "source": source_name(package.source),
            "package_hash": Self::package_semantic_hash(package),
            "action": action,
            "resolved_executable_identity": executable_identity,
        }))
        .expect("normalized descriptor is canonical JSON")
    }

    /// Descriptor and snapshot identity use parsed manifest semantics, never
    /// source formatting. `source_hash` remains separately available for
    /// candidate provenance and trust decisions.
    pub fn package_semantic_hash(package: &ActivePackage) -> Sha256Hash {
        let manifest = normalized_manifest(&package.manifest);
        canonical_sha256(&serde_json::json!({
            "source": source_name(package.source),
            "manifest": manifest,
            "resolved_executable_hashes": package.resolved_executable_hashes,
        }))
        .expect("parsed manifest is canonical JSON")
    }

    /// Reads all source roots on every request as the watcher-loss fallback.
    /// A syntactically invalid replacement keeps the old package only when the
    /// same candidate file still exists. A deleted source file never revives
    /// from last-known-good state, and duplicate PackageId candidates are a
    /// conflict rather than a source-order override.
    pub fn demand_scan(&mut self, roots: &[RegistrySourceRoot]) {
        let before_active: BTreeMap<_, _> = self
            .active
            .iter()
            .map(|(id, package)| (id.clone(), Self::package_semantic_hash(package)))
            .collect();
        let before_diagnostics = self.diagnostics.clone();
        let mut candidates: BTreeMap<String, Vec<ActivePackage>> = BTreeMap::new();
        let mut present_files = BTreeSet::new();
        let mut valid_path_to_id = BTreeMap::new();
        let mut diagnostics = BTreeMap::new();

        for root in roots {
            for (path, text) in stable_candidate_texts(manifest_files(&root.directory)) {
                present_files.insert(path.clone());
                let Some(text) = text else {
                    diagnostics.insert(path, "TOOL_MANIFEST_STABILIZING".to_owned());
                    continue;
                };
                let manifest = match parse_manifest_v1(&text, root.source) {
                    Ok(manifest) => manifest,
                    Err(ManifestError::FutureFormatVersion(_)) => {
                        diagnostics.insert(path, "TOOL_MANIFEST_FUTURE_VERSION".to_owned());
                        continue;
                    }
                    Err(_) => {
                        diagnostics.insert(path, "TOOL_MANIFEST_INVALID".to_owned());
                        continue;
                    }
                };
                if validate_manifest_resources(&manifest, &path).is_err() {
                    diagnostics.insert(path, "TOOL_MANIFEST_SCHEMA_INVALID".to_owned());
                    continue;
                }
                if !manifest.enabled {
                    diagnostics.insert(path, "TOOL_PACKAGE_DISABLED".to_owned());
                    continue;
                }
                let Some(resolved_executable_hashes) = resolve_executable_hashes(&manifest) else {
                    diagnostics.insert(path, "TOOL_EXECUTABLE_UNAVAILABLE".to_owned());
                    continue;
                };
                valid_path_to_id.insert(path.clone(), manifest.package_id.clone());
                candidates
                    .entry(manifest.package_id.clone())
                    .or_default()
                    .push(ActivePackage {
                        source_hash: Sha256Hash::digest(text.as_bytes()),
                        source: root.source,
                        path,
                        resolved_executable_hashes,
                        manifest,
                    });
            }
        }

        let conflicts: BTreeSet<String> = candidates
            .iter()
            .filter(|(_, candidates)| candidates.len() > 1)
            .map(|(package_id, candidates)| {
                for candidate in candidates {
                    diagnostics.insert(candidate.path.clone(), "TOOL_PACKAGE_CONFLICT".to_owned());
                }
                package_id.clone()
            })
            .collect();
        let desired: BTreeMap<String, ActivePackage> = candidates
            .into_iter()
            .filter_map(|(package_id, mut candidates)| {
                (candidates.len() == 1).then(|| (package_id, candidates.remove(0)))
            })
            .collect();
        // At capacity, an already active valid package wins over a newly
        // discovered package regardless of lexical path/ID order.  This
        // prevents adding one candidate from evicting a working tool or
        // accidentally publishing more than the documented limit.
        let active_ids: BTreeSet<_> = self.active.keys().cloned().collect();
        let mut candidates: Vec<_> = desired.into_iter().collect();
        candidates.sort_by(|(left_id, _), (right_id, _)| {
            active_ids
                .contains(right_id)
                .cmp(&active_ids.contains(left_id))
                .then_with(|| left_id.cmp(right_id))
        });
        let mut selected_packages = 0usize;
        let mut selected_actions = 0usize;
        let mut desired = BTreeMap::new();
        for (package_id, package) in candidates {
            let action_count = package.manifest.actions.len();
            if action_count > MAX_ACTIONS_PER_PACKAGE
                || selected_packages >= MAX_PACKAGES
                || selected_actions.saturating_add(action_count) > MAX_ACTIONS
            {
                diagnostics.insert(package.path.clone(), "TOOL_REGISTRY_LIMIT".to_owned());
            } else {
                selected_packages += 1;
                selected_actions += action_count;
                desired.insert(package_id, package);
            }
        }

        // A version-compatible byte identity is only a candidate until its
        // declared probe (and signature policy in the Controller) succeeds.
        // The previously active package therefore remains the LKG. FollowPath
        // is intentionally different: its new full hash is published at once
        // so an old descriptor becomes stale and forces a fresh describe.
        let mut compatible_seen = BTreeSet::new();
        let mut probe_ready = BTreeMap::new();
        for (package_id, package) in desired {
            let requires_probe = package
                .manifest
                .executables
                .iter()
                .any(|executable| executable.update_policy == UpdatePolicy::VersionCompatible);
            let already_active = self.active.get(&package_id).is_some_and(|active| {
                Self::package_semantic_hash(active) == Self::package_semantic_hash(&package)
            });
            if requires_probe && !already_active {
                compatible_seen.insert(package_id.clone());
                diagnostics.insert(package.path.clone(), "TOOL_PROBE_REQUIRED".to_owned());
                self.pending_compatible.insert(package_id, package);
            } else {
                probe_ready.insert(package_id, package);
            }
        }
        self.pending_compatible
            .retain(|package_id, _| compatible_seen.contains(package_id));
        let desired = probe_ready;

        // An invalid replacement is the LKG case.  A freshly missing path is
        // also retained briefly so editor temp-rename saves do not cause a
        // transient tool disappearance.  Cache recovery is deliberately
        // different: a source absent on its first post-restart scan is a real
        // deletion and must never be revived from the cache.
        let cache_first_scan = self.cache_loaded;
        self.cache_loaded = false;
        let mut missing_since = std::mem::take(&mut self.missing_since);
        self.active.retain(|package_id, package| {
            if conflicts.contains(package_id) {
                return false;
            }
            if !present_files.contains(&package.path) {
                if cache_first_scan {
                    return false;
                }
                let first_missing = missing_since
                    .entry(package.path.clone())
                    .or_insert_with(Instant::now);
                if first_missing.elapsed() < MISSING_SOURCE_DEBOUNCE {
                    diagnostics.insert(package.path.clone(), "TOOL_MANIFEST_RENAMING".to_owned());
                    return true;
                }
                return false;
            }
            missing_since.remove(&package.path);
            match valid_path_to_id.get(&package.path) {
                Some(candidate_id) => candidate_id == package_id,
                None => matches!(
                    diagnostics.get(&package.path).map(String::as_str),
                    Some(
                        "TOOL_MANIFEST_INVALID"
                            | "TOOL_MANIFEST_SCHEMA_INVALID"
                            | "TOOL_MANIFEST_FUTURE_VERSION"
                            | "TOOL_EXECUTABLE_UNAVAILABLE"
                            | "TOOL_MANIFEST_STABILIZING"
                            | "TOOL_REGISTRY_LIMIT"
                    )
                ),
            }
        });
        missing_since.retain(|path, _| self.active.values().any(|package| &package.path == path));
        self.missing_since = missing_since;
        for (package_id, package) in desired {
            self.active.insert(package_id, package);
        }

        let after_active: BTreeMap<_, _> = self
            .active
            .iter()
            .map(|(id, package)| (id.clone(), Self::package_semantic_hash(package)))
            .collect();
        if before_active != after_active {
            self.revision += 1;
        }
        if before_diagnostics != diagnostics {
            self.diagnostic_revision += 1;
        }
        self.diagnostics = diagnostics;
    }
}

/// Normalize only fields documented as sets. Argument bindings, examples,
/// parameter order and all process sequences deliberately retain their source
/// order because they alter the execution contract.
fn normalized_manifest(manifest: &ToolPackageManifest) -> ToolPackageManifest {
    let mut manifest = manifest.clone();
    manifest.backend_kinds.sort_by_key(|kind| match kind {
        star_contracts::manifest::BackendKind::Process => "process",
        star_contracts::manifest::BackendKind::ControllerCommand => "controller_command",
    });
    for executable in &mut manifest.executables {
        executable.architectures.sort();
        executable.isolation_compatibility.sort();
    }
    for action in &mut manifest.actions {
        *action = normalized_action(action);
    }
    manifest
}

fn normalized_action(action: &ActionDescriptor) -> ActionDescriptor {
    let mut action = action.clone();
    action.aliases.sort();
    action.tags.sort();
    action.task_kinds.sort();
    action.permission_actions.sort();
    action
}

fn resolve_executable_hashes(
    manifest: &ToolPackageManifest,
) -> Option<BTreeMap<String, Sha256Hash>> {
    let mut identities = BTreeMap::new();
    for executable in &manifest.executables {
        let hash = match executable.update_policy {
            UpdatePolicy::PinnedHash => executable.sha256.clone()?,
            UpdatePolicy::VersionCompatible | UpdatePolicy::FollowPath => {
                let path = Path::new(executable.path.as_deref()?);
                if !path.is_absolute() {
                    return None;
                }
                let file = fs::File::open(path).ok()?;
                Sha256Hash::digest_reader(file).ok()?
            }
        };
        identities.insert(executable.executable_id.clone(), hash);
    }
    Some(identities)
}

/// Demand scan observes every candidate twice across one 250 ms window per
/// source root.  The previous per-file sleep made a 128-package scan take
/// tens of seconds, which is both a reload outage and a way for a writer to
/// starve Registry requests.  Size and last-write must still be unchanged
/// for each individual candidate before it is parsed.
fn stable_candidate_texts(paths: Vec<PathBuf>) -> Vec<(PathBuf, Option<String>)> {
    let first: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let metadata = fs::metadata(&path).ok();
            let stamp = metadata
                .as_ref()
                .and_then(|metadata| metadata.modified().ok())
                .zip(metadata.as_ref().map(fs::Metadata::len));
            (path, stamp)
        })
        .collect();
    if !first.is_empty() {
        std::thread::sleep(Duration::from_millis(250));
    }
    first
        .into_iter()
        .map(|(path, first_stamp)| {
            let second_stamp = fs::metadata(&path).ok().and_then(|metadata| {
                metadata
                    .modified()
                    .ok()
                    .map(|modified| (modified, metadata.len()))
            });
            let text = (first_stamp == second_stamp)
                .then(|| fs::read_to_string(&path).ok())
                .flatten();
            (path, text)
        })
        .collect()
}

fn source_name(source: ManifestSource) -> &'static str {
    match source {
        ManifestSource::Release => "release",
        ManifestSource::User => "user",
        ManifestSource::Project => "project",
    }
}

#[cfg(windows)]
#[link(name = "Normaliz")]
unsafe extern "system" {
    fn NormalizeString(
        norm_form: i32,
        source: *const u16,
        source_length: i32,
        destination: *mut u16,
        destination_length: i32,
    ) -> i32;
}

/// Windows NormalizationKC plus Unicode lowercase.  Search metadata and the
/// query take the same path, while descriptor/arguments hashing deliberately
/// preserves original Unicode bytes.
pub fn normalize_search_text(value: &str) -> String {
    #[cfg(windows)]
    {
        const NORMALIZATION_KC: i32 = 5;
        let source: Vec<u16> = value.encode_utf16().collect();
        let source_length = i32::try_from(source.len()).unwrap_or(i32::MAX);
        let required = unsafe {
            NormalizeString(
                NORMALIZATION_KC,
                source.as_ptr(),
                source_length,
                std::ptr::null_mut(),
                0,
            )
        };
        if required > 0 {
            let mut destination = vec![0u16; required as usize];
            let written = unsafe {
                NormalizeString(
                    NORMALIZATION_KC,
                    source.as_ptr(),
                    source_length,
                    destination.as_mut_ptr(),
                    required,
                )
            };
            if written > 0 {
                destination.truncate(written as usize);
                if let Ok(normalized) = String::from_utf16(&destination) {
                    return normalized.to_lowercase();
                }
            }
        }
    }
    value.to_lowercase()
}

fn search_tokens(value: &str) -> BTreeSet<String> {
    let normalized = normalize_search_text(value);
    let mut tokens = BTreeSet::new();
    let mut current = String::new();
    for character in normalized.chars() {
        if character.is_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            tokens.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.insert(current);
    }
    tokens
}

fn manifest_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(manifest_files(&path));
        } else if path.extension().and_then(|value| value.to_str()) == Some("toml") {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[cfg(test)]
#[allow(clippy::cloned_ref_to_slice_refs)]
mod tests {
    use super::*;

    fn fixture() -> &'static str {
        include_str!("../../../specs/examples/valid/tool-package-manifest-v1.toml")
    }

    fn temp_root(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "star-control-registry-{name}-{}",
            star_ipc::nonce()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    // matrix: MCP-R006 MCP-R009 MCP-R013
    fn invalid_replacement_keeps_last_known_good_but_delete_removes_it() {
        let directory = temp_root("lkg");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let original = registry.active()["user.fake.echo"].source_hash.clone();
        fs::write(&path, "not = [valid").unwrap();
        registry.demand_scan(&[root.clone()]);
        assert_eq!(registry.active()["user.fake.echo"].source_hash, original);
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_INVALID");
        fs::remove_file(&path).unwrap();
        registry.demand_scan(&[root.clone()]);
        assert!(registry.active().contains_key("user.fake.echo"));
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_RENAMING");
        std::thread::sleep(MISSING_SOURCE_DEBOUNCE + Duration::from_millis(100));
        registry.demand_scan(&[root]);
        assert!(registry.active().is_empty());
    }

    #[test]
    // matrix: MCP-R004
    fn editor_temp_rename_never_transiently_removes_the_active_tool() {
        let directory = temp_root("editor-rename");
        let path = directory.join("fake.toml");
        let temporary = directory.join(".fake-editor.tmp");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        fs::write(
            &temporary,
            fixture().replace("Echoes a value.", "Echoes an updated value."),
        )
        .unwrap();
        fs::rename(&path, root.directory.join(".fake-old.tmp")).unwrap();
        registry.demand_scan(&[root.clone()]);
        assert!(registry.active().contains_key("user.fake.echo"));
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_RENAMING");
        fs::rename(&temporary, &path).unwrap();
        registry.demand_scan(&[root]);
        let (_, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(action.summary, "Echoes an updated value.");
    }

    #[test]
    // matrix: MCP-R011
    fn demand_scan_finds_a_change_even_when_no_watcher_event_is_delivered() {
        let directory = temp_root("watcher-missed-event");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let before = registry.snapshot_hash();
        // Do not create or poll a watcher here: this is the lost-event path.
        fs::write(
            &path,
            fixture().replace("Echoes a value.", "Echoes after a missed watcher event."),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        assert_ne!(before, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-R007
    fn invalid_new_candidate_does_not_hide_another_valid_package() {
        let directory = temp_root("invalid-new");
        let valid = directory.join("valid.toml");
        let invalid = directory.join("invalid.toml");
        fs::write(&valid, fixture()).unwrap();
        fs::write(&invalid, "format_version = [").unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        assert!(registry.active().contains_key("user.fake.echo"));
        assert_eq!(registry.diagnostics[&invalid], "TOOL_MANIFEST_INVALID");
    }

    #[test]
    // matrix: MCP-R012
    fn restart_with_invalid_source_restores_durable_last_known_good() {
        let directory = temp_root("durable-lkg");
        let path = directory.join("fake.toml");
        let cache = directory.join("registry-cache.v1.json");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let original = registry.active()["user.fake.echo"].source_hash.clone();
        registry.persist_cache(&cache).unwrap();
        fs::write(&path, "format_version = [").unwrap();
        let mut restarted = RegistryRuntime::load_cache(&cache).unwrap();
        restarted.demand_scan(&[root]);
        assert_eq!(restarted.active()["user.fake.echo"].source_hash, original);
        assert_eq!(restarted.diagnostics[&path], "TOOL_MANIFEST_INVALID");
    }

    #[test]
    // matrix: MCP-R013
    fn restart_with_deleted_source_never_revives_the_cache() {
        let directory = temp_root("durable-delete");
        let path = directory.join("fake.toml");
        let cache = directory.join("registry-cache.v1.json");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        registry.persist_cache(&cache).unwrap();
        fs::remove_file(&path).unwrap();
        let mut restarted = RegistryRuntime::load_cache(&cache).unwrap();
        restarted.demand_scan(&[root]);
        assert!(restarted.active().is_empty());
    }

    #[test]
    // matrix: MCP-R015
    fn duplicate_package_id_is_conflict_not_source_override() {
        let release = temp_root("release");
        let user = temp_root("user");
        let release_path = release.join("fake.toml");
        let user_path = user.join("fake.toml");
        fs::write(&release_path, fixture()).unwrap();
        fs::write(
            &user_path,
            fixture().replace("package_version = \"1.0.0\"", "package_version = \"1.1.0\""),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[
            RegistrySourceRoot {
                source: ManifestSource::Release,
                directory: release,
            },
            RegistrySourceRoot {
                source: ManifestSource::User,
                directory: user,
            },
        ]);
        assert!(registry.active().is_empty());
        assert_eq!(registry.diagnostics[&release_path], "TOOL_PACKAGE_CONFLICT");
        assert_eq!(registry.diagnostics[&user_path], "TOOL_PACKAGE_CONFLICT");
    }

    #[test]
    // matrix: MCP-R001
    fn project_tools_directory_is_a_distinct_live_source() {
        let directory = temp_root("project-source");
        fs::write(directory.join("fake.toml"), fixture()).unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::Project,
            directory,
        }]);
        let package = registry
            .active()
            .get("user.fake.echo")
            .expect("project manifest is live");
        assert_eq!(package.source, ManifestSource::Project);
    }

    #[test]
    // matrix: MCP-H007
    fn snapshot_hash_is_stable_for_identical_scan() {
        let directory = temp_root("hash");
        fs::write(directory.join("fake.toml"), fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let first = registry.snapshot_hash();
        registry.diagnostics.insert(
            PathBuf::from("diagnostic-only"),
            "changed wording".to_owned(),
        );
        registry.diagnostic_revision += 1;
        assert_eq!(first, registry.snapshot_hash());
        registry.demand_scan(&[root]);
        assert_eq!(first, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-R002 MCP-H005
    fn descriptor_hash_changes_with_live_manifest_candidate() {
        let directory = temp_root("descriptor");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let first = RegistryRuntime::descriptor_hash(package, action);
        fs::write(
            &path,
            fixture().replace("Contract fixture action.", "Changed fixture action."),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(first, RegistryRuntime::descriptor_hash(package, action));
    }

    #[test]
    // matrix: MCP-H015
    fn whitespace_changes_raw_candidate_hash_but_not_semantic_descriptor_or_snapshot() {
        let directory = temp_root("semantic-hash");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        let raw = package.source_hash.clone();
        fs::write(
            &path,
            fixture().replace(
                "display_name = \"Fake Echo\"",
                "display_name = \"Fake Echo\"   ",
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(raw, package.source_hash);
        assert_eq!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(snapshot, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-H002
    fn toml_key_order_and_crlf_do_not_change_semantic_hashes() {
        let directory = temp_root("toml-order");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        let reordered = fixture()
            .replacen(
                "package_id = \"user.fake.echo\"\npackage_version = \"1.0.0\"",
                "package_version = \"1.0.0\"\npackage_id = \"user.fake.echo\"",
                1,
            )
            .replace('\n', "\r\n");
        fs::write(&path, reordered).unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(snapshot, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-H003
    fn set_order_changes_do_not_change_semantic_descriptor_or_snapshot() {
        let directory = temp_root("set-hash");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        fs::write(
            &path,
            fixture().replace(
                "permission_actions = [\"local_read\", \"process_run\"]",
                "permission_actions = [\"process_run\", \"local_read\"]",
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_eq!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_eq!(snapshot, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-H004
    fn argv_binding_order_changes_the_descriptor_hash() {
        let directory = temp_root("argv-order");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        fs::write(
            &path,
            format!(
                "{}\n[[actions.argv]]\nkind = \"literal\"\nvalue = \"--after\"\n",
                fixture()
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
    }

    #[test]
    // matrix: MCP-H006
    fn executable_identity_replacement_changes_the_descriptor_hash() {
        let directory = temp_root("executable-identity");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        fs::write(
            &path,
            fixture().replace(
                "C:\\\\Tools\\\\fake-echo.exe",
                "C:\\\\Tools\\\\fake-echo-replaced.exe",
            ),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
    }

    #[test]
    // matrix: MCP-H007
    fn diagnostic_text_and_revision_do_not_change_snapshot_hash() {
        let directory = temp_root("diagnostic-snapshot");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root]);
        let snapshot = registry.snapshot_hash();
        registry
            .diagnostics
            .insert(path, "changed diagnostic wording".to_owned());
        registry.diagnostic_revision += 1;
        assert_eq!(snapshot, registry.snapshot_hash());
    }

    #[test]
    // matrix: MCP-R005
    fn changing_candidate_stays_last_known_good_until_stable() {
        let directory = temp_root("stabilizing");
        let path = directory.join("fake.toml");
        fs::write(&path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let hash = registry.active()["user.fake.echo"].source_hash.clone();
        let writer_path = path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(75));
            fs::write(writer_path, "not = [valid").unwrap();
        });
        registry.demand_scan(&[root]);
        assert_eq!(registry.active()["user.fake.echo"].source_hash, hash);
        assert_eq!(registry.diagnostics[&path], "TOOL_MANIFEST_STABILIZING");
    }

    fn search_fixture(
        package_id: &str,
        tool_id: &str,
        alias: &str,
        summary: &str,
        description: &str,
    ) -> String {
        fixture()
            .replace("user.fake.echo", package_id)
            .replace("user.fake.echo.run", tool_id)
            .replace(
                "description = \"Contract fixture action.\"",
                &format!(
                    "description = \"{description}\"\naliases = [\"{alias}\"]\ntags = [\"knowledge\"]\ntask_kinds = [\"inspect\"]"
                ),
            )
            .replace("summary = \"Echoes a value.\"", &format!("summary = \"{summary}\""))
    }

    fn package_with_actions(package_index: usize, action_count: usize) -> String {
        let package_id = format!("user.capacity.pkg{package_index:03}");
        let manifest = fixture().replace("user.fake.echo", &package_id);
        let (header, action) = manifest
            .split_once("[[actions]]")
            .expect("fixture has one action");
        let mut output = header.to_owned();
        for action_index in 0..action_count {
            output.push_str("[[actions]]");
            output.push_str(&action.replace(
                &format!("{package_id}.run"),
                &format!("{package_id}.action{action_index:03}"),
            ));
        }
        output
    }

    #[test]
    // matrix: MCP-R019 MCP-R020
    fn registry_enforces_128_package_512_action_limits_without_evicting_active_packages() {
        let directory = temp_root("capacity");
        for package_index in 0..128 {
            fs::write(
                directory.join(format!("package-{package_index:03}.toml")),
                package_with_actions(package_index, 4),
            )
            .unwrap();
        }
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory: directory.clone(),
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        assert_eq!(registry.active().len(), 128);
        assert_eq!(registry.search_actions("").len(), 512);
        let snapshot = registry.snapshot_hash();

        let excess = directory.join("00-excess.toml");
        fs::write(&excess, package_with_actions(999, 1)).unwrap();
        registry.demand_scan(&[root]);
        assert_eq!(registry.active().len(), 128);
        assert_eq!(registry.search_actions("").len(), 512);
        assert_eq!(registry.snapshot_hash(), snapshot);
        assert_eq!(registry.diagnostics[&excess], "TOOL_REGISTRY_LIMIT");
        assert!(!registry.active().contains_key("user.capacity.pkg999"));
    }

    #[test]
    // matrix: MCP-H011 MCP-H013 MCP-H014
    fn deterministic_search_uses_the_frozen_multilingual_score_and_tie_break() {
        let directory = temp_root("deterministic-search");
        fs::write(
            directory.join("alpha.toml"),
            search_fixture(
                "user.search.alpha",
                "user.search.alpha.run",
                "별찾기",
                "한국어 English document",
                "계약 설명",
            ),
        )
        .unwrap();
        fs::write(
            directory.join("beta.toml"),
            search_fixture(
                "user.search.beta",
                "user.search.beta.run",
                "second",
                "shared",
                "다른 설명",
            ),
        )
        .unwrap();
        fs::write(
            directory.join("gamma.toml"),
            search_fixture(
                "user.search.gamma",
                "user.search.gamma.run",
                "third",
                "shared",
                "또 다른 설명",
            ),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);

        let cases = [
            ("user.search.alpha.run", 1_000, "user.search.alpha.run"),
            ("별찾기", 800, "user.search.alpha.run"),
            ("knowledge", 300, "user.search.alpha.run"),
            ("한국어", 40, "user.search.alpha.run"),
            ("english", 40, "user.search.alpha.run"),
        ];
        for (query, score, tool_id) in cases {
            let hits = registry.search_actions(query);
            assert_eq!(hits[0].score, score, "query={query}");
            assert_eq!(hits[0].action.tool_id, tool_id, "query={query}");
            let repeated: Vec<_> = registry
                .search_actions(query)
                .iter()
                .map(|hit| (hit.score, hit.action.tool_id.as_str()))
                .collect();
            let first: Vec<_> = hits
                .iter()
                .map(|hit| (hit.score, hit.action.tool_id.as_str()))
                .collect();
            assert_eq!(first, repeated);
        }
        let tied = registry.search_actions("shared");
        assert_eq!(tied[0].score, 40);
        assert_eq!(tied[0].action.tool_id, "user.search.beta.run");
        assert_eq!(tied[1].action.tool_id, "user.search.gamma.run");
    }

    #[test]
    // matrix: MCP-H006 MCP-P013
    fn follow_path_executable_byte_identity_changes_descriptor_and_snapshot() {
        let directory = temp_root("follow-path-identity");
        let executable = directory.join("live.exe");
        fs::write(&executable, b"first executable identity").unwrap();
        let path = executable.display().to_string().replace('\\', "\\\\");
        let manifest = fixture()
            .replace(r"C:\\Tools\\fake-echo.exe", &path)
            .replace("update_policy = \"pinned_hash\"", "update_policy = \"follow_path\"")
            .replace(
                "sha256 = \"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"\n",
                "",
            );
        fs::write(directory.join("follow.toml"), manifest).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let descriptor = RegistryRuntime::descriptor_hash(package, action);
        let snapshot = registry.snapshot_hash();
        let revision = registry.revision;

        fs::write(&executable, b"second executable identity").unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(
            descriptor,
            RegistryRuntime::descriptor_hash(package, action)
        );
        assert_ne!(snapshot, registry.snapshot_hash());
        assert!(registry.revision > revision);
    }

    #[test]
    // matrix: MCP-P011 MCP-P012
    fn compatible_candidate_requires_probe_success_and_failure_keeps_lkg() {
        let directory = temp_root("compatible-probe-lkg");
        let manifest_path = directory.join("tool.toml");
        fs::write(&manifest_path, fixture()).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory: directory.clone(),
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(std::slice::from_ref(&root));
        let lkg_hash = RegistryRuntime::package_semantic_hash(
            registry.active().get("user.fake.echo").unwrap(),
        );

        let executable = directory.join("compatible.exe");
        fs::write(&executable, b"compatible candidate bytes").unwrap();
        let path = executable.display().to_string().replace('\\', "\\\\");
        let compatible = fixture()
            .replace(r"C:\\Tools\\fake-echo.exe", &path)
            .replace("update_policy = \"pinned_hash\"", "update_policy = \"version_compatible\"")
            .replace(
                "sha256 = \"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"\n",
                "product_version_req = \"^1\"\nauthenticode_policy = \"require_subject\"\nauthenticode_subject = \"Test Publisher\"\n",
            )
            .replace(
                "architectures = [\"x86_64\"]",
                "architectures = [\"x86_64\"]\n\n[executables.probe]\nkind = \"argv\"\nargs = [\"probe-version\"]\noutput_format = \"semver_line\"\nversion_pattern = '^(?P<product>[0-9]+\\.[0-9]+\\.[0-9]+) interface=(?P<interface>[0-9]+\\.[0-9]+\\.[0-9]+)$'",
            );
        fs::write(&manifest_path, compatible).unwrap();
        registry.demand_scan(std::slice::from_ref(&root));
        assert_eq!(
            RegistryRuntime::package_semantic_hash(
                registry.active().get("user.fake.echo").unwrap(),
            ),
            lkg_hash,
            "unprobed candidate must not replace the active package"
        );
        assert!(registry.probe_candidate("user.fake.echo").is_some());
        assert_eq!(registry.diagnostics[&manifest_path], "TOOL_PROBE_REQUIRED");

        assert!(registry.reject_compatible_probe("user.fake.echo"));
        assert_eq!(
            RegistryRuntime::package_semantic_hash(
                registry.active().get("user.fake.echo").unwrap(),
            ),
            lkg_hash
        );
        assert_eq!(
            registry.diagnostics[&manifest_path],
            "TOOL_PROBE_FAILED_LKG_RETAINED"
        );

        assert!(registry.accept_compatible_probe("user.fake.echo"));
        let active = registry.active().get("user.fake.echo").unwrap();
        assert_ne!(RegistryRuntime::package_semantic_hash(active), lkg_hash);
        assert_eq!(
            active.resolved_executable_hashes["fake-echo"],
            Sha256Hash::digest(b"compatible candidate bytes")
        );
    }

    #[test]
    // matrix: MCP-H008 MCP-S004
    fn descriptor_contains_secret_reference_identity_but_never_secret_value() {
        let directory = temp_root("secret-ref-hash");
        let path = directory.join("secret.toml");
        let with_ref = fixture().replace(
            "architectures = [\"x86_64\"]",
            "architectures = [\"x86_64\"]\n\n[[executables.environment_values]]\nname = \"STAR_SECRET\"\nsecret_ref = \"env:SECRET_A\"",
        );
        fs::write(&path, &with_ref).unwrap();
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        };
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[root.clone()]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        let first = RegistryRuntime::descriptor_hash(package, action);
        let serialized = serde_json::to_string(&normalized_manifest(&package.manifest)).unwrap();
        assert!(serialized.contains("env:SECRET_A"));
        assert!(!serialized.contains("actual-secret-value"));

        fs::write(&path, with_ref.replace("env:SECRET_A", "env:SECRET_B")).unwrap();
        registry.demand_scan(&[root]);
        let (package, action) = registry.find_action("user.fake.echo.run").unwrap();
        assert_ne!(first, RegistryRuntime::descriptor_hash(package, action));
    }

    #[test]
    // matrix: MCP-R008
    fn invalid_required_core_blocks_core_readiness_without_hiding_external_packages() {
        let release = temp_root("required-core-invalid");
        let user = temp_root("required-core-external");
        fs::write(
            release.join("star-control-core.toml"),
            "format_version = 1\npackage_id = \"star.control.core\"\nrequired = true\ninvalid = [",
        )
        .unwrap();
        fs::write(user.join("external.toml"), fixture()).unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[
            RegistrySourceRoot {
                source: ManifestSource::Release,
                directory: release,
            },
            RegistrySourceRoot {
                source: ManifestSource::User,
                directory: user,
            },
        ]);
        assert!(!registry.core_ready());
        assert!(registry.find_action("user.fake.echo.run").is_some());
        assert_eq!(registry.search_actions("user.fake.echo.run").len(), 1);
    }

    fn replacement_fixture(package_id: &str, target: Option<&str>) -> String {
        let source = search_fixture(
            package_id,
            &format!("{package_id}.run"),
            "replacement",
            "Replacement action",
            "Replacement test package",
        );
        target.map_or(source.clone(), |target| {
            source.replace(
                "backend_kinds = [\"process\"]",
                &format!(
                    "backend_kinds = [\"process\"]\nreplaces = [{{ package_id = \"{target}\", version_req = \"^1.0\" }}]"
                ),
            )
        })
    }

    #[test]
    // matrix: MCP-R016
    fn replacement_graph_accepts_one_trusted_bounded_replacer_and_rejects_conflicts() {
        let directory = temp_root("replacement-valid");
        fs::write(
            directory.join("target.toml"),
            replacement_fixture("user.replace.target", None),
        )
        .unwrap();
        fs::write(
            directory.join("replacer.toml"),
            replacement_fixture("user.replace.new", Some("user.replace.target")),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = ["user.replace.new".to_owned()].into_iter().collect();
        let resolution = registry.resolve_replacements(&trusted);
        assert_eq!(
            resolution.replaced_by.get("user.replace.target"),
            Some(&"user.replace.new".to_owned())
        );
        assert!(resolution.conflicts.is_empty());
        let visible: Vec<_> = registry
            .search_actions_with_trust("replacement", &trusted)
            .iter()
            .map(|hit| hit.package.manifest.package_id.as_str())
            .collect();
        assert_eq!(visible, vec!["user.replace.new"]);

        let directory = temp_root("replacement-multiple");
        for (name, target) in [
            ("user.multi.target", None),
            ("user.multi.first", Some("user.multi.target")),
            ("user.multi.second", Some("user.multi.target")),
        ] {
            fs::write(
                directory.join(format!("{name}.toml")),
                replacement_fixture(name, target),
            )
            .unwrap();
        }
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = registry.active.keys().cloned().collect();
        let resolution = registry.resolve_replacements(&trusted);
        assert!(resolution.replaced_by.is_empty());
        assert!(resolution.conflicts.contains("user.multi.first"));
        assert!(resolution.conflicts.contains("user.multi.second"));

        let directory = temp_root("replacement-cycle");
        fs::write(
            directory.join("a.toml"),
            replacement_fixture("user.cycle.a", Some("user.cycle.b")),
        )
        .unwrap();
        fs::write(
            directory.join("b.toml"),
            replacement_fixture("user.cycle.b", Some("user.cycle.a")),
        )
        .unwrap();
        let mut registry = RegistryRuntime::default();
        registry.demand_scan(&[RegistrySourceRoot {
            source: ManifestSource::User,
            directory,
        }]);
        let trusted: BTreeSet<_> = registry.active.keys().cloned().collect();
        let resolution = registry.resolve_replacements(&trusted);
        assert!(resolution.replaced_by.is_empty());
        assert_eq!(resolution.conflicts.len(), 2);
    }

    fn bounded_registry_fixture(index: usize) -> String {
        let package_id = format!("user.limit.p{index:03}");
        let source = fixture().replace("user.fake.echo", &package_id);
        let (prefix, action) = source.split_once("[[actions]]").unwrap();
        let mut result = prefix.to_owned();
        for action_index in 0..4 {
            result.push_str("[[actions]]");
            result.push_str(&action.replace(
                &format!("tool_id = \"{package_id}.run\""),
                &format!("tool_id = \"{package_id}.action{action_index}\""),
            ));
        }
        result
    }

    #[test]
    // matrix: MCP-R019 MCP-R020
    fn registry_caps_128_packages_and_512_actions_without_evicting_active_entries() {
        let directory = temp_root("registry-capacity");
        for index in 0..128 {
            fs::write(
                directory.join(format!("p{index:03}.toml")),
                bounded_registry_fixture(index),
            )
            .unwrap();
        }
        let root = RegistrySourceRoot {
            source: ManifestSource::User,
            directory: directory.clone(),
        };
        let mut registry = RegistryRuntime::default();
        let started = std::time::Instant::now();
        registry.demand_scan(&[root.clone()]);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(registry.active().len(), 128);
        assert_eq!(
            registry
                .active()
                .values()
                .map(|package| package.manifest.actions.len())
                .sum::<usize>(),
            512
        );
        let first: Vec<_> = registry
            .search_actions("")
            .iter()
            .map(|hit| hit.action.tool_id.clone())
            .collect();
        let second: Vec<_> = registry
            .search_actions("")
            .iter()
            .map(|hit| hit.action.tool_id.clone())
            .collect();
        assert_eq!(first, second);
        assert_eq!(first.len(), 512);

        fs::write(
            directory.join("overflow.toml"),
            replacement_fixture("user.limit.overflow", None),
        )
        .unwrap();
        registry.demand_scan(&[root]);
        assert_eq!(registry.active().len(), 128);
        assert!(!registry.active().contains_key("user.limit.overflow"));
        assert!(
            registry
                .diagnostics
                .values()
                .any(|diagnostic| diagnostic == "TOOL_REGISTRY_LIMIT")
        );
    }
}
