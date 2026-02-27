use std::fmt::{Display, Formatter};

use worker::{Response, Result};

use crate::models::ErrorResponse;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized(String),
    BadRequest(String),
    NotFound(String),
    Upstream(String),
    Parse(String),
    Validation(String),
    Internal(String),
}

impl ApiError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized(_) => "unauthorized",
            Self::BadRequest(_) => "bad_request",
            Self::NotFound(_) => "not_found",
            Self::Upstream(_) => "upstream_error",
            Self::Parse(_) => "parse_error",
            Self::Validation(_) => "validation_error",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Unauthorized(message)
            | Self::BadRequest(message)
            | Self::NotFound(message)
            | Self::Upstream(message)
            | Self::Parse(message)
            | Self::Validation(message)
            | Self::Internal(message) => message,
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::Unauthorized(_) => 401,
            Self::BadRequest(_) => 400,
            Self::NotFound(_) => 404,
            Self::Upstream(_) => 502,
            Self::Parse(_) => 422,
            Self::Validation(_) => 422,
            Self::Internal(_) => 500,
        }
    }

    pub fn into_response(self) -> Result<Response> {
        let mut response = Response::from_json(&ErrorResponse {
            code: self.code().to_string(),
            message: self.message().to_string(),
        })?;
        response.headers_mut().set("Cache-Control", "no-store")?;
        Ok(response.with_status(self.status_code()))
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for ApiError {}

impl From<worker::Error> for ApiError {
    fn from(error: worker::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::Parse(error.to_string())
    }
}

impl From<worker::KvError> for ApiError {
    fn from(error: worker::KvError) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<url::ParseError> for ApiError {
    fn from(error: url::ParseError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<std::num::ParseIntError> for ApiError {
    fn from(error: std::num::ParseIntError) -> Self {
        Self::BadRequest(error.to_string())
    }
}
