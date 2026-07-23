//! Exact GitHub Releases publisher for `ReleaseManifestV2`.
//!
//! The adapter creates a draft first, uploads only manifest-bound assets,
//! publishes once, and then proves the remote bytes by downloading every
//! release asset. A timeout never triggers a second write; the release engine
//! calls `reconcile`, which is read-only.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use star_contracts::{
    Sha256Hash,
    release_v2::{ReleaseAssetBindingV1, ReleaseManifestV2},
};
use star_domain::versioned_fingerprint;
use star_release::{
    candidate::{PublishObservation, PublisherAdapter},
    publisher::verify_release_asset_binding,
};
use thiserror::Error;

const MAX_GH_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_GH_STDERR_BYTES: u64 = 1024 * 1024;
const MAX_RELEASE_ASSET_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct GhCliConfig {
    pub executable: PathBuf,
    pub executable_sha256: Sha256Hash,
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub digest: Option<Sha256Hash>,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRelease {
    pub id: u64,
    pub tag_name: String,
    pub target_commitish: String,
    pub draft: bool,
    pub prerelease: bool,
    pub html_url: String,
    pub updated_at: String,
    pub assets: Vec<RemoteReleaseAsset>,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum GitHubClientError {
    #[error("GitHub CLI is unavailable or changed identity")]
    Unavailable,
    #[error("GitHub request timed out")]
    Timeout,
    #[error("GitHub returned invalid or excessive output")]
    InvalidOutput,
    #[error("GitHub rejected the request")]
    Rejected,
}

pub trait GitHubReleaseClient {
    fn inspect_release(
        &mut self,
        repository: &str,
        tag: &str,
    ) -> Result<Option<RemoteRelease>, GitHubClientError>;

    fn create_draft(
        &mut self,
        repository: &str,
        tag: &str,
        target_commitish: &str,
        title: &str,
        notes: &Path,
        prerelease: bool,
    ) -> Result<RemoteRelease, GitHubClientError>;

    fn upload_asset(
        &mut self,
        repository: &str,
        tag: &str,
        source: &Path,
        remote_name: &str,
    ) -> Result<(), GitHubClientError>;

    fn publish_release(
        &mut self,
        repository: &str,
        release_id: u64,
        prerelease: bool,
    ) -> Result<(), GitHubClientError>;

    fn download_asset(
        &mut self,
        repository: &str,
        asset_id: u64,
        destination: &Path,
    ) -> Result<(), GitHubClientError>;
}

pub struct GitHubReleasePublisher<C> {
    client: C,
    project_root: PathBuf,
    evidence_root: PathBuf,
    binding: ReleaseAssetBindingV1,
}

impl<C> GitHubReleasePublisher<C> {
    pub fn new(
        client: C,
        project_root: PathBuf,
        evidence_root: PathBuf,
        binding: ReleaseAssetBindingV1,
    ) -> Result<Self, GitHubClientError> {
        let project_root =
            fs::canonicalize(project_root).map_err(|_| GitHubClientError::Unavailable)?;
        fs::create_dir_all(&evidence_root).map_err(|_| GitHubClientError::Unavailable)?;
        let evidence_root =
            fs::canonicalize(evidence_root).map_err(|_| GitHubClientError::Unavailable)?;
        Ok(Self {
            client,
            project_root,
            evidence_root,
            binding,
        })
    }

    pub fn into_client(self) -> C {
        self.client
    }
}

impl<C: GitHubReleaseClient> PublisherAdapter for GitHubReleasePublisher<C> {
    fn publish(&mut self, manifest: &ReleaseManifestV2) -> PublishObservation {
        if verify_release_asset_binding(manifest, &self.binding).is_err()
            || self.verify_local_assets().is_err()
        {
            return PublishObservation::Failed;
        }
        match self.publish_inner(manifest) {
            Ok(observation) => observation,
            Err(PublishFailure::BeforeEffect) => PublishObservation::Failed,
            Err(PublishFailure::Timeout) => PublishObservation::Timeout,
            Err(PublishFailure::Partial(reason)) => PublishObservation::Partial {
                receipt_ref: self.write_receipt("partial", &reason),
            },
        }
    }

    fn reconcile(&mut self, manifest: &ReleaseManifestV2) -> PublishObservation {
        if verify_release_asset_binding(manifest, &self.binding).is_err()
            || self.verify_local_assets().is_err()
        {
            return PublishObservation::Failed;
        }
        let remote = match self
            .client
            .inspect_release(&self.binding.repository, &self.binding.tag)
        {
            Ok(Some(remote)) => remote,
            Ok(None) | Err(GitHubClientError::Timeout) => return PublishObservation::Timeout,
            Err(_) => return PublishObservation::Failed,
        };
        if remote.draft {
            return PublishObservation::Partial {
                receipt_ref: self.write_receipt("draft_after_timeout", &remote.html_url),
            };
        }
        self.verify_remote_release(manifest, &remote)
    }
}

#[derive(Debug)]
enum PublishFailure {
    BeforeEffect,
    Timeout,
    Partial(String),
}

impl<C: GitHubReleaseClient> GitHubReleasePublisher<C> {
    fn publish_inner(
        &mut self,
        manifest: &ReleaseManifestV2,
    ) -> Result<PublishObservation, PublishFailure> {
        let mut remote = match self
            .client
            .inspect_release(&self.binding.repository, &self.binding.tag)
        {
            Ok(Some(remote)) => remote,
            Ok(None) => {
                let notes = self
                    .resolve_project_file(&self.binding.notes_relative_path)
                    .map_err(|_| PublishFailure::BeforeEffect)?;
                match self.client.create_draft(
                    &self.binding.repository,
                    &self.binding.tag,
                    &self.binding.target_commitish,
                    &self.binding.title,
                    &notes,
                    self.binding.prerelease,
                ) {
                    Ok(remote) => remote,
                    Err(GitHubClientError::Timeout | GitHubClientError::InvalidOutput) => {
                        return Err(PublishFailure::Timeout);
                    }
                    Err(_) => return Err(PublishFailure::BeforeEffect),
                }
            }
            Err(GitHubClientError::Timeout) => return Err(PublishFailure::Timeout),
            Err(_) => return Err(PublishFailure::BeforeEffect),
        };

        if !remote.draft {
            return Ok(self.verify_remote_release(manifest, &remote));
        }
        self.validate_remote_identity(&remote)
            .map_err(|reason| PublishFailure::Partial(reason.to_owned()))?;
        let remote_assets = remote
            .assets
            .iter()
            .map(|asset| (asset.name.as_str(), asset))
            .collect::<BTreeMap<_, _>>();
        for asset in &self.binding.assets {
            if let Some(existing) = remote_assets.get(asset.remote_name.as_str()) {
                if existing.size != asset.size
                    || existing.state != "uploaded"
                    || existing
                        .digest
                        .as_ref()
                        .is_some_and(|digest| digest != &asset.sha256)
                {
                    return Err(PublishFailure::Partial(format!(
                        "remote_asset_conflict:{}",
                        asset.remote_name
                    )));
                }
                continue;
            }
            let source = self
                .resolve_project_file(&asset.relative_path)
                .map_err(|_| PublishFailure::Partial("local_asset_unavailable".to_owned()))?;
            match self.client.upload_asset(
                &self.binding.repository,
                &self.binding.tag,
                &source,
                &asset.remote_name,
            ) {
                Ok(()) => {}
                Err(GitHubClientError::Timeout) => return Err(PublishFailure::Timeout),
                Err(_) => {
                    return Err(PublishFailure::Partial(format!(
                        "asset_upload_failed:{}",
                        asset.remote_name
                    )));
                }
            }
        }
        remote = match self
            .client
            .inspect_release(&self.binding.repository, &self.binding.tag)
        {
            Ok(Some(remote)) => remote,
            Err(GitHubClientError::Timeout) => return Err(PublishFailure::Timeout),
            _ => return Err(PublishFailure::Partial("draft_readback_failed".to_owned())),
        };
        self.verify_remote_assets_only(&remote)
            .map_err(PublishFailure::Partial)?;
        match self.client.publish_release(
            &self.binding.repository,
            remote.id,
            self.binding.prerelease,
        ) {
            Ok(()) => {}
            Err(GitHubClientError::Timeout) => return Err(PublishFailure::Timeout),
            Err(_) => return Err(PublishFailure::Partial("publish_failed".to_owned())),
        }
        remote = match self
            .client
            .inspect_release(&self.binding.repository, &self.binding.tag)
        {
            Ok(Some(remote)) => remote,
            Err(GitHubClientError::Timeout) => return Err(PublishFailure::Timeout),
            _ => {
                return Err(PublishFailure::Partial(
                    "published_readback_failed".to_owned(),
                ));
            }
        };
        Ok(self.verify_remote_release(manifest, &remote))
    }

    fn verify_local_assets(&self) -> Result<(), GitHubClientError> {
        for asset in &self.binding.assets {
            let path = self.resolve_project_file(&asset.relative_path)?;
            let metadata = fs::metadata(&path).map_err(|_| GitHubClientError::Unavailable)?;
            if !metadata.is_file()
                || metadata.len() != asset.size
                || hash_file(&path, MAX_RELEASE_ASSET_BYTES)? != asset.sha256
            {
                return Err(GitHubClientError::Rejected);
            }
        }
        Ok(())
    }

    fn verify_remote_release(
        &mut self,
        manifest: &ReleaseManifestV2,
        remote: &RemoteRelease,
    ) -> PublishObservation {
        if remote.draft
            || self.validate_remote_identity(remote).is_err()
            || self.verify_remote_assets_only(remote).is_err()
        {
            return PublishObservation::Partial {
                receipt_ref: self.write_receipt("remote_mismatch", &remote.html_url),
            };
        }
        let snapshot_ref = match self.write_snapshot(remote) {
            Ok(path) => path,
            Err(_) => return PublishObservation::Failed,
        };
        PublishObservation::Verified {
            artifact_set_digest: manifest
                .artifact_set_digest
                .clone()
                .expect("binding verification required an artifact set digest"),
            snapshot_ref,
        }
    }

    fn validate_remote_identity(&self, remote: &RemoteRelease) -> Result<(), &'static str> {
        if remote.tag_name != self.binding.tag
            || remote.target_commitish != self.binding.target_commitish
            || remote.prerelease != self.binding.prerelease
        {
            Err("remote_release_identity_conflict")
        } else {
            Ok(())
        }
    }

    fn verify_remote_assets_only(&mut self, remote: &RemoteRelease) -> Result<(), String> {
        self.validate_remote_identity(remote)
            .map_err(str::to_owned)?;
        let expected_names = self
            .binding
            .assets
            .iter()
            .map(|asset| asset.remote_name.as_str())
            .collect::<BTreeSet<_>>();
        let observed_names = remote
            .assets
            .iter()
            .map(|asset| asset.name.as_str())
            .collect::<BTreeSet<_>>();
        if expected_names != observed_names || observed_names.len() != remote.assets.len() {
            return Err("remote_asset_set_conflict".to_owned());
        }
        for expected in &self.binding.assets {
            let remote_asset = remote
                .assets
                .iter()
                .find(|asset| asset.name == expected.remote_name)
                .ok_or_else(|| "remote_asset_missing".to_owned())?;
            if remote_asset.size != expected.size || remote_asset.state != "uploaded" {
                return Err(format!(
                    "remote_asset_metadata_conflict:{}",
                    expected.remote_name
                ));
            }
            if remote_asset
                .digest
                .as_ref()
                .is_some_and(|digest| digest != &expected.sha256)
            {
                return Err(format!(
                    "remote_asset_digest_conflict:{}",
                    expected.remote_name
                ));
            }
            let destination = self.evidence_root.join(format!(
                "remote-{}-{}-{}.part",
                remote.id,
                remote_asset.id,
                nonce()
            ));
            self.client
                .download_asset(&self.binding.repository, remote_asset.id, &destination)
                .map_err(|error| format!("remote_asset_download:{error}"))?;
            let digest = hash_file(&destination, MAX_RELEASE_ASSET_BYTES)
                .map_err(|_| "remote_asset_hash_failed".to_owned())?;
            let _ = fs::remove_file(&destination);
            if digest != expected.sha256 {
                return Err(format!(
                    "remote_asset_digest_conflict:{}",
                    expected.remote_name
                ));
            }
        }
        Ok(())
    }

    fn resolve_project_file(&self, relative: &str) -> Result<PathBuf, GitHubClientError> {
        let candidate = fs::canonicalize(self.project_root.join(relative))
            .map_err(|_| GitHubClientError::Unavailable)?;
        if !candidate.starts_with(&self.project_root) {
            return Err(GitHubClientError::Rejected);
        }
        Ok(candidate)
    }

    fn write_snapshot(&self, remote: &RemoteRelease) -> Result<String, GitHubClientError> {
        let value = serde_json::json!({
            "schema_id":"star.github-release-remote-snapshot",
            "schema_version":1,
            "repository":self.binding.repository,
            "release":remote,
            "artifact_set_digest":self.binding.artifact_set_digest,
            "asset_binding_fingerprint":self.binding.binding_fingerprint,
        });
        let fingerprint = versioned_fingerprint("star.github-release-remote-snapshot", 1, &value)
            .map_err(|_| GitHubClientError::InvalidOutput)?;
        let path = self.evidence_root.join(format!(
            "github-release-{}.json",
            fingerprint.as_str().trim_start_matches("sha256:")
        ));
        write_idempotent_json(&path, &value)?;
        Ok(path.to_string_lossy().into_owned())
    }

    fn write_receipt(&self, state: &str, detail: &str) -> String {
        let value = serde_json::json!({
            "schema_id":"star.github-release-publish-receipt",
            "schema_version":1,
            "release_manifest_id":self.binding.release_manifest_id,
            "asset_binding_fingerprint":self.binding.binding_fingerprint,
            "state":state,
            "detail":detail,
        });
        let fingerprint = versioned_fingerprint("star.github-release-publish-receipt", 1, &value)
            .unwrap_or_else(|_| Sha256Hash::digest(detail.as_bytes()));
        let path = self.evidence_root.join(format!(
            "github-receipt-{}.json",
            fingerprint.as_str().trim_start_matches("sha256:")
        ));
        let _ = write_idempotent_json(&path, &value);
        path.to_string_lossy().into_owned()
    }
}

pub struct GhCliClient {
    config: GhCliConfig,
}

impl GhCliClient {
    pub fn new(config: GhCliConfig) -> Result<Self, GitHubClientError> {
        if config.timeout < Duration::from_secs(1)
            || config.timeout > Duration::from_secs(600)
            || !config.executable.is_absolute()
            || hash_file(&config.executable, 128 * 1024 * 1024)? != config.executable_sha256
        {
            return Err(GitHubClientError::Unavailable);
        }
        Ok(Self { config })
    }

    fn run_json(&self, args: &[String]) -> Result<serde_json::Value, GitHubClientError> {
        let outcome = self.run(args, MAX_GH_JSON_BYTES)?;
        if !outcome.status.success() {
            return Err(GitHubClientError::Rejected);
        }
        serde_json::from_slice(&outcome.stdout).map_err(|_| GitHubClientError::InvalidOutput)
    }

    fn run(&self, args: &[String], max_stdout: u64) -> Result<CommandOutcome, GitHubClientError> {
        if hash_file(&self.config.executable, 128 * 1024 * 1024)? != self.config.executable_sha256 {
            return Err(GitHubClientError::Unavailable);
        }
        run_bounded_command(
            &self.config.executable,
            args,
            self.config.timeout,
            max_stdout,
        )
    }

    fn run_to_file(
        &self,
        args: &[String],
        destination: &Path,
        max_stdout: u64,
    ) -> Result<(ExitStatus, Vec<u8>), GitHubClientError> {
        if hash_file(&self.config.executable, 128 * 1024 * 1024)? != self.config.executable_sha256 {
            return Err(GitHubClientError::Unavailable);
        }
        run_bounded_command_to_file(
            &self.config.executable,
            args,
            self.config.timeout,
            destination,
            max_stdout,
        )
    }
}

impl GitHubReleaseClient for GhCliClient {
    fn inspect_release(
        &mut self,
        repository: &str,
        tag: &str,
    ) -> Result<Option<RemoteRelease>, GitHubClientError> {
        let endpoint = format!("repos/{repository}/releases/tags/{tag}");
        let outcome = self.run(&["api".to_owned(), endpoint], MAX_GH_JSON_BYTES)?;
        if !outcome.status.success() {
            return if is_not_found(&outcome.stderr) {
                Ok(None)
            } else {
                Err(GitHubClientError::Rejected)
            };
        }
        let value: serde_json::Value = serde_json::from_slice(&outcome.stdout)
            .map_err(|_| GitHubClientError::InvalidOutput)?;
        parse_remote_release(value).map(Some)
    }

    fn create_draft(
        &mut self,
        repository: &str,
        tag: &str,
        target_commitish: &str,
        title: &str,
        notes: &Path,
        prerelease: bool,
    ) -> Result<RemoteRelease, GitHubClientError> {
        let mut args = vec![
            "release".to_owned(),
            "create".to_owned(),
            tag.to_owned(),
            "--repo".to_owned(),
            repository.to_owned(),
            "--target".to_owned(),
            target_commitish.to_owned(),
            "--title".to_owned(),
            title.to_owned(),
            "--notes-file".to_owned(),
            notes.to_string_lossy().into_owned(),
            "--draft".to_owned(),
        ];
        if prerelease {
            args.push("--prerelease".to_owned());
        }
        let outcome = self.run(&args, MAX_GH_JSON_BYTES)?;
        if !outcome.status.success() {
            return Err(GitHubClientError::Rejected);
        }
        self.inspect_release(repository, tag)?
            .ok_or(GitHubClientError::InvalidOutput)
    }

    fn upload_asset(
        &mut self,
        repository: &str,
        tag: &str,
        source: &Path,
        remote_name: &str,
    ) -> Result<(), GitHubClientError> {
        let prepared = prepare_upload_source(source, remote_name)?;
        let outcome = self.run(
            &[
                "release".to_owned(),
                "upload".to_owned(),
                tag.to_owned(),
                prepared.path.to_string_lossy().into_owned(),
                "--repo".to_owned(),
                repository.to_owned(),
            ],
            MAX_GH_JSON_BYTES,
        )?;
        if outcome.status.success() {
            Ok(())
        } else {
            Err(GitHubClientError::Rejected)
        }
    }

    fn publish_release(
        &mut self,
        repository: &str,
        release_id: u64,
        prerelease: bool,
    ) -> Result<(), GitHubClientError> {
        let endpoint = format!("repos/{repository}/releases/{release_id}");
        let args = vec![
            "api".to_owned(),
            "--method".to_owned(),
            "PATCH".to_owned(),
            endpoint,
            "-F".to_owned(),
            "draft=false".to_owned(),
            "-F".to_owned(),
            format!("prerelease={prerelease}"),
        ];
        self.run_json(&args).map(|_| ())
    }

    fn download_asset(
        &mut self,
        repository: &str,
        asset_id: u64,
        destination: &Path,
    ) -> Result<(), GitHubClientError> {
        if destination.exists() {
            return Err(GitHubClientError::Rejected);
        }
        let endpoint = format!("repos/{repository}/releases/assets/{asset_id}");
        let (status, _stderr) = self.run_to_file(
            &[
                "api".to_owned(),
                "-H".to_owned(),
                "Accept: application/octet-stream".to_owned(),
                endpoint,
            ],
            destination,
            MAX_RELEASE_ASSET_BYTES,
        )?;
        if !status.success() {
            let _ = fs::remove_file(destination);
            return Err(GitHubClientError::Rejected);
        }
        File::open(destination)
            .and_then(|file| file.sync_all())
            .map_err(|_| GitHubClientError::Unavailable)
    }
}

#[derive(Debug)]
struct CommandOutcome {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Debug)]
struct PreparedUploadSource {
    path: PathBuf,
    directory: PathBuf,
}

impl Drop for PreparedUploadSource {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

fn prepare_upload_source(
    source: &Path,
    remote_name: &str,
) -> Result<PreparedUploadSource, GitHubClientError> {
    let remote_path = Path::new(remote_name);
    if remote_name.is_empty()
        || remote_name.len() > 255
        || remote_path.components().count() != 1
        || remote_path.file_name().and_then(|name| name.to_str()) != Some(remote_name)
    {
        return Err(GitHubClientError::Rejected);
    }
    let source_metadata = fs::metadata(source).map_err(|_| GitHubClientError::Unavailable)?;
    if !source_metadata.is_file() || source_metadata.len() > MAX_RELEASE_ASSET_BYTES {
        return Err(GitHubClientError::Unavailable);
    }
    let directory = std::env::temp_dir().join(format!("star-gh-upload-{}", nonce()));
    fs::create_dir(&directory).map_err(|_| GitHubClientError::Unavailable)?;
    let path = directory.join(remote_name);
    let copied = match fs::copy(source, &path) {
        Ok(copied) => copied,
        Err(_) => {
            let _ = fs::remove_dir(&directory);
            return Err(GitHubClientError::Unavailable);
        }
    };
    if copied != source_metadata.len()
        || OpenOptions::new()
            .write(true)
            .open(&path)
            .and_then(|file| file.sync_all())
            .is_err()
    {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
        return Err(GitHubClientError::Unavailable);
    }
    Ok(PreparedUploadSource { path, directory })
}

fn run_bounded_command(
    executable: &Path,
    args: &[String],
    timeout: Duration,
    max_stdout: u64,
) -> Result<CommandOutcome, GitHubClientError> {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let stdout_path = std::env::temp_dir().join(format!("star-gh-{suffix}.stdout"));
    let stderr_path = std::env::temp_dir().join(format!("star-gh-{suffix}.stderr"));
    let stdout_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stdout_path)
        .map_err(|_| GitHubClientError::Unavailable)?;
    let stderr_file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stderr_path)
    {
        Ok(file) => file,
        Err(_) => {
            let _ = fs::remove_file(&stdout_path);
            return Err(GitHubClientError::Unavailable);
        }
    };
    let mut child = match Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            let _ = fs::remove_file(&stdout_path);
            let _ = fs::remove_file(&stderr_path);
            return Err(GitHubClientError::Unavailable);
        }
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&stdout_path);
                let _ = fs::remove_file(&stderr_path);
                return Err(GitHubClientError::Timeout);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&stdout_path);
                let _ = fs::remove_file(&stderr_path);
                return Err(GitHubClientError::Unavailable);
            }
        }
    };
    let stdout = read_bounded_file(&stdout_path, max_stdout);
    let stderr = read_bounded_file(&stderr_path, MAX_GH_STDERR_BYTES);
    let _ = fs::remove_file(&stdout_path);
    let _ = fs::remove_file(&stderr_path);
    Ok(CommandOutcome {
        status,
        stdout: stdout?,
        stderr: stderr?,
    })
}

fn run_bounded_command_to_file(
    executable: &Path,
    args: &[String],
    timeout: Duration,
    destination: &Path,
    max_stdout: u64,
) -> Result<(ExitStatus, Vec<u8>), GitHubClientError> {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    if destination.exists() {
        return Err(GitHubClientError::Rejected);
    }
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let stderr_path = std::env::temp_dir().join(format!("star-gh-{suffix}.stderr"));
    let stdout_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| GitHubClientError::Unavailable)?;
    let stderr_file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stderr_path)
    {
        Ok(file) => file,
        Err(_) => {
            let _ = fs::remove_file(destination);
            return Err(GitHubClientError::Unavailable);
        }
    };
    let mut child = match Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            let _ = fs::remove_file(destination);
            let _ = fs::remove_file(&stderr_path);
            return Err(GitHubClientError::Unavailable);
        }
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(destination);
                let _ = fs::remove_file(&stderr_path);
                return Err(GitHubClientError::Timeout);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(destination);
                let _ = fs::remove_file(&stderr_path);
                return Err(GitHubClientError::Unavailable);
            }
        }
    };
    let output_size = fs::metadata(destination)
        .map(|metadata| metadata.len())
        .map_err(|_| GitHubClientError::Unavailable);
    let stderr = read_bounded_file(&stderr_path, MAX_GH_STDERR_BYTES);
    let _ = fs::remove_file(&stderr_path);
    match output_size {
        Ok(size) if size <= max_stdout => {}
        Ok(_) => {
            let _ = fs::remove_file(destination);
            return Err(GitHubClientError::InvalidOutput);
        }
        Err(error) => {
            let _ = fs::remove_file(destination);
            return Err(error);
        }
    }
    match stderr {
        Ok(stderr) => Ok((status, stderr)),
        Err(error) => {
            let _ = fs::remove_file(destination);
            Err(error)
        }
    }
}

fn read_bounded_file(path: &Path, maximum: u64) -> Result<Vec<u8>, GitHubClientError> {
    let metadata = fs::metadata(path).map_err(|_| GitHubClientError::Unavailable)?;
    if metadata.len() > maximum {
        return Err(GitHubClientError::InvalidOutput);
    }
    fs::read(path).map_err(|_| GitHubClientError::Unavailable)
}

fn hash_file(path: &Path, maximum: u64) -> Result<Sha256Hash, GitHubClientError> {
    let metadata = fs::metadata(path).map_err(|_| GitHubClientError::Unavailable)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(GitHubClientError::Unavailable);
    }
    let file = File::open(path).map_err(|_| GitHubClientError::Unavailable)?;
    Sha256Hash::digest_reader(file).map_err(|_| GitHubClientError::Unavailable)
}

fn nonce() -> String {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn parse_remote_release(value: serde_json::Value) -> Result<RemoteRelease, GitHubClientError> {
    #[derive(Deserialize)]
    struct ApiAsset {
        id: u64,
        name: String,
        size: u64,
        digest: Option<String>,
        state: String,
    }
    #[derive(Deserialize)]
    struct ApiRelease {
        id: u64,
        tag_name: String,
        target_commitish: String,
        draft: bool,
        prerelease: bool,
        html_url: String,
        updated_at: String,
        assets: Vec<ApiAsset>,
    }
    let parsed: ApiRelease =
        serde_json::from_value(value).map_err(|_| GitHubClientError::InvalidOutput)?;
    let assets = parsed
        .assets
        .into_iter()
        .map(|asset| {
            let digest = asset
                .digest
                .map(|digest| digest.parse::<Sha256Hash>())
                .transpose()
                .map_err(|_| GitHubClientError::InvalidOutput)?;
            Ok(RemoteReleaseAsset {
                id: asset.id,
                name: asset.name,
                size: asset.size,
                digest,
                state: asset.state,
            })
        })
        .collect::<Result<Vec<_>, GitHubClientError>>()?;
    Ok(RemoteRelease {
        id: parsed.id,
        tag_name: parsed.tag_name,
        target_commitish: parsed.target_commitish,
        draft: parsed.draft,
        prerelease: parsed.prerelease,
        html_url: parsed.html_url,
        updated_at: parsed.updated_at,
        assets,
    })
}

fn is_not_found(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    stderr.contains("http 404") || stderr.contains("not found")
}

fn write_idempotent_json(path: &Path, value: &serde_json::Value) -> Result<(), GitHubClientError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|_| GitHubClientError::InvalidOutput)?;
    bytes.push(b'\n');
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => std::io::Write::write_all(&mut file, &bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| GitHubClientError::Unavailable),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path).map_err(|_| GitHubClientError::Unavailable)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(GitHubClientError::Rejected)
            }
        }
        Err(_) => Err(GitHubClientError::Unavailable),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use star_contracts::{
        ProjectId, ReleaseManifestId, ScopeRevisionId, TaskSpecId,
        release_v2::{
            RELEASE_ASSET_BINDING_V1_SCHEMA_ID, RELEASE_MANIFEST_V2_SCHEMA_ID, ReleaseArchitecture,
            ReleaseArtifactV2, ReleaseAssetSourceV1, ReleaseIdentityBinding, ReleaseStatus,
        },
    };
    use star_release::publisher::seal_release_asset_binding;

    use super::*;

    #[derive(Default)]
    struct FakeClient {
        release: Option<RemoteRelease>,
        downloads: BTreeMap<u64, Vec<u8>>,
        failures: VecDeque<GitHubClientError>,
        creates: usize,
        uploads: usize,
        publishes: usize,
    }

    impl FakeClient {
        fn fail(&mut self) -> Result<(), GitHubClientError> {
            match self.failures.pop_front() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }

    impl GitHubReleaseClient for FakeClient {
        fn inspect_release(
            &mut self,
            _repository: &str,
            _tag: &str,
        ) -> Result<Option<RemoteRelease>, GitHubClientError> {
            self.fail()?;
            Ok(self.release.clone())
        }

        fn create_draft(
            &mut self,
            _repository: &str,
            tag: &str,
            target_commitish: &str,
            _title: &str,
            _notes: &Path,
            prerelease: bool,
        ) -> Result<RemoteRelease, GitHubClientError> {
            self.fail()?;
            self.creates += 1;
            let release = RemoteRelease {
                id: 7,
                tag_name: tag.to_owned(),
                target_commitish: target_commitish.to_owned(),
                draft: true,
                prerelease,
                html_url: "https://example.invalid/draft/7".to_owned(),
                updated_at: "2026-07-23T00:00:00Z".to_owned(),
                assets: vec![],
            };
            self.release = Some(release.clone());
            Ok(release)
        }

        fn upload_asset(
            &mut self,
            _repository: &str,
            _tag: &str,
            source: &Path,
            remote_name: &str,
        ) -> Result<(), GitHubClientError> {
            self.fail()?;
            self.uploads += 1;
            let bytes = fs::read(source).unwrap();
            let id = 100 + self.uploads as u64;
            self.downloads.insert(id, bytes.clone());
            self.release
                .as_mut()
                .unwrap()
                .assets
                .push(RemoteReleaseAsset {
                    id,
                    name: remote_name.to_owned(),
                    size: bytes.len() as u64,
                    digest: Some(Sha256Hash::digest(&bytes)),
                    state: "uploaded".to_owned(),
                });
            Ok(())
        }

        fn publish_release(
            &mut self,
            _repository: &str,
            _release_id: u64,
            _prerelease: bool,
        ) -> Result<(), GitHubClientError> {
            self.fail()?;
            self.publishes += 1;
            self.release.as_mut().unwrap().draft = false;
            Ok(())
        }

        fn download_asset(
            &mut self,
            _repository: &str,
            asset_id: u64,
            destination: &Path,
        ) -> Result<(), GitHubClientError> {
            self.fail()?;
            fs::write(destination, self.downloads.get(&asset_id).unwrap())
                .map_err(|_| GitHubClientError::Unavailable)
        }
    }

    struct Fixture {
        root: PathBuf,
        evidence: PathBuf,
        manifest: ReleaseManifestV2,
        binding: ReleaseAssetBindingV1,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "star-github-adapter-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("asset.bin"), b"asset").unwrap();
        fs::write(root.join("CHANGELOG.md"), b"notes").unwrap();
        let manifest = ReleaseManifestV2 {
            schema_id: RELEASE_MANIFEST_V2_SCHEMA_ID.to_owned(),
            schema_version: 2,
            release_manifest_id: ReleaseManifestId::new(),
            revision: 1,
            supersedes: None,
            product_id: "star-control".to_owned(),
            version: "0.1.0".to_owned(),
            channel: "github_releases".to_owned(),
            task_spec_ref: TaskSpecId::new(),
            scope_revision_ref: ScopeRevisionId::new(),
            source_revisions: vec![],
            identity_binding: ReleaseIdentityBinding {
                config_fingerprint: Sha256Hash::digest(b"config"),
                catalog_fingerprint: Sha256Hash::digest(b"catalog"),
                tool_descriptor_fingerprints: vec![],
                profile_fingerprint: Sha256Hash::digest(b"profile"),
                environment_fingerprints: vec![],
            },
            verification_layers: vec![],
            build_invocation_refs: vec![],
            artifacts: vec![ReleaseArtifactV2 {
                logical_name: "asset".to_owned(),
                role: "package".to_owned(),
                architecture: ReleaseArchitecture::X64,
                size: 5,
                media_type: "application/octet-stream".to_owned(),
                sha256: Sha256Hash::digest(b"asset"),
            }],
            artifact_set_digest: Some(Sha256Hash::digest(b"set")),
            included_files_manifest_ref: None,
            metadata_refs: vec![],
            supply_chain_applicability: vec![],
            sbom_ref: None,
            provenance_ref: None,
            signature_refs: vec![],
            compatibility: vec![],
            validation_refs: vec![],
            release_gate_refs: vec![],
            remote_actions: vec![],
            approval_request_refs: vec![],
            remote_operation_refs: vec![],
            before_remote_snapshot_refs: vec![],
            after_remote_snapshot_refs: vec![],
            rollback_plan_ref: "rollback".to_owned(),
            rollback_artifact_ref: None,
            user_data_policy: "preserve".to_owned(),
            remaining_risks: vec![],
            external_gates: vec![],
            status: ReleaseStatus::Approved,
            manifest_fingerprint: Sha256Hash::digest(b"manifest"),
        };
        let binding = seal_release_asset_binding(
            &manifest,
            ProjectId::new(),
            vec![ReleaseAssetSourceV1 {
                logical_name: "asset".to_owned(),
                remote_name: "asset.bin".to_owned(),
                role: "package".to_owned(),
                architecture: ReleaseArchitecture::X64,
                media_type: "application/octet-stream".to_owned(),
                relative_path: "asset.bin".to_owned(),
                size: 5,
                sha256: Sha256Hash::digest(b"asset"),
            }],
            "a".repeat(40),
            "CHANGELOG.md".to_owned(),
        )
        .unwrap();
        assert_eq!(binding.schema_id, RELEASE_ASSET_BINDING_V1_SCHEMA_ID);
        Fixture {
            evidence: root.join("evidence"),
            root,
            manifest,
            binding,
        }
    }

    #[test]
    fn gh_upload_snapshot_uses_the_exact_remote_name_and_rejects_path_components() {
        let fixture = fixture();
        let prepared = prepare_upload_source(&fixture.root.join("asset.bin"), "renamed.bin")
            .expect("bounded upload snapshot");
        assert_eq!(prepared.path.file_name().unwrap(), "renamed.bin");
        assert_eq!(fs::read(&prepared.path).unwrap(), b"asset");
        assert_eq!(
            prepare_upload_source(&fixture.root.join("asset.bin"), "nested/asset.bin").unwrap_err(),
            GitHubClientError::Rejected
        );
    }

    #[test]
    fn creates_draft_uploads_once_publishes_and_readbacks_exact_assets() {
        let fixture = fixture();
        let client = FakeClient::default();
        let mut publisher = GitHubReleasePublisher::new(
            client,
            fixture.root.clone(),
            fixture.evidence.clone(),
            fixture.binding.clone(),
        )
        .unwrap();
        let observation = publisher.publish(&fixture.manifest);
        assert!(matches!(observation, PublishObservation::Verified { .. }));
        let client = publisher.into_client();
        assert_eq!(
            (client.creates, client.uploads, client.publishes),
            (1, 1, 1)
        );
    }

    #[test]
    fn published_exact_release_is_idempotent_and_never_writes_again() {
        let fixture = fixture();
        let client = FakeClient {
            release: Some(RemoteRelease {
                id: 7,
                tag_name: fixture.binding.tag.clone(),
                target_commitish: fixture.binding.target_commitish.clone(),
                draft: false,
                prerelease: false,
                html_url: "https://example.invalid/releases/7".to_owned(),
                updated_at: "2026-07-23T00:00:00Z".to_owned(),
                assets: vec![RemoteReleaseAsset {
                    id: 101,
                    name: "asset.bin".to_owned(),
                    size: 5,
                    digest: Some(Sha256Hash::digest(b"asset")),
                    state: "uploaded".to_owned(),
                }],
            }),
            downloads: BTreeMap::from([(101, b"asset".to_vec())]),
            ..FakeClient::default()
        };
        let mut publisher = GitHubReleasePublisher::new(
            client,
            fixture.root.clone(),
            fixture.evidence.clone(),
            fixture.binding.clone(),
        )
        .unwrap();
        assert!(matches!(
            publisher.publish(&fixture.manifest),
            PublishObservation::Verified { .. }
        ));
        let client = publisher.into_client();
        assert_eq!(
            (client.creates, client.uploads, client.publishes),
            (0, 0, 0)
        );
    }

    #[test]
    fn timeout_reconcile_is_read_only_and_preserves_unknown_or_partial() {
        let fixture = fixture();
        let mut client = FakeClient::default();
        client.failures.push_back(GitHubClientError::Timeout);
        let mut publisher = GitHubReleasePublisher::new(
            client,
            fixture.root.clone(),
            fixture.evidence.clone(),
            fixture.binding.clone(),
        )
        .unwrap();
        assert_eq!(
            publisher.publish(&fixture.manifest),
            PublishObservation::Timeout
        );
        assert_eq!(
            publisher.reconcile(&fixture.manifest),
            PublishObservation::Timeout
        );
        let client = publisher.into_client();
        assert_eq!(
            (client.creates, client.uploads, client.publishes),
            (0, 0, 0)
        );
    }

    #[test]
    fn conflicting_remote_asset_never_clobbers() {
        let fixture = fixture();
        let client = FakeClient {
            release: Some(RemoteRelease {
                id: 7,
                tag_name: fixture.binding.tag.clone(),
                target_commitish: fixture.binding.target_commitish.clone(),
                draft: true,
                prerelease: false,
                html_url: "https://example.invalid/draft/7".to_owned(),
                updated_at: "2026-07-23T00:00:00Z".to_owned(),
                assets: vec![RemoteReleaseAsset {
                    id: 101,
                    name: "asset.bin".to_owned(),
                    size: 6,
                    digest: Some(Sha256Hash::digest(b"wrong")),
                    state: "uploaded".to_owned(),
                }],
            }),
            ..FakeClient::default()
        };
        let mut publisher = GitHubReleasePublisher::new(
            client,
            fixture.root.clone(),
            fixture.evidence.clone(),
            fixture.binding.clone(),
        )
        .unwrap();
        assert!(matches!(
            publisher.publish(&fixture.manifest),
            PublishObservation::Partial { .. }
        ));
        let client = publisher.into_client();
        assert_eq!(client.uploads, 0);
        assert_eq!(client.publishes, 0);
    }
}
