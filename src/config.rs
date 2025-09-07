//! Configuration management for sort operations

use crate::error::{SortError, SortResult};
use std::str::FromStr;

/// Sort key specification for field-based sorting
#[derive(Debug, Clone)]
pub struct SortKey {
    /// Starting field number (1-based)
    pub start_field: usize,
    /// Starting character position within field (1-based, optional)
    pub start_char: Option<usize>,
    /// Ending field number (1-based, optional)
    pub end_field: Option<usize>,
    /// Ending character position within field (1-based, optional)
    pub end_char: Option<usize>,
    /// Sort options specific to this key
    pub options: SortKeyOptions,
}

/// Options specific to a sort key
#[derive(Debug, Clone, Default)]
pub struct SortKeyOptions {
    pub numeric: bool,
    pub general_numeric: bool,
    pub month: bool,
    pub reverse: bool,
    pub ignore_case: bool,
    pub dictionary_order: bool,
    pub ignore_leading_blanks: bool,
    pub human_numeric: bool,
    pub version: bool,
    pub random: bool,
}

impl SortKey {
    /// Parse a sort key from a string like "2,4" or "1.3,1.5" or "2nr"
    pub fn parse(keydef: &str) -> SortResult<Self> {
        // Split by comma to get start and optional end
        let parts: Vec<&str> = keydef.split(',').collect();
        if parts.is_empty() || parts.len() > 2 {
            return Err(SortError::parse_error(&format!(
                "invalid key specification: {keydef}"
            )));
        }

        // Parse start position and options
        let (start_field, start_char, start_opts) = Self::parse_field_spec(parts[0])?;

        // Parse end position if present
        let (end_field, end_char, end_opts) = if parts.len() == 2 {
            let (field, char_pos, opts) = Self::parse_field_spec(parts[1])?;
            (Some(field), char_pos, opts)
        } else {
            (None, None, SortKeyOptions::default())
        };

        // Merge options (start options take precedence)
        let mut options = start_opts;
        // Apply end options only if they're set and start options aren't
        if !options.numeric {
            options.numeric = end_opts.numeric;
        }
        if !options.general_numeric {
            options.general_numeric = end_opts.general_numeric;
        }
        if !options.month {
            options.month = end_opts.month;
        }
        if !options.reverse {
            options.reverse = end_opts.reverse;
        }
        if !options.ignore_case {
            options.ignore_case = end_opts.ignore_case;
        }
        if !options.dictionary_order {
            options.dictionary_order = end_opts.dictionary_order;
        }
        if !options.ignore_leading_blanks {
            options.ignore_leading_blanks = end_opts.ignore_leading_blanks;
        }
        if !options.human_numeric {
            options.human_numeric = end_opts.human_numeric;
        }
        if !options.version {
            options.version = end_opts.version;
        }
        if !options.random {
            options.random = end_opts.random;
        }

        Ok(Self {
            start_field,
            start_char,
            end_field,
            end_char,
            options,
        })
    }

    /// Parse a field specification like "2" or "2.3" or "2nr"
    fn parse_field_spec(spec: &str) -> SortResult<(usize, Option<usize>, SortKeyOptions)> {
        if spec.is_empty() {
            return Err(SortError::parse_error("empty field specification"));
        }

        let mut chars = spec.chars().peekable();
        let mut field_str = String::new();
        let mut char_str = String::new();
        let mut options = SortKeyOptions::default();

        // Parse field number
        while let Some(&ch) = chars.peek() {
            if ch.is_ascii_digit() {
                field_str.push(ch);
                chars.next();
            } else {
                break;
            }
        }

        if field_str.is_empty() {
            return Err(SortError::parse_error(&format!(
                "invalid field specification: {spec}"
            )));
        }

        let field = field_str
            .parse::<usize>()
            .map_err(|_| SortError::parse_error(&format!("invalid field number: {field_str}")))?;

        if field == 0 {
            return Err(SortError::parse_error("field numbers start at 1"));
        }

        // Check for character position (after a dot)
        let char_pos = if chars.peek() == Some(&'.') {
            chars.next(); // consume the dot
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() {
                    char_str.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }

            if char_str.is_empty() {
                None
            } else {
                let pos = char_str.parse::<usize>().map_err(|_| {
                    SortError::parse_error(&format!("invalid character position: {char_str}"))
                })?;
                if pos == 0 {
                    return Err(SortError::parse_error("character positions start at 1"));
                }
                Some(pos)
            }
        } else {
            None
        };

        // Parse options (single letters after the field spec)
        for ch in chars {
            match ch {
                'n' => options.numeric = true,
                'g' => options.general_numeric = true,
                'M' => options.month = true,
                'r' => options.reverse = true,
                'f' => options.ignore_case = true,
                'd' => options.dictionary_order = true,
                'b' => options.ignore_leading_blanks = true,
                'h' => options.human_numeric = true,
                'V' => options.version = true,
                'R' => options.random = true,
                'i' => {} // ignore non-printing - not fully implemented
                'z' => {} // zero-terminated - handled globally
                _ => {
                    return Err(SortError::parse_error(&format!("invalid key option: {ch}")));
                }
            }
        }

        Ok((field, char_pos, options))
    }
}

/// Main configuration structure for sort operations
#[derive(Debug, Clone)]
pub struct SortConfig {
    /// Primary sort mode
    pub mode: SortMode,
    /// Sort order (normal or reverse)
    pub reverse: bool,
    /// Output only unique lines
    pub unique: bool,
    /// Use stable sort algorithm
    pub stable: bool,
    /// Check if input is already sorted
    pub check: bool,
    /// Merge already sorted files
    pub merge: bool,
    /// Use zero bytes as line terminators instead of newlines
    pub zero_terminated: bool,
    /// Ignore case differences
    pub ignore_case: bool,
    /// Consider only dictionary order (alphanumeric and blanks)
    pub dictionary_order: bool,
    /// Ignore leading blanks
    pub ignore_leading_blanks: bool,
    /// Ignore non-printing characters
    pub ignore_nonprinting: bool,
    /// Field separator character
    pub field_separator: Option<char>,
    /// Sort keys (field specifications)
    pub keys: Vec<SortKey>,
    /// Output file path
    pub output_file: Option<String>,
    /// Buffer size for I/O operations
    pub buffer_size: Option<usize>,
    /// Number of parallel threads to use
    pub parallel_threads: Option<usize>,
    /// Files to read from (if not specified, use stdin)
    pub input_files: Vec<String>,
    /// Debug mode (for troubleshooting)
    pub debug: bool,
    /// Compress temporary files
    pub compress_temp: bool,
    /// Temporary directory for external sorting
    pub temp_dir: Option<String>,
}

/// Sort mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// Standard lexicographic sorting
    Lexicographic,
    /// Numeric sorting (integers)
    Numeric,
    /// General numeric sorting (floating point)
    GeneralNumeric,
    /// Human-readable numeric sorting (with suffixes like K, M, G)
    HumanNumeric,
    /// Month name sorting
    Month,
    /// Version number sorting
    Version,
    /// Random sorting (but group identical keys)
    Random,
}

/// Sort order enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortConfig {
    fn default() -> Self {
        Self {
            mode: SortMode::Lexicographic,
            reverse: false,
            unique: false,
            stable: false,
            check: false,
            merge: false,
            zero_terminated: false,
            ignore_case: false,
            dictionary_order: false,
            ignore_leading_blanks: false,
            ignore_nonprinting: false,
            field_separator: None,
            keys: Vec::new(),
            output_file: None,
            buffer_size: None,
            parallel_threads: None,
            input_files: Vec::new(),
            debug: false,
            compress_temp: false,
            temp_dir: None,
        }
    }
}

impl SortConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the sort mode
    pub fn with_mode(mut self, mode: SortMode) -> Self {
        self.mode = mode;
        self
    }

    /// Enable reverse sorting
    pub fn with_reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Enable unique output
    pub fn with_unique(mut self, unique: bool) -> Self {
        self.unique = unique;
        self
    }

    /// Enable stable sorting
    pub fn with_stable(mut self, stable: bool) -> Self {
        self.stable = stable;
        self
    }

    /// Enable check mode
    pub fn with_check(mut self, check: bool) -> Self {
        self.check = check;
        self
    }

    /// Enable merge mode
    pub fn with_merge(mut self, merge: bool) -> Self {
        self.merge = merge;
        self
    }

    /// Enable zero-terminated lines
    pub fn with_zero_terminated(mut self, zero_terminated: bool) -> Self {
        self.zero_terminated = zero_terminated;
        self
    }

    /// Set field separator
    pub fn with_field_separator(mut self, separator: Option<char>) -> Self {
        self.field_separator = separator;
        self
    }

    /// Add a sort key
    pub fn add_key(mut self, key: SortKey) -> Self {
        self.keys.push(key);
        self
    }

    /// Set output file
    pub fn with_output_file(mut self, output_file: Option<String>) -> Self {
        self.output_file = output_file;
        self
    }

    /// Set buffer size
    pub fn with_buffer_size(mut self, buffer_size: Option<usize>) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    /// Set parallel threads
    pub fn with_parallel_threads(mut self, threads: Option<usize>) -> Self {
        self.parallel_threads = threads;
        self
    }

    /// Set input files
    pub fn with_input_files(mut self, files: Vec<String>) -> Self {
        self.input_files = files;
        self
    }

    /// Enable debug mode
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Parse buffer size from string (simplified)
    pub fn set_buffer_size_from_string(&mut self, size_str: &str) -> SortResult<()> {
        // Simple parsing for now - just parse as number
        let size = size_str
            .parse::<usize>()
            .map_err(|_| SortError::internal("Invalid buffer size"))?;
        self.buffer_size = Some(size);
        Ok(())
    }

    /// Validate configuration for consistency
    pub fn validate(&self) -> SortResult<()> {
        // Check for conflicting modes
        if self.check && self.merge {
            return Err(SortError::conflicting_options(
                "cannot use both --check and --merge",
            ));
        }

        if self.check && self.unique {
            return Err(SortError::conflicting_options(
                "--check is incompatible with --unique",
            ));
        }

        if self.merge && self.unique {
            // This is actually allowed, but warn about performance implications
        }

        // Validate field separator
        if let Some(sep) = self.field_separator {
            if sep == '\0' && !self.zero_terminated {
                return Err(SortError::invalid_field_separator(
                    "null character separator requires -z option",
                ));
            }
        }

        // Check for reasonable buffer size
        if let Some(buffer_size) = self.buffer_size {
            if buffer_size < 1024 {
                return Err(SortError::invalid_buffer_size(
                    "buffer size too small (minimum 1KB)",
                ));
            }
            if buffer_size > 1024 * 1024 * 1024 * 8 {
                // 8GB limit
                return Err(SortError::invalid_buffer_size(
                    "buffer size too large (maximum 8GB)",
                ));
            }
        }

        // Validate thread count
        if let Some(threads) = self.parallel_threads {
            if threads == 0 {
                return Err(SortError::thread_pool_error(
                    "thread count must be positive",
                ));
            }
            if threads > 1024 {
                return Err(SortError::thread_pool_error(
                    "too many threads (maximum 1024)",
                ));
            }
        }

        Ok(())
    }

    /// Get the effective sort order
    pub fn sort_order(&self) -> SortOrder {
        if self.reverse {
            SortOrder::Descending
        } else {
            SortOrder::Ascending
        }
    }

    /// Check if random sort is enabled
    pub fn random_sort(&self) -> bool {
        matches!(self.mode, SortMode::Random)
    }

    /// Check if numeric sort mode is enabled
    pub fn numeric_sort(&self) -> bool {
        matches!(
            self.mode,
            SortMode::Numeric | SortMode::GeneralNumeric | SortMode::HumanNumeric
        )
    }

    /// Check if any keys have specific sort types
    pub fn has_typed_keys(&self) -> bool {
        false // Simplified - no complex key checking
    }

    /// Get the number of input files (0 means stdin)
    pub fn input_file_count(&self) -> usize {
        self.input_files.len()
    }

    /// Check if reading from stdin
    pub fn reading_from_stdin(&self) -> bool {
        self.input_files.is_empty() || (self.input_files.len() == 1 && self.input_files[0] == "-")
    }

    /// Check if writing to stdout
    pub fn writing_to_stdout(&self) -> bool {
        self.output_file.is_none()
    }

    /// Get effective buffer size (with default)
    pub fn effective_buffer_size(&self) -> usize {
        self.buffer_size.unwrap_or(1024 * 1024) // 1MB default
    }

    /// Get effective thread count
    pub fn effective_thread_count(&self) -> usize {
        self.parallel_threads.unwrap_or_else(num_cpus::get)
    }

    /// Create a configuration for merge operations
    pub fn for_merge(&self) -> Self {
        let mut config = self.clone();
        config.merge = true;
        config.check = false;
        config
    }

    /// Create a configuration for check operations
    pub fn for_check(&self) -> Self {
        let mut config = self.clone();
        config.check = true;
        config.merge = false;
        config.unique = false; // Not applicable for check
        config
    }
}

impl FromStr for SortMode {
    type Err = SortError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "lexicographic" | "text" | "default" => Ok(SortMode::Lexicographic),
            "numeric" | "n" => Ok(SortMode::Numeric),
            "general-numeric" | "g" => Ok(SortMode::GeneralNumeric),
            "human-numeric" | "h" => Ok(SortMode::HumanNumeric),
            "month" | "m" => Ok(SortMode::Month),
            "version" | "v" => Ok(SortMode::Version),
            "random" | "r" => Ok(SortMode::Random),
            _ => Err(SortError::parse_error(&format!("unknown sort mode: {s}"))),
        }
    }
}

impl std::fmt::Display for SortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            SortMode::Lexicographic => "lexicographic",
            SortMode::Numeric => "numeric",
            SortMode::GeneralNumeric => "general-numeric",
            SortMode::HumanNumeric => "human-numeric",
            SortMode::Month => "month",
            SortMode::Version => "version",
            SortMode::Random => "random",
        };
        write!(f, "{name}")
    }
}

/// Builder pattern for creating configurations
pub struct SortConfigBuilder {
    config: SortConfig,
}

impl SortConfigBuilder {
    /// Start building a new configuration
    pub fn new() -> Self {
        Self {
            config: SortConfig::default(),
        }
    }

    /// Set sort mode
    pub fn mode(mut self, mode: SortMode) -> Self {
        self.config.mode = mode;
        self
    }

    /// Enable reverse sorting
    pub fn reverse(mut self) -> Self {
        self.config.reverse = true;
        self
    }

    /// Enable unique output
    pub fn unique(mut self) -> Self {
        self.config.unique = true;
        self
    }

    /// Enable stable sorting
    pub fn stable(mut self) -> Self {
        self.config.stable = true;
        self
    }

    /// Enable check mode
    pub fn check(mut self) -> Self {
        self.config.check = true;
        self
    }

    /// Enable merge mode
    pub fn merge(mut self) -> Self {
        self.config.merge = true;
        self
    }

    /// Enable zero-terminated lines
    pub fn zero_terminated(mut self) -> Self {
        self.config.zero_terminated = true;
        self
    }

    /// Set field separator
    pub fn field_separator(mut self, separator: char) -> Self {
        self.config.field_separator = Some(separator);
        self
    }

    /// Add a sort key
    pub fn key(mut self, key: SortKey) -> Self {
        self.config.keys.push(key);
        self
    }

    /// Set output file
    pub fn output_file(mut self, file: String) -> Self {
        self.config.output_file = Some(file);
        self
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = Some(size);
        self
    }

    /// Build the final configuration
    pub fn build(self) -> SortResult<SortConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for SortConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Preset configurations for common use cases
pub mod presets {
    use super::*;

    /// Configuration for numeric sorting
    pub fn numeric() -> SortConfig {
        SortConfig::new().with_mode(SortMode::Numeric)
    }

    /// Configuration for version sorting
    pub fn version() -> SortConfig {
        SortConfig::new().with_mode(SortMode::Version)
    }

    /// Configuration for human-readable sizes
    pub fn human_numeric() -> SortConfig {
        SortConfig::new().with_mode(SortMode::HumanNumeric)
    }

    /// Configuration for case-insensitive sorting
    pub fn case_insensitive() -> SortConfig {
        let mut config = SortConfig::new();
        config.ignore_case = true;
        config
    }

    /// Configuration for sorting with unique output
    pub fn unique() -> SortConfig {
        SortConfig::new().with_unique(true)
    }

    /// Configuration for reverse sorting
    pub fn reverse() -> SortConfig {
        SortConfig::new().with_reverse(true)
    }

    /// Configuration for stable sorting
    pub fn stable() -> SortConfig {
        SortConfig::new().with_stable(true)
    }

    /// Configuration for merge mode
    pub fn merge() -> SortConfig {
        SortConfig::new().with_merge(true)
    }

    /// Configuration for check mode
    pub fn check() -> SortConfig {
        SortConfig::new().with_check(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SortConfig::default();
        assert_eq!(config.mode, SortMode::Lexicographic);
        assert!(!config.reverse);
        assert!(!config.unique);
        assert!(!config.stable);
    }

    #[test]
    fn test_config_builder() {
        let config = SortConfigBuilder::new()
            .mode(SortMode::Numeric)
            .reverse()
            .unique()
            .build()
            .expect("Failed to build test config");

        assert_eq!(config.mode, SortMode::Numeric);
        assert!(config.reverse);
        assert!(config.unique);
    }

    #[test]
    fn test_sort_mode_from_str() {
        assert_eq!(
            "numeric"
                .parse::<SortMode>()
                .expect("Failed to parse numeric mode"),
            SortMode::Numeric
        );
        assert_eq!(
            "version"
                .parse::<SortMode>()
                .expect("Failed to parse version mode"),
            SortMode::Version
        );
        assert!("invalid".parse::<SortMode>().is_err());
    }

    #[test]
    fn test_validate_conflicting_options() {
        let config = SortConfig {
            check: true,
            merge: true,
            ..Default::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_effective_buffer_size() {
        let config = SortConfig::default();
        assert_eq!(config.effective_buffer_size(), 1024 * 1024);

        let config = SortConfig::default().with_buffer_size(Some(2048));
        assert_eq!(config.effective_buffer_size(), 2048);
    }

    #[test]
    fn test_presets() {
        let config = presets::numeric();
        assert_eq!(config.mode, SortMode::Numeric);

        let config = presets::reverse();
        assert!(config.reverse);

        let config = presets::unique();
        assert!(config.unique);
    }

    #[test]
    fn test_reading_from_stdin() {
        let config = SortConfig::default();
        assert!(config.reading_from_stdin());

        let config = SortConfig::default().with_input_files(vec!["-".to_string()]);
        assert!(config.reading_from_stdin());

        let config = SortConfig::default().with_input_files(vec!["file.txt".to_string()]);
        assert!(!config.reading_from_stdin());
    }
}
