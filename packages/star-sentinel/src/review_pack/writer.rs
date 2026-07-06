use super::artifact::validate_review_pack_artifact;
use crate::constants::{
    REVIEW_PACK_JSON_FILE, REVIEW_PACK_MARKDOWN_FILE, STAR_SENTINEL_TOOL_OUTPUT_DIR,
};
use crate::json_fields::missing_field;
use crate::model::ReviewPackArtifactRefs;
use crate::SentinelError;
use serde_json::Value;
use star_control_state::StateStore;
use std::path::Path;

pub fn write_review_pack_artifacts(
    store: &StateStore,
    job_id: &str,
    review_pack: &Value,
    schema_root: impl AsRef<Path>,
) -> Result<ReviewPackArtifactRefs, SentinelError> {
    validate_review_pack_artifact(review_pack, schema_root.as_ref())?;
    let markdown = review_pack
        .get("review_pack_markdown")
        .and_then(Value::as_str)
        .ok_or_else(|| missing_field(REVIEW_PACK_JSON_FILE, "review_pack_markdown"))?;

    let tool_json_ref = store
        .write_tool_json(
            job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            REVIEW_PACK_JSON_FILE,
            review_pack,
        )
        .map_err(|source| SentinelError::State { source })?;
    let tool_markdown_ref = store
        .write_tool_text(
            job_id,
            STAR_SENTINEL_TOOL_OUTPUT_DIR,
            REVIEW_PACK_MARKDOWN_FILE,
            markdown,
        )
        .map_err(|source| SentinelError::State { source })?;
    let review_json_ref = store
        .write_review_pack_json(job_id, REVIEW_PACK_JSON_FILE, review_pack)
        .map_err(|source| SentinelError::State { source })?;
    let review_markdown_ref = store
        .write_review_pack_markdown(job_id, REVIEW_PACK_MARKDOWN_FILE, markdown)
        .map_err(|source| SentinelError::State { source })?;

    Ok(ReviewPackArtifactRefs {
        tool_json_ref,
        tool_markdown_ref,
        review_json_ref,
        review_markdown_ref,
    })
}
