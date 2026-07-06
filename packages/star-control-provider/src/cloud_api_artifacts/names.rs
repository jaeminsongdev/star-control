use crate::{OpenAiCompatibleRequestApi, OpenAiCompatibleResponseKind};

pub(super) fn credential_reference_kind(credential_ref: &str) -> &str {
    credential_ref
        .split_once(':')
        .map(|(kind, _)| kind)
        .unwrap_or("unknown")
}

pub(super) fn request_api_name(api: OpenAiCompatibleRequestApi) -> &'static str {
    match api {
        OpenAiCompatibleRequestApi::Responses => "responses",
        OpenAiCompatibleRequestApi::ChatCompletions => "chat_completions",
    }
}

pub(super) fn response_kind_name(kind: OpenAiCompatibleResponseKind) -> &'static str {
    match kind {
        OpenAiCompatibleResponseKind::Responses => "responses",
        OpenAiCompatibleResponseKind::ChatCompletions => "chat_completions",
    }
}
