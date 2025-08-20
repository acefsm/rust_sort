use memmap2::Mmap;
use std::path::Path;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::cmp::Ordering;
use crate::simd_compare::SIMDCompare;

/// Zero-copy line representation that points directly into memory-mapped data
#[derive(Debug, Clone, Copy)]
pub struct Line {
    /// Pointer to the start of the line in the mapped memory
    start: *const u8,
    /// Length of the line (excluding newline)
    len: u32,
}

// SAFETY: Line is safe to send between threads because:
// 1. It only contains pointers to immutable memory-mapped data
// 2. The memory-mapped files remain valid for the entire lifetime of the sort operation
// 3. No thread can mutate the underlying memory during sorting
unsafe impl Send for Line {}
unsafe impl Sync for Line {}

impl Line {
    /// Create a new Line from a slice
    pub fn new(data: &[u8]) -> Self {
        Self {
            start: data.as_ptr(),
            len: data.len() as u32,
        }
    }

    /// Get the line data as a byte slice
    /// # Safety
    /// The caller must ensure that:
    /// 1. The memory this Line points to is still valid (not freed)
    /// 2. The memory-mapped file has not been unmapped
    /// 3. No other code is mutating this memory region
    pub unsafe fn as_bytes(&self) -> &[u8] {
        // SAFETY: We create a slice from the raw pointer with the stored length.
        // The caller guarantees the memory is still valid and immutable.
        unsafe {
            std::slice::from_raw_parts(self.start, self.len as usize)
        }
    }

    /// Fast numeric parsing for simple integers (optimized path)
    pub fn parse_int(&self) -> Option<i64> {
        // SAFETY: as_bytes() is safe here because Line was created from valid memory
        // that remains valid throughout the sorting operation
        let bytes = unsafe { self.as_bytes() };
        if bytes.is_empty() { return Some(0); }
        
        let mut start = 0;
        let negative = if bytes[0] == b'-' {
            start = 1;
            true
        } else {
            false
        };
        
        if start >= bytes.len() { return None; }
        
        let mut result: i64 = 0;
        for &byte in &bytes[start..] {
            if !byte.is_ascii_digit() { return None; }
            result = result.checked_mul(10)?;
            result = result.checked_add((byte - b'0') as i64)?;
        }
        
        Some(if negative { -result } else { result })
    }

    /// Parse as general numeric (supports scientific notation, inf, nan)
    pub fn parse_general_numeric(&self) -> f64 {
        let bytes = unsafe { self.as_bytes() };
        if let Ok(s) = std::str::from_utf8(bytes) {
            let trimmed = s.trim();
            
            // Handle special cases
            if trimmed.is_empty() {
                return 0.0;
            }
            
            // Parse as float (handles scientific notation automatically)
            match trimmed.parse::<f64>() {
                Ok(val) => val,
                Err(_) => {
                    // Check for special strings
                    let lower = trimmed.to_lowercase();
                    if lower == "inf" || lower == "+inf" || lower == "infinity" {
                        f64::INFINITY
                    } else if lower == "-inf" || lower == "-infinity" {
                        f64::NEG_INFINITY
                    } else if lower == "nan" {
                        f64::NAN
                    } else {
                        // Non-numeric strings sort to beginning (like GNU sort)
                        f64::NEG_INFINITY
                    }
                }
            }
        } else {
            f64::NEG_INFINITY
        }
    }
    
    /// Compare as general numeric values (scientific notation support)
    pub fn compare_general_numeric(&self, other: &Line) -> Ordering {
        let a = self.parse_general_numeric();
        let b = other.parse_general_numeric();
        
        // Handle NaN specially (NaN sorts last in GNU sort)
        match (a.is_nan(), b.is_nan()) {
            (true, true) => unsafe { self.as_bytes().cmp(other.as_bytes()) }, // Lexicographic tie-breaker
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                // Use total_cmp for consistent ordering including -0.0 vs 0.0
                match a.total_cmp(&b) {
                    Ordering::Equal => {
                        // When numeric values are equal, use lexicographic comparison as tie-breaker
                        // This matches GNU sort behavior
                        unsafe { self.as_bytes().cmp(other.as_bytes()) }
                    }
                    other => other
                }
            }
        }
    }

    /// Fast comparison for numeric values (GNU sort style - no string conversion)
    pub fn compare_numeric(&self, other: &Line) -> Ordering {
        // Try fast path for simple integers
        if let (Some(a), Some(b)) = (self.parse_int(), other.parse_int()) {
            return a.cmp(&b);
        }
        
        // GNU sort style: compare as strings with numeric logic
        self.compare_numeric_string_style(other)
    }

    /// GNU sort-style numeric string comparison (key optimization!)
    fn compare_numeric_string_style(&self, other: &Line) -> Ordering {
        let a_bytes = unsafe { self.as_bytes() };
        let b_bytes = unsafe { other.as_bytes() };
        
        // Skip leading whitespace
        let a_start = self.skip_leading_space(a_bytes);
        let b_start = self.skip_leading_space(b_bytes);
        
        if a_start >= a_bytes.len() && b_start >= b_bytes.len() { return Ordering::Equal; }
        if a_start >= a_bytes.len() { return Ordering::Less; }
        if b_start >= b_bytes.len() { return Ordering::Greater; }
        
        let a_rest = &a_bytes[a_start..];
        let b_rest = &b_bytes[b_start..];
        
        // Check signs
        let (a_negative, a_num_start) = self.parse_sign(a_rest);
        let (b_negative, b_num_start) = self.parse_sign(b_rest);
        
        match (a_negative, b_negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                // Same sign - compare magnitudes
                let a_digits = &a_rest[a_num_start..];
                let b_digits = &b_rest[b_num_start..];
                
                // Skip leading zeros (GNU sort behavior)
                let a_no_zeros = self.skip_leading_zeros(a_digits);
                let b_no_zeros = self.skip_leading_zeros(b_digits);
                
                // Compare by digit count first (major optimization!)
                let a_digit_count = self.count_leading_digits(&a_digits[a_no_zeros..]);
                let b_digit_count = self.count_leading_digits(&b_digits[b_no_zeros..]);
                
                let magnitude_cmp = match a_digit_count.cmp(&b_digit_count) {
                    Ordering::Equal => {
                        // Same digit count - lexicographic comparison
                        a_digits[a_no_zeros..a_no_zeros + a_digit_count]
                            .cmp(&b_digits[b_no_zeros..b_no_zeros + b_digit_count])
                    }
                    other => other,
                };
                
                if a_negative { magnitude_cmp.reverse() } else { magnitude_cmp }
            }
        }
    }

    fn skip_leading_space(&self, bytes: &[u8]) -> usize {
        bytes.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(bytes.len())
    }

    fn parse_sign(&self, bytes: &[u8]) -> (bool, usize) {
        if bytes.is_empty() { return (false, 0); }
        match bytes[0] {
            b'-' => (true, 1),
            b'+' => (false, 1),
            _ => (false, 0),
        }
    }

    fn skip_leading_zeros(&self, bytes: &[u8]) -> usize {
        bytes.iter().position(|&b| b != b'0').unwrap_or(bytes.len())
    }

    fn count_leading_digits(&self, bytes: &[u8]) -> usize {
        bytes.iter().take_while(|&&b| b.is_ascii_digit()).count()
    }

    /// Byte-level numeric comparison for complex numbers
    fn compare_numeric_bytes(&self, other: &Line) -> Ordering {
        let a_bytes = unsafe { self.as_bytes() };
        let b_bytes = unsafe { other.as_bytes() };
        
        // Skip leading whitespace
        let a_trimmed = a_bytes.iter().skip_while(|&&b| b == b' ' || b == b'\t').collect::<Vec<_>>();
        let b_trimmed = b_bytes.iter().skip_while(|&&b| b == b' ' || b == b'\t').collect::<Vec<_>>();
        
        if a_trimmed.is_empty() && b_trimmed.is_empty() { return Ordering::Equal; }
        if a_trimmed.is_empty() { return Ordering::Less; }
        if b_trimmed.is_empty() { return Ordering::Greater; }
        
        // Compare signs
        let a_negative = *a_trimmed[0] == b'-';
        let b_negative = *b_trimmed[0] == b'-';
        
        match (a_negative, b_negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                // Same sign, compare magnitudes
                let a_digits: Vec<u8> = a_trimmed.iter()
                    .skip_while(|&&&b| b == b'-' || b == b'+')
                    .filter(|&&&b| b.is_ascii_digit())
                    .map(|&&b| b)
                    .collect();
                let b_digits: Vec<u8> = b_trimmed.iter()
                    .skip_while(|&&&b| b == b'-' || b == b'+')
                    .filter(|&&&b| b.is_ascii_digit())
                    .map(|&&b| b)
                    .collect();
                
                let magnitude_cmp = match a_digits.len().cmp(&b_digits.len()) {
                    Ordering::Equal => a_digits.cmp(&b_digits),
                    other => other,
                };
                
                if a_negative { magnitude_cmp.reverse() } else { magnitude_cmp }
            }
        }
    }

    /// Get the length of the line
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Check if the line is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// SIMD-accelerated case-insensitive comparison
    pub fn compare_ignore_case(&self, other: &Line) -> Ordering {
        let a_bytes = unsafe { self.as_bytes() };
        let b_bytes = unsafe { other.as_bytes() };
        
        // Use SIMD for performance boost
        SIMDCompare::compare_case_insensitive_simd(a_bytes, b_bytes)
    }

    /// SIMD-accelerated lexicographic comparison
    pub fn compare_lexicographic(&self, other: &Line) -> Ordering {
        let a_bytes = unsafe { self.as_bytes() };
        let b_bytes = unsafe { other.as_bytes() };
        
        // Use SIMD for maximum performance
        SIMDCompare::compare_bytes_simd(a_bytes, b_bytes)
    }
}

/// Memory-mapped file with parsed lines
pub struct MappedFile {
    _mmap: Mmap, // Keep mmap alive
    lines: Vec<Line>,
}

impl MappedFile {
    /// Create a new SimpleMappedFile from a file path
    pub fn new(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        
        // Parse lines while keeping references to the mmap
        let lines = parse_lines(&mmap);
        
        Ok(Self {
            _mmap: mmap,
            lines,
        })
    }

    /// Get the lines in this file
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
}

/// Fast line parsing that creates Line structs pointing into the mmap'd data
fn parse_lines(data: &[u8]) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0;
    
    for (i, &byte) in data.iter().enumerate() {
        if byte == b'\n' {
            let line_data = &data[start..i];
            lines.push(Line::new(line_data));
            start = i + 1;
        }
    }
    
    // Handle last line if it doesn't end with newline
    if start < data.len() {
        let line_data = &data[start..];
        lines.push(Line::new(line_data));
    }
    
    lines
}

/// Zero-copy line reader for streaming large files
pub struct ZeroCopyReader {
    reader: BufReader<File>,
    buffer: Vec<u8>,
    lines: Vec<Line>,
}

impl ZeroCopyReader {
    pub fn new(file: File) -> Self {
        Self {
            reader: BufReader::new(file),
            buffer: Vec::with_capacity(64 * 1024), // 64KB buffer
            lines: Vec::new(),
        }
    }

    /// Read next chunk of lines, reusing the internal buffer
    pub fn read_chunk(&mut self) -> io::Result<&[Line]> {
        self.buffer.clear();
        self.lines.clear();
        
        let mut total_read = 0;
        const CHUNK_SIZE: usize = 64 * 1024;
        
        // Read up to CHUNK_SIZE bytes
        while total_read < CHUNK_SIZE {
            let mut line_buf = Vec::new();
            let bytes_read = self.reader.read_until(b'\n', &mut line_buf)?;
            
            if bytes_read == 0 {
                break; // EOF
            }
            
            let start_idx = self.buffer.len();
            self.buffer.extend_from_slice(&line_buf);
            
            // Remove trailing newline if present
            let end_idx = if line_buf.ends_with(&[b'\n']) {
                self.buffer.len() - 1
            } else {
                self.buffer.len()
            };
            
            let line_data = &self.buffer[start_idx..end_idx];
            self.lines.push(Line::new(line_data));
            
            total_read += bytes_read;
        }
        
        Ok(&self.lines)
    }
}

/// Optimized numeric comparison for Line structs
pub fn compare_numeric_lines(a: &Line, b: &Line) -> Ordering {
    unsafe {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();
        
        // Fast path for simple integer comparison
        if let (Some(a_num), Some(b_num)) = (parse_int(a_bytes), parse_int(b_bytes)) {
            return a_num.cmp(&b_num);
        }
        
        // Fall back to lexicographic comparison for complex numbers
        compare_numeric_bytes(a_bytes, b_bytes)
    }
}

/// Fast integer parsing for simple cases (digits only, no signs/decimals)
fn parse_int(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() {
        return Some(0);
    }
    
    let mut result: i64 = 0;
    let mut negative = false;
    let mut start = 0;
    
    // Handle leading sign
    if bytes[0] == b'-' {
        negative = true;
        start = 1;
    } else if bytes[0] == b'+' {
        start = 1;
    }
    
    // Parse digits
    for &byte in &bytes[start..] {
        if !byte.is_ascii_digit() {
            return None; // Not a simple integer
        }
        
        result = result.checked_mul(10)?;
        result = result.checked_add((byte - b'0') as i64)?;
    }
    
    if negative {
        result = -result;
    }
    
    Some(result)
}

/// Numeric comparison for complex numbers (with decimals, scientific notation, etc.)
fn compare_numeric_bytes(a: &[u8], b: &[u8]) -> Ordering {
    // Skip leading whitespace
    let a = skip_whitespace(a);
    let b = skip_whitespace(b);
    
    // Handle empty strings
    match (a.is_empty(), b.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        (false, false) => {
            // Continue with comparison
        }
    }
    
    // Extract signs
    let (a_negative, a_digits) = extract_sign(a);
    let (b_negative, b_digits) = extract_sign(b);
    
    // Compare signs
    match (a_negative, b_negative) {
        (false, true) => return Ordering::Greater,
        (true, false) => return Ordering::Less,
        _ => {}
    }
    
    // Both have same sign, compare magnitudes
    let magnitude_cmp = compare_magnitude(a_digits, b_digits);
    
    if a_negative {
        magnitude_cmp.reverse()
    } else {
        magnitude_cmp
    }
}

fn skip_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
    &bytes[start..]
}

fn extract_sign(bytes: &[u8]) -> (bool, &[u8]) {
    if bytes.starts_with(&[b'-']) {
        (true, &bytes[1..])
    } else if bytes.starts_with(&[b'+']) {
        (false, &bytes[1..])
    } else {
        (false, bytes)
    }
}

fn compare_magnitude(a: &[u8], b: &[u8]) -> Ordering {
    // Find decimal points
    let a_dot = a.iter().position(|&b| b == b'.');
    let b_dot = b.iter().position(|&b| b == b'.');
    
    let (a_int, a_frac) = match a_dot {
        Some(pos) => (&a[..pos], &a[pos + 1..]),
        None => (a, &[][..]),
    };
    
    let (b_int, b_frac) = match b_dot {
        Some(pos) => (&b[..pos], &b[pos + 1..]),
        None => (b, &[][..]),
    };
    
    // Compare integer parts
    let int_cmp = compare_integer_parts(a_int, b_int);
    if int_cmp != Ordering::Equal {
        return int_cmp;
    }
    
    // Compare fractional parts
    compare_fractional_parts(a_frac, b_frac)
}

fn compare_integer_parts(a: &[u8], b: &[u8]) -> Ordering {
    // Remove leading zeros
    let a = skip_leading_zeros(a);
    let b = skip_leading_zeros(b);
    
    // Compare lengths first
    let len_cmp = a.len().cmp(&b.len());
    if len_cmp != Ordering::Equal {
        return len_cmp;
    }
    
    // Same length, compare digit by digit
    a.cmp(b)
}

fn compare_fractional_parts(a: &[u8], b: &[u8]) -> Ordering {
    let max_len = a.len().max(b.len());
    
    for i in 0..max_len {
        let a_digit = a.get(i).copied().unwrap_or(b'0');
        let b_digit = b.get(i).copied().unwrap_or(b'0');
        
        let cmp = a_digit.cmp(&b_digit);
        if cmp != Ordering::Equal {
            return cmp;
        }
    }
    
    Ordering::Equal
}

fn skip_leading_zeros(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| b != b'0').unwrap_or(bytes.len());
    if start == bytes.len() {
        &[b'0'] // All zeros, return single zero
    } else {
        &bytes[start..]
    }
}

/// Fast case-insensitive comparison
pub fn compare_case_insensitive(a: &[u8], b: &[u8]) -> Ordering {
    let min_len = a.len().min(b.len());
    
    for i in 0..min_len {
        let a_char = a[i].to_ascii_lowercase();
        let b_char = b[i].to_ascii_lowercase();
        
        match a_char.cmp(&b_char) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    
    a.len().cmp(&b.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_line_creation() {
        let data = b"hello world";
        let line = Line::new(data);
        
        unsafe {
            assert_eq!(line.as_bytes(), b"hello world");
        }
        assert_eq!(line.len(), 11);
    }
    
    #[test]
    fn test_numeric_comparison() {
        let a = Line::new(b"123");
        let b = Line::new(b"456");
        let c = Line::new(b"123");
        
        assert_eq!(compare_numeric_lines(&a, &b), Ordering::Less);
        assert_eq!(compare_numeric_lines(&b, &a), Ordering::Greater);
        assert_eq!(compare_numeric_lines(&a, &c), Ordering::Equal);
    }
    
    #[test]
    fn test_simple_int_parsing() {
        assert_eq!(parse_int(b"123"), Some(123));
        assert_eq!(parse_int(b"-456"), Some(-456));
        assert_eq!(parse_int(b"+789"), Some(789));
        assert_eq!(parse_int(b"0"), Some(0));
        assert_eq!(parse_int(b""), Some(0));
        assert_eq!(parse_int(b"12.34"), None); // Not simple
        assert_eq!(parse_int(b"abc"), None); // Not numeric
    }
}