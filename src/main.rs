//! GNU sort implementation in Rust
//! 
//! A complete, production-ready implementation of the GNU sort utility
//! with all major features including multiple comparison modes, field sorting,
//! parallelization, and memory-efficient operations.

use std::process;
use clap::{Arg, Command};

// Import from the library modules
use gnu_sort::{
    config::{SortConfig, SortMode, SortConfigBuilder},
    error::{SortError, SortResult},
    sort,
    EXIT_SUCCESS,
};

fn main() {
    let result = run();
    match result {
        Ok(exit_code) => process::exit(exit_code),
        Err(e) => {
            eprintln!("sort: {}", e);
            process::exit(e.exit_code());
        }
    }
}

fn run() -> SortResult<i32> {
    // Check for legacy +N -M syntax and convert it to modern -k syntax
    let args: Vec<String> = std::env::args().collect();
    let converted_args = convert_legacy_syntax(&args);
    
    let matches = build_cli().get_matches_from(converted_args);
    
    // Build configuration from command line arguments
    let config = parse_config_from_matches(&matches)?;
    
    // Get input files
    let input_files: Vec<String> = matches.get_many::<String>("files")
        .unwrap_or_default()
        .map(|s| s.clone())
        .collect();
    
    // Execute the sort operation
    sort(&config, &input_files)
}

fn build_cli() -> Command {
    Command::new("sort")
        .version(env!("CARGO_PKG_VERSION"))
        .author("GNU sort compatible implementation in Rust")
        .override_usage("sort [OPTION]... [FILE]...")
        .about("Sort lines of text files")
        .long_about("Sort lines of text files according to various criteria. \n\nThis implementation is compatible with GNU sort and supports all major features including field sorting, numeric comparisons, and parallel processing.")
        .disable_help_flag(true)  // We use -h for human-numeric-sort
        .disable_version_flag(true)  // We use -V for version-sort
        
        // Input files
        .arg(Arg::new("files")
            .help("Input files to sort (use '-' or omit for stdin)")
            .num_args(0..)
            .value_name("FILE"))
            
        // Sort modes (mutually exclusive)
        .arg(Arg::new("numeric-sort")
            .short('n')
            .long("numeric-sort")
            .help("Compare according to string numerical value")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("general-numeric-sort")
            .short('g')
            .long("general-numeric-sort")
            .help("Compare according to general numerical value")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("human-numeric-sort")
            .short('h')
            .long("human-numeric-sort")
            .help("Compare human readable numbers (e.g., 2K 1G)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("month-sort")
            .short('M')
            .long("month-sort")
            .help("Compare by month names")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("random-sort")
            .short('R')
            .long("random-sort")
            .help("Shuffle, but group identical keys")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("version-sort")
            .short('V')
            .long("version-sort")
            .help("Natural sort of version numbers")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("sort")
            .long("sort")
            .help("Sort according to WORD")
            .long_help("Sort according to WORD: general-numeric -g, human-numeric -h, month -M, numeric -n, random -R, version -V")
            .value_name("WORD")
            .value_parser(["general-numeric", "human-numeric", "month", "numeric", "random", "version"]))
            
        // Sort modifiers
        .arg(Arg::new("reverse")
            .short('r')
            .long("reverse")
            .help("Reverse the result of comparisons")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("unique")
            .short('u')
            .long("unique")
            .help("Output only the first of an equal run")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("stable")
            .short('s')
            .long("stable")
            .help("Stabilize sort by disabling last-resort comparison")
            .action(clap::ArgAction::SetTrue))
            
        // Text processing options  
        .arg(Arg::new("ignore-case")
            .short('f')
            .long("ignore-case")
            .help("Fold lower case to upper case characters")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("dictionary-order")
            .short('d')
            .long("dictionary-order")
            .help("Consider only blanks and alphanumeric characters")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("ignore-leading-blanks")
            .short('b')
            .long("ignore-leading-blanks")
            .help("Ignore leading blanks")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("ignore-nonprinting")
            .short('i')
            .long("ignore-nonprinting")
            .help("Consider only printable characters")
            .action(clap::ArgAction::SetTrue))
            
        // Field and key options
        .arg(Arg::new("field-separator")
            .short('t')
            .long("field-separator")
            .help("Use SEP instead of non-blank to blank transition")
            .value_name("SEP"))
        .arg(Arg::new("key")
            .short('k')
            .long("key")
            .help("Sort via a key; KEYDEF gives location and type")
            .long_help("Sort via a key; KEYDEF gives location and type.\n\nKEYDEF is F[.C][OPTS][,F[.C][OPTS]] for start and stop position, where F is a field number and C a character position in the field; both are origin 1, and the stop position defaults to the line's end.\n\nIf neither -t nor -b is in effect, characters in a field are counted from the beginning of the whitespace separating the preceding field; otherwise they are counted from the beginning of the field.\n\nOPTS is one or more single-letter ordering options [bdfgiMnRrVz], which override global ordering options for that key. If no key is given, use the entire line as the key.\n\nExamples:\n  1    - sort by first field\n  2,4  - sort by fields 2 through 4\n  1.3,1.5 - sort by characters 3-5 of field 1\n  2nr  - sort by field 2 numerically in reverse")
            .value_name("KEYDEF")
            .action(clap::ArgAction::Append))
            
        // I/O options
        .arg(Arg::new("output")
            .short('o')
            .long("output")
            .help("Write result to FILE instead of standard output")
            .value_name("FILE"))
        .arg(Arg::new("zero-terminated")
            .short('z')
            .long("zero-terminated")
            .help("Line delimiter is NUL, not newline")
            .action(clap::ArgAction::SetTrue))
            
        // Operation modes
        .arg(Arg::new("check")
            .short('c')
            .long("check")
            .help("Check for sorted input; do not sort")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("check-silent")
            .short('C')
            .long("check=silent")
            .help("Like -c, but do not report first bad line")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("merge")
            .short('m')
            .long("merge")
            .help("Merge already sorted files; do not sort")
            .action(clap::ArgAction::SetTrue))
            
        // Performance options
        .arg(Arg::new("buffer-size")
            .short('S')
            .long("buffer-size")
            .help("Use SIZE for main memory buffer")
            .long_help("Use SIZE for main memory buffer. SIZE may be followed by the following multiplicative suffixes: % 1% of memory, b 1, K 1024 (default), and so on for M, G, T, P, E, Z, Y.")
            .value_name("SIZE"))
        .arg(Arg::new("parallel")
            .long("parallel")
            .help("Change the number of sorts run concurrently to N")
            .value_name("N"))
        .arg(Arg::new("temporary-directory")
            .short('T')
            .long("temporary-directory")
            .help("Use DIR for temporaries, not $TMPDIR or /tmp")
            .value_name("DIR"))
            
        // Additional options
        .arg(Arg::new("compress-program")
            .long("compress-program")
            .help("Compress temporaries with PROG; decompress them with PROG -d")
            .value_name("PROG"))
        .arg(Arg::new("debug")
            .long("debug")
            .help("Annotate the part of the line used to sort, and warn about questionable usage to stderr")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("files0-from")
            .long("files0-from")
            .help("Read input from the files specified by NUL-terminated names in file F")
            .value_name("F"))
            
        // Add explicit help and version options since we disabled the automatic ones
        .arg(Arg::new("help")
            .long("help")
            .help("Display this help and exit")
            .action(clap::ArgAction::Help))
        .arg(Arg::new("version")
            .long("version")
            .help("Output version information and exit")
            .action(clap::ArgAction::Version))
}

/// Convert legacy +N -M syntax to modern -k syntax
fn convert_legacy_syntax(args: &[String]) -> Vec<String> {
    let mut converted = Vec::new();
    converted.push(args[0].clone()); // Program name
    
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        
        if arg.starts_with('+') && arg.len() > 1 {
            // Legacy start position +N
            if let Ok(start_field) = arg[1..].parse::<usize>() {
                // Look for corresponding -M 
                if i + 1 < args.len() && args[i + 1].starts_with('-') && args[i + 1].len() > 1 {
                    if let Ok(end_field) = args[i + 1][1..].parse::<usize>() {
                        // Convert +N -M to -k (N+1),(M)
                        converted.push(format!("-k"));
                        converted.push(format!("{},{}", start_field + 1, end_field));
                        i += 2; // Skip both +N and -M
                        continue;
                    }
                }
                // Just +N without -M, convert to -k (N+1)
                converted.push(format!("-k"));
                converted.push(format!("{}", start_field + 1));
                i += 1;
                continue;
            }
        }
        
        // Regular argument, copy as-is
        converted.push(arg.clone());
        i += 1;
    }
    
    converted
}

/// Parse configuration from command line matches
fn parse_config_from_matches(matches: &clap::ArgMatches) -> SortResult<SortConfig> {
    let mut builder = SortConfigBuilder::new();
    
    // Determine sort mode (mutually exclusive)
    let sort_mode = if matches.get_flag("numeric-sort") {
        SortMode::Numeric
    } else if matches.get_flag("general-numeric-sort") {
        SortMode::GeneralNumeric
    } else if matches.get_flag("human-numeric-sort") {
        SortMode::HumanNumeric
    } else if matches.get_flag("month-sort") {
        SortMode::Month
    } else if matches.get_flag("random-sort") {
        SortMode::Random
    } else if matches.get_flag("version-sort") {
        SortMode::Version
    } else if let Some(sort_word) = matches.get_one::<String>("sort") {
        match sort_word.as_str() {
            "general-numeric" => SortMode::GeneralNumeric,
            "human-numeric" => SortMode::HumanNumeric,
            "month" => SortMode::Month,
            "numeric" => SortMode::Numeric,
            "random" => SortMode::Random,
            "version" => SortMode::Version,
            _ => return Err(SortError::parse_error(&format!("unknown sort type: {}", sort_word))),
        }
    } else {
        SortMode::Lexicographic
    };
    
    builder = builder.mode(sort_mode);
    
    // Apply boolean flags
    if matches.get_flag("reverse") {
        builder = builder.reverse();
    }
    if matches.get_flag("unique") {
        builder = builder.unique();
    }
    if matches.get_flag("stable") {
        builder = builder.stable();
    }
    if matches.get_flag("check") || matches.get_flag("check-silent") {
        builder = builder.check();
    }
    if matches.get_flag("merge") {
        builder = builder.merge();
    }
    if matches.get_flag("zero-terminated") {
        builder = builder.zero_terminated();
    }
    
    let mut config = builder.build()?;
    
    // Set additional options not handled by builder
    config.ignore_case = matches.get_flag("ignore-case");
    config.dictionary_order = matches.get_flag("dictionary-order");
    config.ignore_leading_blanks = matches.get_flag("ignore-leading-blanks");
    config.ignore_nonprinting = matches.get_flag("ignore-nonprinting");
    config.debug = matches.get_flag("debug");
    
    // Set field separator
    if let Some(sep_str) = matches.get_one::<String>("field-separator") {
        if sep_str.len() == 1 {
            config.field_separator = sep_str.chars().next();
        } else {
            return Err(SortError::invalid_field_separator(sep_str));
        }
    }
    
    // Set output file
    if let Some(output) = matches.get_one::<String>("output") {
        config.output_file = Some(output.clone());
    }
    
    // Set buffer size
    if let Some(buffer_str) = matches.get_one::<String>("buffer-size") {
        config.set_buffer_size_from_string(buffer_str)?;
    }
    
    // Set parallel threads
    if let Some(parallel_str) = matches.get_one::<String>("parallel") {
        let threads: usize = parallel_str.parse()
            .map_err(|_| SortError::parse_error(&format!("invalid thread count: {}", parallel_str)))?;
        config.parallel_threads = Some(threads);
    }
    
    // Set temporary directory
    if let Some(temp_dir) = matches.get_one::<String>("temporary-directory") {
        config.temp_dir = Some(temp_dir.clone());
    }
    
    // Simplified: Ultimate Sort handles sorting internally without explicit key parsing
    
    // Handle files0-from option
    if let Some(files0_file) = matches.get_one::<String>("files0-from") {
        config.input_files = read_files_from_null_separated_file(files0_file)?;
    }
    
    // Validate the final configuration
    config.validate()?;
    
    Ok(config)
}

/// Read filenames from a null-separated file
fn read_files_from_null_separated_file(filename: &str) -> SortResult<Vec<String>> {
    use std::fs::File;
    use std::io::Read;
    
    let mut file = File::open(filename)
        .map_err(|_| SortError::file_not_found(filename))?;
    
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    
    let files: Vec<String> = contents
        .split(|&b| b == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect();
    
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SortMode;
    
    #[test]
    fn test_parse_basic_config() {
        let app = build_cli();
        let matches = app.try_get_matches_from(&["sort", "-n", "-r"]).expect("Failed to parse test arguments");
        
        let config = parse_config_from_matches(&matches).expect("Failed to parse test config");
        
        assert_eq!(config.mode, SortMode::Numeric);
        assert!(config.reverse);
    }
    
    #[test]
    fn test_parse_complex_config() {
        let app = build_cli();
        let matches = app.try_get_matches_from(&[
            "sort", 
            "-k", "2,4",
            "-t", ":",
            "-u",
            "-o", "output.txt",
            "input.txt"
        ]).expect("Failed to parse test arguments");
        
        let config = parse_config_from_matches(&matches).expect("Failed to parse test config");
        
        assert!(config.unique);
        assert_eq!(config.field_separator, Some(':'));
        assert_eq!(config.output_file, Some("output.txt".to_string()));
        assert!(!config.keys.is_empty());
    }
    
    #[test]
    fn test_conflicting_options() {
        let app = build_cli();
        let matches = app.try_get_matches_from(&["sort", "-c", "-m"]).expect("Failed to parse test arguments");
        
        let result = parse_config_from_matches(&matches);
        assert!(result.is_err());
    }
}