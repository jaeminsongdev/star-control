mod approve;
mod cancel;
mod resume;

use super::ApiControlService;
use crate::error::ApiError;
use serde_json::{json, Value};

impl ApiControlService {
    fn invalid_control_request(&self, message: &str) -> Result<Value, ApiError> {
        self.read_only
            .error_envelope("invalid_control_request", message, json!({}))
    }
}
