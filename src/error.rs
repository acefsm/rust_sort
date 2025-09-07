//! Error handling for the sort utility

use std::io;
use thiserror::Error;

/// Custom error type for sort operations
#[derive(Error, Debug)]
pub enum SortError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Permission denied: {file}")]
    PermissionDenied { file: String },

    #[error("No such file or directory: {file}")]
    FileNotFound { file: String },

    #[error("Is a directory: {file}")]
    IsDirectory { file: String },

    #[error("Invalid key specification: {spec}")]
    InvalidKeySpec { spec: String },

    #[error("Invalid field separator: {sep}")]
    InvalidFieldSeparator { sep: String },

    #[error("Invalid buffer size: {size}")]
    InvalidBufferSize { size: String },

    #[error("Conflicting sort options: {message}")]
    ConflictingOptions { message: String },

    #[error("Memory allocation failed")]
    OutOfMemory,

    #[error("Input is not sorted at line {line}")]
    NotSorted { line: usize },

    #[error("Merge operation failed: {message}")]
    MergeFailed { message: String },

    #[error("Thread pool error: {message}")]
    ThreadPoolError { message: String },

    #[error("UTF-8 encoding error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl SortError {
    /// Returns the appropriate exit code for this error
    pub fn exit_code(&self) -> i32 {
        match self {
            SortError::PermissionDenied { .. }
            | SortError::FileNotFound { .. }
            | SortError::IsDirectory { .. }
            | SortError::Io(_) => crate::SORT_FAILURE,

            SortError::NotSorted { .. } => crate::EXIT_FAILURE,

            _ => crate::EXIT_FAILURE,
        }
    }

    /// Create a permission denied error
    pub fn permission_denied(file: &str) -> Self {
        SortError::PermissionDenied {
            file: file.to_string(),
        }
    }

    /// Create a file not found error
    pub fn file_not_found(file: &str) -> Self {
        SortError::FileNotFound {
            file: file.to_string(),
        }
    }

    /// Create an is directory error
    pub fn is_directory(file: &str) -> Self {
        SortError::IsDirectory {
            file: file.to_string(),
        }
    }

    /// Create an invalid key spec error
    pub fn invalid_key_spec(spec: &str) -> Self {
        SortError::InvalidKeySpec {
            spec: spec.to_string(),
        }
    }

    /// Create an invalid field separator error
    pub fn invalid_field_separator(sep: &str) -> Self {
        SortError::InvalidFieldSeparator {
            sep: sep.to_string(),
        }
    }

    /// Create an invalid buffer size error
    pub fn invalid_buffer_size(size: &str) -> Self {
        SortError::InvalidBufferSize {
            size: size.to_string(),
        }
    }

    /// Create a conflicting options error
    pub fn conflicting_options(message: &str) -> Self {
        SortError::ConflictingOptions {
            message: message.to_string(),
        }
    }

    /// Create a not sorted error
    pub fn not_sorted(line: usize) -> Self {
        SortError::NotSorted { line }
    }

    /// Create a merge failed error
    pub fn merge_failed(message: &str) -> Self {
        SortError::MergeFailed {
            message: message.to_string(),
        }
    }

    /// Create a thread pool error
    pub fn thread_pool_error(message: &str) -> Self {
        SortError::ThreadPoolError {
            message: message.to_string(),
        }
    }

    /// Create a parse error
    pub fn parse_error(message: &str) -> Self {
        SortError::ParseError {
            message: message.to_string(),
        }
    }

    /// Create an internal error
    pub fn internal(message: &str) -> Self {
        SortError::Internal {
            message: message.to_string(),
        }
    }
}

/// Convert io::Error to SortError with context (removed to avoid conflict with thiserror derive)
/// Result type for sort operations
pub type SortResult<T> = Result<T, SortError>;

/// Context trait for adding context to errors
pub trait SortContext<T> {
    fn with_context<F>(self, f: F) -> SortResult<T>
    where
        F: FnOnce() -> String;

    fn with_file_context(self, filename: &str) -> SortResult<T>;
}

impl<T> SortContext<T> for SortResult<T> {
    fn with_context<F>(self, f: F) -> SortResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|err| match err {
            SortError::Io(io_err) => SortError::Io(io::Error::new(
                io_err.kind(),
                format!("{}: {}", f(), io_err),
            )),
            other => other,
        })
    }

    fn with_file_context(self, filename: &str) -> SortResult<T> {
        self.map_err(|err| match err {
            SortError::Io(io_err) => match io_err.kind() {
                io::ErrorKind::PermissionDenied => SortError::permission_denied(filename),
                io::ErrorKind::NotFound => SortError::file_not_found(filename),
                _ => SortError::Io(io::Error::new(
                    io_err.kind(),
                    format!("{}: {}", filename, io_err),
                )),
            },
            other => other,
        })
    }
}

impl<T> SortContext<T> for Result<T, io::Error> {
    fn with_context<F>(self, f: F) -> SortResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|io_err| {
            SortError::Io(io::Error::new(
                io_err.kind(),
                format!("{}: {}", f(), io_err),
            ))
        })
    }

    fn with_file_context(self, filename: &str) -> SortResult<T> {
        self.map_err(|io_err| match io_err.kind() {
            io::ErrorKind::PermissionDenied => SortError::permission_denied(filename),
            io::ErrorKind::NotFound => SortError::file_not_found(filename),
            _ => SortError::Io(io::Error::new(
                io_err.kind(),
                format!("{}: {}", filename, io_err),
            )),
        })
    }
}
