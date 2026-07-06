mod markdown;
mod storage;

use crate::constants::RELEASE_REVIEW_PACK_PATH;
use crate::error::ReleaseReadinessError;
use crate::writer::ReleaseReadinessWriter;
use markdown::render_release_review_pack_markdown;
use serde_json::Value;
use star_control_state::{ArtifactKind, StateStore};
use storage::write_new_text;

#[derive(Debug, Clone)]
pub struct ReleaseReviewPackWriter {
    readiness_writer: ReleaseReadinessWriter,
}

impl ReleaseReviewPackWriter {
    pub fn new(schema_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            readiness_writer: ReleaseReadinessWriter::new(schema_root),
        }
    }

    pub fn build_markdown(&self, readiness: &Value) -> Result<String, ReleaseReadinessError> {
        self.readiness_writer.validate_readiness(readiness)?;
        Ok(render_release_review_pack_markdown(readiness))
    }

    pub fn write(
        &self,
        store: &StateStore,
        job_id: &str,
        readiness: &Value,
    ) -> Result<Value, ReleaseReadinessError> {
        let markdown = self.build_markdown(readiness)?;
        let path = store.resolve_job_path(job_id, RELEASE_REVIEW_PACK_PATH)?;
        write_new_text(&path, &markdown)?;
        store
            .artifact_ref(
                job_id,
                RELEASE_REVIEW_PACK_PATH,
                ArtifactKind::ReviewPack,
                "star-control-release",
                None,
                Some("release review pack Markdown artifact"),
            )
            .map_err(ReleaseReadinessError::from)
    }
}
