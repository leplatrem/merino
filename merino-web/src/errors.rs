//! Any errors that merino-web might generate, and supporting implementations.
//!
//! This module implements the supporting functionalities to manipulate
//! [crate::error::HandlerError] to make it esier to send them to Sentry.
//! The [crate::error::HandlerError] wraps the internal error [crate::error::HandlerErrorKind]
//! and a related backtrace.
//! The corresponding backtrace is captured when the error is created.
//! This happens automatically when a [crate::error::HandlerErrorKind] is converted into a [crate::error::HandlerError].
//!
//! Developers are expected to use [crate::error::HandlerError] as the error type of
//! their functions and to set the appropriate error by
//! * explicitly converting it using `into()`, e.g. `Err(HandlerErrorKind::Internal.into())`,
//! * implicitly converting it using the question mark operator, e.g. `Err(HandleErrorKind::Interal)?`.
//!
//! New errors can be added by extending [crate::error::HandlerErrorKind].

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use backtrace::Backtrace;
use serde_json::Value;
use thiserror::Error;

/// The Standard Error for most of Merino
pub struct HandlerError {
    // Important: please make sure to update the implementation of
    // std::fmt::Debug for this struct if new fields are added here.
    /// The wrapped error value.
    kind: HandlerErrorKind,
    /// The backtrace related to the wrapped error.
    pub(crate) backtrace: Backtrace,
}

/// An error that happened in a web handler.
#[derive(Error, Debug)]
pub enum HandlerErrorKind {
    /// A generic error, when there is nothing more specific to say.
    #[error("Internal error")]
    Internal,

    /// An error that indicates that one of the request headers is malformed.
    #[error("Malformed header: {0}")]
    MalformedHeader(&'static str),
}

impl From<HandlerErrorKind> for actix_web::Error {
    fn from(kind: HandlerErrorKind) -> Self {
        let error: HandlerError = kind.into();
        error.into()
    }
}

impl HandlerError {
    /// Access the wrapped error.
    pub fn kind(&self) -> &HandlerErrorKind {
        &self.kind
    }

    /// Get an `HandlerError` representing an `Internal` error.
    ///
    /// This is a convenience function: the same result can be
    /// achieved by directly using `HandlerErrorKind::Internal.into()`.
    pub fn internal() -> Self {
        HandlerErrorKind::Internal.into()
    }
}

impl Error for HandlerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.kind.source()
    }
}

impl<T> From<T> for HandlerError
where
    HandlerErrorKind: From<T>,
{
    fn from(item: T) -> Self {
        HandlerError {
            kind: HandlerErrorKind::from(item),
            backtrace: Backtrace::new(),
        }
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl std::fmt::Debug for HandlerError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        // Sentry will scan the printed debug information for `HandlerError`
        // to determine the "event type" to display and to group events by:
        // to make sure different errors don't get grouped together, we format
        // the name of this debug struct as `HandlerError/<error name>`.
        // See `sentry::parse_type_from_debug` used by middleware/sentry.rs
        fmt.debug_struct(&format!("HandlerError/{:?}", &self.kind))
            .field("kind", &self.kind)
            .field("backtrace", &self.backtrace)
            .finish()
    }
}

impl ResponseError for HandlerError {
    /// Convert the error to an HTTP status code.
    fn status_code(&self) -> StatusCode {
        match self.kind() {
            HandlerErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            HandlerErrorKind::MalformedHeader(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let mut response = HashMap::new();
        response.insert(
            "error".to_owned(),
            Value::String(format!("{}", self.kind())),
        );
        HttpResponse::InternalServerError().json(response)
    }
}
