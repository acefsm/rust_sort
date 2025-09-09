use crate::radix_sort::RadixSort;
use crate::simd_compare::SIMDCompare;
use crate::zero_copy::{Line, MappedFile};
use rayon::prelude::*;
use std::cmp::Ordering;
/// External sorting implementation for very large datasets
/// Uses divide-and-conquer with disk-based temporary files to handle datasets larger than RAM
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// External sorter for handling very large datasets efficiently
pub struct ExternalSort {
    /// Maximum chunk size in memory (bytes)
    max_chunk_size: usize,
    /// Whether to use parallel processing
    parallel: bool,
    /// Whether to use radix sort for numeric data
    use_radix: bool,
    /// Temporary directory for chunk files
    temp_dir: TempDir,
}

impl ExternalSort {
    /// Create new external sorter with memory limit
    pub fn new(
        max_memory_mb: usize,
        parallel: bool,
        use_radix: bool,
        temp_dir_path: Option<&str>,
    ) -> io::Result<Self> {
        let max_chunk_size = max_memory_mb * 1024 * 1024; // Convert MB to bytes

        // Create temp directory in specified location or use default
        let temp_dir = if let Some(path) = temp_dir_path {
            tempfile::tempdir_in(path)?
        } else if let Ok(tmpdir) = std::env::var("TMPDIR") {
            tempfile::tempdir_in(tmpdir)?
        } else {
            tempfile::tempdir()?
        };

        Ok(Self {
            max_chunk_size,
            parallel,
            use_radix,
            temp_dir,
        })
    }

    /// Main external sort entry point
    pub fn sort_file(
        &self,
        input_path: &Path,
        output_path: &Path,
        numeric: bool,
        unique: bool,
    ) -> io::Result<()> {
        // Step 1: Estimate file size and determine strategy
        let file_size = std::fs::metadata(input_path)?.len() as usize;

        if file_size <= self.max_chunk_size {
            // File fits in memory - use in-memory sorting
            return self.sort_in_memory(input_path, output_path, numeric, unique);
        }

        // Step 2: Split file into sorted chunks
        let chunk_files = self.create_sorted_chunks(input_path, numeric)?;

        // Step 3: Merge sorted chunks
        self.merge_sorted_chunks(&chunk_files, output_path, numeric, unique)?;

        Ok(())
    }

    /// Sort file that fits entirely in memory
    fn sort_in_memory(
        &self,
        input_path: &Path,
        output_path: &Path,
        numeric: bool,
        unique: bool,
    ) -> io::Result<()> {
        let mapped_file = MappedFile::new(input_path)?;
        let lines = mapped_file.lines();

        let mut simple_lines: Vec<Line> = lines.to_vec();

        if numeric && self.use_radix {
            let radix_sorter = RadixSort::new(self.parallel);
            radix_sorter.sort_numeric_lines(&mut simple_lines);
        } else if self.parallel && simple_lines.len() > 10000 {
            if numeric {
                simple_lines.par_sort_unstable_by(|a, b| a.compare_numeric(b));
            } else {
                simple_lines.par_sort_unstable_by(|a, b| a.compare_lexicographic(b));
            }
        } else if numeric {
            simple_lines.sort_unstable_by(|a, b| a.compare_numeric(b));
        } else {
            simple_lines.sort_unstable_by(|a, b| a.compare_lexicographic(b));
        }
        
        // Remove duplicates if unique mode
        if unique {
            simple_lines.dedup_by(|a, b| unsafe {
                a.as_bytes() == b.as_bytes()
            });
        }

        // Write sorted output
        let mut output = BufWriter::new(File::create(output_path)?);
        for line in &simple_lines {
            unsafe {
                output.write_all(line.as_bytes())?;
                output.write_all(b"\n")?;
            }
        }
        output.flush()?;

        Ok(())
    }

    /// Create sorted chunks from large input file
    fn create_sorted_chunks(&self, input_path: &Path, numeric: bool) -> io::Result<Vec<PathBuf>> {
        let file = File::open(input_path)?;
        let mut reader = BufReader::new(file);
        let mut chunk_files = Vec::new();
        let mut chunk_number = 0;

        loop {
            // Read chunk of lines that fits in memory
            let (lines, eof) = self.read_chunk_lines(&mut reader)?;
            if lines.is_empty() {
                break;
            }

            // Sort the chunk
            let sorted_lines = self.sort_chunk(lines, numeric)?;

            // Write sorted chunk to temporary file
            let chunk_path = self.write_chunk_to_file(&sorted_lines, chunk_number)?;
            chunk_files.push(chunk_path);
            chunk_number += 1;

            if eof {
                break;
            }
        }

        Ok(chunk_files)
    }

    /// Read a chunk of lines that fits in memory (optimized for large files)
    fn read_chunk_lines(&self, reader: &mut BufReader<File>) -> io::Result<(Vec<String>, bool)> {
        let mut lines = Vec::new();
        let mut total_size = 0;
        let mut line = String::new();

        // Pre-allocate capacity for better performance
        lines.reserve(self.max_chunk_size / 20); // Estimate ~20 chars per line

        while total_size < self.max_chunk_size {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;

            if bytes_read == 0 {
                // EOF reached
                return Ok((lines, true));
            }

            // Remove trailing newline
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }

            total_size += line.len();
            lines.push(std::mem::take(&mut line));
        }

        Ok((lines, false))
    }

    /// Sort a chunk using optimized algorithms for large data  
    fn sort_chunk(&self, mut lines: Vec<String>, numeric: bool) -> io::Result<Vec<String>> {
        // For large chunks, always prefer parallel sorting
        const LARGE_CHUNK_THRESHOLD: usize = 50_000;

        if numeric && self.use_radix && self.is_all_simple_integers(&lines) {
            // Use radix sort for simple integers
            self.radix_sort_strings(&mut lines)?;
        } else {
            // Use optimized comparison-based sort
            if self.parallel && lines.len() > LARGE_CHUNK_THRESHOLD {
                // For very large chunks, use parallel sort
                if numeric {
                    lines.par_sort_unstable_by(|a, b| self.compare_numeric_strings(a, b));
                } else {
                    lines.par_sort_unstable_by(|a, b| {
                        SIMDCompare::compare_bytes_simd(a.as_bytes(), b.as_bytes())
                    });
                }
            } else if lines.len() > 10_000 {
                // Medium chunks - parallel but less aggressive
                if numeric {
                    lines.par_sort_unstable_by(|a, b| self.compare_numeric_strings(a, b));
                } else {
                    lines.par_sort_unstable_by(|a, b| {
                        SIMDCompare::compare_bytes_simd(a.as_bytes(), b.as_bytes())
                    });
                }
            } else {
                // Small chunks - sequential
                if numeric {
                    lines.sort_unstable_by(|a, b| self.compare_numeric_strings(a, b));
                } else {
                    lines.sort_unstable_by(|a, b| {
                        SIMDCompare::compare_bytes_simd(a.as_bytes(), b.as_bytes())
                    });
                }
            }
        }

        Ok(lines)
    }

    /// Check if all strings are simple integers
    fn is_all_simple_integers(&self, lines: &[String]) -> bool {
        // Sample first 100 lines to determine if all are simple integers
        let sample_size = lines.len().min(100);
        lines[..sample_size].iter().all(|line| {
            SIMDCompare::is_all_digits_simd(line.as_bytes())
                || (line.starts_with('-') && SIMDCompare::is_all_digits_simd(&line.as_bytes()[1..]))
        })
    }

    /// Radix sort for string integers
    fn radix_sort_strings(&self, lines: &mut [String]) -> io::Result<()> {
        // Convert to (value, index) pairs
        let mut values: Vec<(i64, usize)> = lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let value = line.parse::<i64>().unwrap_or(0);
                (value, idx)
            })
            .collect();

        // Sort by value
        if self.parallel {
            values.par_sort_unstable_by_key(|(value, _)| *value);
        } else {
            values.sort_unstable_by_key(|(value, _)| *value);
        }

        // Reconstruct lines in sorted order
        // Create a permutation vector
        let permutation: Vec<usize> = values.into_iter().map(|(_, idx)| idx).collect();

        // Apply permutation efficiently without unnecessary cloning
        let mut sorted = Vec::with_capacity(lines.len());
        for _ in 0..lines.len() {
            sorted.push(String::new());
        }

        for (new_idx, &old_idx) in permutation.iter().enumerate() {
            sorted[new_idx] = std::mem::take(&mut lines[old_idx]);
        }

        // Replace original with sorted
        for (i, line) in sorted.into_iter().enumerate() {
            lines[i] = line;
        }

        Ok(())
    }

    /// Compare numeric strings efficiently
    fn compare_numeric_strings(&self, a: &str, b: &str) -> Ordering {
        // Fast path for simple integers
        if let (Ok(a_num), Ok(b_num)) = (a.parse::<i64>(), b.parse::<i64>()) {
            return a_num.cmp(&b_num);
        }

        // Fall back to byte-level numeric comparison
        self.compare_numeric_bytes(a.as_bytes(), b.as_bytes())
    }

    /// Byte-level numeric comparison
    fn compare_numeric_bytes(&self, a: &[u8], b: &[u8]) -> Ordering {
        // Skip leading whitespace
        let a = self.skip_whitespace(a);
        let b = self.skip_whitespace(b);

        // Handle empty strings
        match (a.is_empty(), b.is_empty()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }

        // Extract signs
        let (a_negative, a_digits) = self.extract_sign(a);
        let (b_negative, b_digits) = self.extract_sign(b);

        // Compare signs
        match (a_negative, b_negative) {
            (false, true) => return Ordering::Greater,
            (true, false) => return Ordering::Less,
            _ => {}
        }

        // Compare magnitudes
        let magnitude_cmp = self.compare_magnitude(a_digits, b_digits);

        if a_negative {
            magnitude_cmp.reverse()
        } else {
            magnitude_cmp
        }
    }

    fn skip_whitespace<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let start = bytes
            .iter()
            .position(|&b| !b.is_ascii_whitespace())
            .unwrap_or(bytes.len());
        &bytes[start..]
    }

    fn extract_sign<'a>(&self, bytes: &'a [u8]) -> (bool, &'a [u8]) {
        if bytes.starts_with(b"-") {
            (true, &bytes[1..])
        } else if bytes.starts_with(b"+") {
            (false, &bytes[1..])
        } else {
            (false, bytes)
        }
    }

    fn compare_magnitude(&self, a: &[u8], b: &[u8]) -> Ordering {
        // Remove leading zeros
        let a = self.skip_leading_zeros(a);
        let b = self.skip_leading_zeros(b);

        // Compare lengths first (longer number is bigger)
        match a.len().cmp(&b.len()) {
            Ordering::Equal => a.cmp(b), // Same length, compare lexicographically
            other => other,
        }
    }

    fn skip_leading_zeros<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let start = bytes.iter().position(|&b| b != b'0').unwrap_or(bytes.len());
        if start == bytes.len() {
            b"0" // All zeros, return single zero
        } else {
            &bytes[start..]
        }
    }

    /// Write sorted chunk to temporary file
    fn write_chunk_to_file(&self, lines: &[String], chunk_number: usize) -> io::Result<PathBuf> {
        let chunk_path = self
            .temp_dir
            .path()
            .join(format!("chunk_{chunk_number:06}.txt"));
        let mut writer = BufWriter::new(File::create(&chunk_path)?);

        for line in lines {
            writeln!(writer, "{line}")?;
        }
        writer.flush()?;

        Ok(chunk_path)
    }

    /// Merge sorted chunks using k-way merge
    fn merge_sorted_chunks(
        &self,
        chunk_files: &[PathBuf],
        output_path: &Path,
        _numeric: bool,
        unique: bool,
    ) -> io::Result<()> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        if chunk_files.is_empty() {
            return Ok(());
        }

        if chunk_files.len() == 1 {
            // Single chunk, just copy it
            std::fs::copy(&chunk_files[0], output_path)?;
            return Ok(());
        }

        // Open all chunk files
        let mut readers: Vec<BufReader<File>> = chunk_files
            .iter()
            .map(|path| File::open(path).map(BufReader::new))
            .collect::<Result<Vec<_>, _>>()?;

        let mut output = BufWriter::new(File::create(output_path)?);

        // Priority queue for k-way merge
        #[derive(Debug)]
        struct MergeItem {
            line: String,
            reader_index: usize,
        }

        impl PartialEq for MergeItem {
            fn eq(&self, other: &Self) -> bool {
                self.line == other.line
            }
        }

        impl Eq for MergeItem {}

        impl PartialOrd for MergeItem {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for MergeItem {
            fn cmp(&self, other: &Self) -> Ordering {
                // Simple lexicographic comparison (reversed for min-heap)
                self.line.cmp(&other.line).reverse()
            }
        }

        impl MergeItem {
            #[allow(dead_code)]
            fn compare_numeric(&self, other: &str) -> Ordering {
                // Fast path for simple integers
                if let (Ok(a), Ok(b)) = (self.line.parse::<i64>(), other.parse::<i64>()) {
                    return a.cmp(&b);
                }
                // Fall back to string comparison
                self.line.cmp(&other.to_string())
            }
        }

        let mut heap: BinaryHeap<Reverse<MergeItem>> = BinaryHeap::new();

        // Initialize heap with first line from each reader
        for (idx, reader) in readers.iter_mut().enumerate() {
            let mut line = String::new();
            if reader.read_line(&mut line)? > 0 {
                if line.ends_with('\n') {
                    line.pop();
                }
                heap.push(Reverse(MergeItem {
                    line,
                    reader_index: idx,
                }));
            }
        }

        // Merge process
        let mut last_line: Option<String> = None;
        while let Some(Reverse(item)) = heap.pop() {
            // If unique mode, skip duplicates
            if unique {
                if let Some(ref prev) = last_line {
                    if prev == &item.line {
                        // Skip duplicate, but still read next line from same reader
                        let reader_idx = item.reader_index;
                        let mut line = String::new();
                        if readers[reader_idx].read_line(&mut line)? > 0 {
                            if line.ends_with('\n') {
                                line.pop();
                            }
                            heap.push(Reverse(MergeItem {
                                line,
                                reader_index: reader_idx,
                            }));
                        }
                        continue;
                    }
                }
                last_line = Some(item.line.clone());
            }
            
            writeln!(output, "{}", item.line)?;

            // Read next line from the same reader
            let reader_idx = item.reader_index;
            let mut line = String::new();
            if readers[reader_idx].read_line(&mut line)? > 0 {
                if line.ends_with('\n') {
                    line.pop();
                }
                heap.push(Reverse(MergeItem {
                    line,
                    reader_index: reader_idx,
                }));
            }
        }

        output.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_external_sort_small_file() -> io::Result<()> {
        let temp_dir = TempDir::new()?;
        let input_file = temp_dir.path().join("input.txt");
        let output_file = temp_dir.path().join("output.txt");

        // Create test input
        fs::write(&input_file, "3\n1\n4\n1\n5\n9\n2\n6\n")?;

        // Sort with external sorter
        let sorter = ExternalSort::new(1, false, true, None)?; // 1MB limit
        sorter.sort_file(&input_file, &output_file, true, false)?;

        // Verify output
        let output_content = fs::read_to_string(&output_file)?;
        assert_eq!(output_content, "1\n1\n2\n3\n4\n5\n6\n9\n");

        Ok(())
    }
}
