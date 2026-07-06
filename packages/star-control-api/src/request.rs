use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl ApiMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiRequest {
    method: ApiMethod,
    path: String,
    body: Value,
}

impl ApiRequest {
    pub fn new(method: ApiMethod, path: impl Into<String>) -> Self {
        Self::with_body(method, path, Value::Null)
    }

    pub fn with_body(method: ApiMethod, path: impl Into<String>, body: Value) -> Self {
        Self {
            method,
            path: path.into(),
            body,
        }
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self::new(ApiMethod::Get, path)
    }

    pub fn post(path: impl Into<String>, body: Value) -> Self {
        Self::with_body(ApiMethod::Post, path, body)
    }

    pub fn method(&self) -> ApiMethod {
        self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn body(&self) -> &Value {
        &self.body
    }
}
