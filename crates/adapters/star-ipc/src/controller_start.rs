//! Verified Controller bootstrap without PATH lookup or same-Job fallback.

use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command},
};

use star_contracts::Sha256Hash;
use thiserror::Error;
use windows::{
    Win32::{
        Storage::FileSystem::FILE_SHARE_READ,
        System::{
            JobObjects::{
                IsProcessInJob, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
                JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectExtendedLimitInformation, QueryInformationJobObject,
            },
            Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_NO_WINDOW, GetCurrentProcess},
        },
    },
    core::BOOL,
};

#[derive(Debug, Error)]
pub enum ControllerStartError {
    #[error("installed Controller image identity does not match")]
    IdentityMismatch,
    #[error("installed Controller image cannot be leased")]
    Lease(#[from] io::Error),
    #[error("outer Job does not allow a durable Controller breakaway")]
    OuterJobDenied,
    #[error("Controller process could not start")]
    Start,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OuterJobPolicy {
    NotInJob,
    BreakawayAllowed,
    Denied,
}

pub struct VerifiedControllerImage {
    path: PathBuf,
    #[allow(dead_code)]
    lease: File,
    hash: Sha256Hash,
}

impl VerifiedControllerImage {
    pub fn open(path: &Path, expected_hash: &Sha256Hash) -> Result<Self, ControllerStartError> {
        let path = path.canonicalize()?;
        let lease = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(&path)?;
        let actual = Sha256Hash::digest_reader(lease.try_clone()?)?;
        if &actual != expected_hash {
            return Err(ControllerStartError::IdentityMismatch);
        }
        Ok(Self {
            path,
            lease,
            hash: actual,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn hash(&self) -> &Sha256Hash {
        &self.hash
    }

    pub fn start_background(&self) -> Result<Child, ControllerStartError> {
        let policy = current_outer_job_policy()?;
        let flags = launch_flags(policy)?;
        Command::new(&self.path)
            .arg("--background")
            .creation_flags(flags)
            .spawn()
            .map_err(|_| ControllerStartError::Start)
    }
}

pub fn classify_outer_job(in_job: bool, limit_flags: u32) -> OuterJobPolicy {
    if !in_job {
        OuterJobPolicy::NotInJob
    } else if limit_flags
        & (JOB_OBJECT_LIMIT_BREAKAWAY_OK.0 | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK.0)
        != 0
    {
        OuterJobPolicy::BreakawayAllowed
    } else {
        OuterJobPolicy::Denied
    }
}

pub fn launch_flags(policy: OuterJobPolicy) -> Result<u32, ControllerStartError> {
    Ok(match policy {
        OuterJobPolicy::NotInJob => CREATE_NO_WINDOW.0,
        OuterJobPolicy::BreakawayAllowed => CREATE_NO_WINDOW.0 | CREATE_BREAKAWAY_FROM_JOB.0,
        OuterJobPolicy::Denied => return Err(ControllerStartError::OuterJobDenied),
    })
}

pub fn current_outer_job_policy() -> Result<OuterJobPolicy, ControllerStartError> {
    let mut in_job = BOOL(0);
    unsafe { IsProcessInJob(GetCurrentProcess(), None, &mut in_job) }
        .map_err(|_| ControllerStartError::OuterJobDenied)?;
    if !in_job.as_bool() {
        return Ok(OuterJobPolicy::NotInJob);
    }
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    unsafe {
        QueryInformationJobObject(
            None,
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            None,
        )
    }
    .map_err(|_| ControllerStartError::OuterJobDenied)?;
    Ok(classify_outer_job(
        true,
        limits.BasicLimitInformation.LimitFlags.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // matrix: MCP-I013
    fn verified_image_lease_prevents_path_replacement_and_mismatch_never_starts() {
        let directory =
            std::env::temp_dir().join(format!("star-controller-image-{}", crate::nonce()));
        std::fs::create_dir_all(&directory).unwrap();
        let installed = directory.join("star-controller.exe");
        let replacement = directory.join("replacement.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &installed).unwrap();
        std::fs::write(&replacement, b"different executable bytes").unwrap();
        let expected = Sha256Hash::digest_reader(File::open(&installed).unwrap()).unwrap();
        let lease = VerifiedControllerImage::open(&installed, &expected).unwrap();
        assert_eq!(lease.path(), installed.canonicalize().unwrap());
        assert_eq!(lease.hash(), &expected);
        assert!(std::fs::rename(&replacement, &installed).is_err());
        assert!(matches!(
            VerifiedControllerImage::open(&installed, &Sha256Hash::digest(b"wrong")),
            Err(ControllerStartError::IdentityMismatch)
        ));
        drop(lease);
        assert!(std::fs::rename(&replacement, &installed).is_ok());
    }

    #[test]
    // matrix: MCP-I014
    fn outer_job_policy_uses_breakaway_or_fails_before_same_job_start() {
        assert_eq!(
            launch_flags(classify_outer_job(false, 0)).unwrap(),
            CREATE_NO_WINDOW.0
        );
        assert_ne!(
            launch_flags(classify_outer_job(true, JOB_OBJECT_LIMIT_BREAKAWAY_OK.0)).unwrap()
                & CREATE_BREAKAWAY_FROM_JOB.0,
            0
        );
        assert!(matches!(
            launch_flags(classify_outer_job(true, 0)),
            Err(ControllerStartError::OuterJobDenied)
        ));
    }
}
