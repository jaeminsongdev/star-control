mod request;
mod response;

pub use request::{
    OpenAiCompatiblePreparedRequest, OpenAiCompatibleRequestApi, OpenAiCompatibleRequestBuilder,
    OpenAiCompatibleRequestError,
};
pub use response::{
    OpenAiCompatibleParseError, OpenAiCompatibleParsedResponse, OpenAiCompatibleResponseKind,
    OpenAiCompatibleResponseParser,
};

#[cfg(test)]
mod tests;
