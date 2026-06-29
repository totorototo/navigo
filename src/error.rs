//! Error types for fallible trace operations.

use std::error::Error;
use std::fmt;

/// Errors returned by fallible [`crate::Trace`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceError {
    /// The trace has no locations.
    EmptyTrace,
    /// `start_index` was not smaller than `end_index`.
    InvalidRange {
        start_index: usize,
        end_index: usize,
    },
    /// `index` is not a valid location index for a trace of length `len`.
    IndexOutOfBounds { index: usize, len: usize },
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceError::EmptyTrace => write!(f, "trace has no locations"),
            TraceError::InvalidRange {
                start_index,
                end_index,
            } => write!(
                f,
                "start_index ({start_index}) must be smaller than end_index ({end_index})"
            ),
            TraceError::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds for trace of length {len}")
            }
        }
    }
}

impl Error for TraceError {}
