mod body;
mod error;
mod fields;

use crate::{ExecutionRequest, ProviderInstance};
pub use error::OpenAiCompatibleRequestError;
use fields::{endpoint_url, request_api, required_string_pointer};
use serde_json::Value;

const DEFAULT_RESPONSES_PATH: &str = "/responses";
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";
const DEFAULT_API: OpenAiCompatibleRequestApi = OpenAiCompatibleRequestApi::Responses;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiCompatibleRequestApi {
    Responses,
    ChatCompletions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiCompatiblePreparedRequest {
    api: OpenAiCompatibleRequestApi,
    method: String,
    url: String,
    body: Value,
}

impl OpenAiCompatiblePreparedRequest {
    pub fn api(&self) -> OpenAiCompatibleRequestApi {
        self.api
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn body(&self) -> &Value {
        &self.body
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenAiCompatibleRequestBuilder;

impl OpenAiCompatibleRequestBuilder {
    pub fn build(
        &self,
        request: &ExecutionRequest,
        instance: &ProviderInstance,
    ) -> Result<OpenAiCompatiblePreparedRequest, OpenAiCompatibleRequestError> {
        let api = request_api(instance.value())?;
        let base_url = required_string_pointer(instance.value(), "/endpoint/base_url")?;
        let model = required_string_pointer(instance.value(), "/endpoint/model")?;
        let url = endpoint_url(&base_url, api);
        let body = body::request_body(api, &model, request.goal());

        Ok(OpenAiCompatiblePreparedRequest {
            api,
            method: "POST".to_string(),
            url,
            body,
        })
    }
}
