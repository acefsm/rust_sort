//! GNU sort implementation in Rust
//! 
//! This crate provides a complete, production-ready implementation of the GNU sort utility
//! with all major features including multiple comparison modes, field sorting, parallelization,
//! and memory-efficient operations.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::all)]

pub mod error;
pub mod config;

// Core sorting implementations
pub mod zero_copy;
pub mod core_sort;
pub mod radix_sort;
pub mod simd_compare;
pub mod external_sort;
pub mod adaptive_sort;
pub mod hash_sort;
pub mod args;
pub mod locale;

// Re-export commonly used types
pub use error::{SortError, SortResult};
pub use config::{SortConfig, SortMode, SortOrder};

/// Exit codes matching GNU sort
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;
pub const SORT_FAILURE: i32 = 2;

/// Main sort function that processes input according to configuration
pub fn sort(config: &SortConfig, input_files: &[String]) -> SortResult<i32> {
    // Use Core Sort implementation for optimal performance
    let args = crate::args::SortArgs {
        files: input_files.to_vec(),
        output: config.output_file.clone(),
        reverse: config.reverse,
        numeric_sort: matches!(config.mode, crate::config::SortMode::Numeric),
        general_numeric_sort: matches!(config.mode, crate::config::SortMode::GeneralNumeric),
        random_sort: matches!(config.mode, crate::config::SortMode::Random),
        ignore_case: config.ignore_case,
        unique: config.unique,
        stable: config.stable,
        field_separator: config.field_separator,
        zero_terminated: config.zero_terminated,
        check: config.check,
        merge: config.merge,
    };
    
    let core_sort = crate::core_sort::CoreSort::new(args, config.clone());
    core_sort.sort().map_err(|e| SortError::internal(&e.to_string()))?;
    Ok(EXIT_SUCCESS)
}