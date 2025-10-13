use thiserror::Error;

use super::DidCheqdError;

#[derive(Error, Debug)]
pub enum ParsingErrorSource {
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Invalid URL: {0}")]
    UrlParsingError(url::ParseError),
    #[error("Invalid encoding: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error("Invalid encoding: {0}")]
    IntConversionError(#[from] std::num::TryFromIntError),
}

impl From<serde_json::Error> for DidCheqdError {
    fn from(error: serde_json::Error) -> Self {
        DidCheqdError::ParsingError(ParsingErrorSource::JsonError(error))
    }
}

impl From<url::ParseError> for DidCheqdError {
    fn from(error: url::ParseError) -> Self {
        DidCheqdError::ParsingError(ParsingErrorSource::UrlParsingError(error))
    }
}

impl From<std::string::FromUtf8Error> for DidCheqdError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        DidCheqdError::ParsingError(ParsingErrorSource::Utf8Error(error))
    }
}

impl From<std::num::TryFromIntError> for DidCheqdError {
    fn from(error: std::num::TryFromIntError) -> Self {
        DidCheqdError::ParsingError(ParsingErrorSource::IntConversionError(error))
    }
}
