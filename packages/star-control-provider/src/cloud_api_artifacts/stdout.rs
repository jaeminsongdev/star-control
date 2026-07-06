use crate::{OpenAiCompatiblePreparedRequest, ProviderManifest};

pub(crate) fn api_offline_stdout_value(
    manifest: &ProviderManifest,
    prepared_request: &OpenAiCompatiblePreparedRequest,
    fixture_relative_path: &str,
) -> String {
    format!(
        "cloud API offline fixture\nprovider_id={}\nkind={}\ntransport={}\nrequest_method={}\nrequest_url={}\nfixture_path={}\ntransport_execution=offline_fixture\nlive_api_call=false\n",
        manifest.id(),
        manifest.kind(),
        manifest.transport(),
        prepared_request.method(),
        prepared_request.url(),
        fixture_relative_path,
    )
}

pub(crate) fn api_live_approval_stdout_value(
    manifest: &ProviderManifest,
    prepared_request: &OpenAiCompatiblePreparedRequest,
) -> String {
    format!(
        "cloud API live transport approval required\nprovider_id={}\nkind={}\ntransport={}\nrequest_method={}\nrequest_url={}\ntransport_execution=approval_required\nlive_api_call=false\ncredential_materialized=false\n",
        manifest.id(),
        manifest.kind(),
        manifest.transport(),
        prepared_request.method(),
        prepared_request.url(),
    )
}
