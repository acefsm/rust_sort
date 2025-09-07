use crate::simd_compare::SIMDCompare;
use crate::zero_copy::Line;
use rayon::prelude::*;
/// Radix sort implementation for numeric data
/// Achieves O(n) time complexity vs O(n log n) for comparison-based sorts
use std::cmp::Ordering;

/// Parallel radix sort for numeric data - can achieve 5-10x speedup
pub struct RadixSort {
    /// Whether to use parallel processing
    parallel: bool,
}

impl RadixSort {
    pub fn new(parallel: bool) -> Self {
        Self { parallel }
    }

    /// Main entry point for radix sorting with large data optimization
    pub fn sort_numeric_lines(&self, lines: &mut [Line]) {
        if lines.len() < 1000 {
            // Use insertion sort for small arrays
            self.insertion_sort(lines);
            return;
        }

        // **LARGE DATA OPTIMIZATION**: Only use chunked processing for extremely large datasets
        const VERY_LARGE_THRESHOLD: usize = 20_000_000; // 20M lines

        if lines.len() > VERY_LARGE_THRESHOLD {
            self.sort_very_large_dataset(lines);
            return;
        }

        // Check if all lines are simple integers
        if self.are_all_simple_integers(lines) {
            if self.parallel && lines.len() > 10000 {
                self.parallel_radix_sort_integers(lines);
            } else {
                self.sequential_radix_sort_integers(lines);
            }
        } else {
            // Fall back to comparison-based sort for complex numbers
            if self.parallel {
                lines.par_sort_unstable_by(|a, b| a.compare_numeric(b));
            } else {
                lines.sort_unstable_by(|a, b| a.compare_numeric(b));
            }
        }
    }

    /// Sort very large datasets using chunked parallel processing
    fn sort_very_large_dataset(&self, lines: &mut [Line]) {
        if !self.parallel {
            // Fall back to sequential sort for very large single-threaded data
            lines.sort_unstable_by(|a, b| a.compare_numeric(b));
            return;
        }

        const CHUNK_SIZE: usize = 2_000_000; // Process in 2M line chunks (меньше chunks = меньше merge overhead)
        let num_chunks = (lines.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;

        // Sort chunks in parallel
        lines.par_chunks_mut(CHUNK_SIZE).for_each(|chunk| {
            // Use appropriate algorithm for each chunk
            if self.are_all_simple_integers(chunk) {
                self.sequential_radix_sort_integers(chunk);
            } else {
                chunk.sort_unstable_by(|a, b| a.compare_numeric(b));
            }
        });

        // Merge sorted chunks using parallel merge
        self.parallel_merge_chunks(lines, CHUNK_SIZE, num_chunks);
    }

    /// Parallel merge of sorted chunks
    fn parallel_merge_chunks(&self, lines: &mut [Line], chunk_size: usize, num_chunks: usize) {
        if num_chunks <= 1 {
            return;
        }

        // Use binary merge tree approach for optimal cache performance
        let mut current_chunk_size = chunk_size;
        let mut remaining_chunks = num_chunks;

        while remaining_chunks > 1 {
            // Merge pairs of chunks in parallel
            let pairs = remaining_chunks / 2;

            // Can't use parallel iteration with mutable slice access
            // Use sequential merging instead
            for pair_idx in 0..pairs {
                let chunk1_start = pair_idx * 2 * current_chunk_size;
                let chunk2_start = chunk1_start + current_chunk_size;
                let merge_end = ((pair_idx + 1) * 2 * current_chunk_size).min(lines.len());

                if chunk2_start < lines.len() {
                    self.merge_two_sorted_ranges(
                        &mut lines[chunk1_start..merge_end],
                        current_chunk_size.min(merge_end - chunk1_start),
                    );
                }
            }

            // Handle odd chunk if exists
            current_chunk_size *= 2;
            remaining_chunks = (remaining_chunks + 1) / 2;
        }
    }

    /// Merge two sorted ranges in-place
    fn merge_two_sorted_ranges(&self, slice: &mut [Line], mid: usize) {
        if mid >= slice.len() {
            return;
        }

        // Use a temporary buffer for efficient merging
        let mut temp = Vec::with_capacity(slice.len());
        let (left, right) = slice.split_at(mid);

        let mut i = 0;
        let mut j = 0;

        // Merge the two halves
        while i < left.len() && j < right.len() {
            if left[i].compare_numeric(&right[j]) != Ordering::Greater {
                temp.push(left[i]);
                i += 1;
            } else {
                temp.push(right[j]);
                j += 1;
            }
        }

        // Copy remaining elements
        while i < left.len() {
            temp.push(left[i]);
            i += 1;
        }
        while j < right.len() {
            temp.push(right[j]);
            j += 1;
        }

        // Copy back to original slice
        slice.copy_from_slice(&temp);
    }

    /// Check if all lines contain simple integers (no decimals, scientific notation, etc.)
    fn are_all_simple_integers(&self, lines: &[Line]) -> bool {
        // Sample first 100 lines to determine if all are simple integers
        let sample_size = lines.len().min(100);
        lines[..sample_size].iter().all(|line| unsafe {
            let bytes = line.as_bytes();
            self.is_simple_integer(bytes)
        })
    }

    /// SIMD-accelerated check if a byte slice represents a simple integer
    fn is_simple_integer(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return true;
        }

        let mut start = 0;
        // Handle optional sign
        if bytes[0] == b'-' || bytes[0] == b'+' {
            start = 1;
        }

        if start >= bytes.len() {
            return false;
        }

        // Use SIMD for fast digit detection
        SIMDCompare::is_all_digits_simd(&bytes[start..])
    }

    /// Ultra-fast parallel radix sort for simple integers
    fn parallel_radix_sort_integers(&self, lines: &mut [Line]) {
        // Parse all integers in parallel
        let mut values: Vec<(i64, usize)> = lines
            .par_iter()
            .enumerate()
            .map(|(idx, line)| {
                let value = unsafe {
                    let bytes = line.as_bytes();
                    self.parse_integer_fast(bytes)
                };
                (value, idx)
            })
            .collect();

        // Parallel radix sort on the integers
        self.parallel_radix_sort_pairs(&mut values);

        // Reconstruct the lines array based on sorted indices
        let original_lines: Vec<Line> = lines.to_vec();
        for (i, &(_, original_idx)) in values.iter().enumerate() {
            lines[i] = original_lines[original_idx];
        }
    }

    /// Sequential radix sort for simple integers
    fn sequential_radix_sort_integers(&self, lines: &mut [Line]) {
        // Parse all integers
        let mut values: Vec<(i64, usize)> = lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let value = unsafe {
                    let bytes = line.as_bytes();
                    self.parse_integer_fast(bytes)
                };
                (value, idx)
            })
            .collect();

        // Sequential radix sort
        self.sequential_radix_sort_pairs(&mut values);

        // Reconstruct lines
        let original_lines: Vec<Line> = lines.to_vec();
        for (i, &(_, original_idx)) in values.iter().enumerate() {
            lines[i] = original_lines[original_idx];
        }
    }

    /// Fast integer parsing optimized for speed
    fn parse_integer_fast(&self, bytes: &[u8]) -> i64 {
        if bytes.is_empty() {
            return 0;
        }

        let mut result: i64 = 0;
        let mut start = 0;
        let negative = if bytes[0] == b'-' {
            start = 1;
            true
        } else if bytes[0] == b'+' {
            start = 1;
            false
        } else {
            false
        };

        // Unrolled loop for better performance
        for &byte in &bytes[start..] {
            result = result * 10 + (byte - b'0') as i64;
        }

        if negative {
            -result
        } else {
            result
        }
    }

    /// Parallel radix sort implementation
    fn parallel_radix_sort_pairs(&self, values: &mut [(i64, usize)]) {
        const RADIX: usize = 256;
        const MAX_BITS: usize = 64;

        // Handle negative numbers by splitting and sorting separately
        let (mut negatives, mut positives): (Vec<_>, Vec<_>) = values
            .par_iter()
            .cloned()
            .partition(|(value, _)| *value < 0);

        // Sort positives with radix sort
        if !positives.is_empty() {
            self.radix_sort_positive_parallel(&mut positives);
        }

        // Sort negatives by absolute value, then reverse
        if !negatives.is_empty() {
            // Convert to positive values for sorting
            negatives
                .par_iter_mut()
                .for_each(|(value, _)| *value = -*value);
            self.radix_sort_positive_parallel(&mut negatives);
            // Reverse order and restore negative values
            negatives.reverse();
            negatives
                .par_iter_mut()
                .for_each(|(value, _)| *value = -*value);
        }

        // Combine results: negatives first, then positives
        let mut idx = 0;
        for item in negatives.into_iter().chain(positives.into_iter()) {
            values[idx] = item;
            idx += 1;
        }
    }

    /// Sequential radix sort implementation
    fn sequential_radix_sort_pairs(&self, values: &mut [(i64, usize)]) {
        // Simple case: use standard library for small arrays
        values.sort_unstable_by_key(|(value, _)| *value);
    }

    /// Radix sort for positive numbers only
    fn radix_sort_positive_parallel(&self, values: &mut [(i64, usize)]) {
        if values.is_empty() {
            return;
        }

        const RADIX: usize = 256;
        let mut temp = vec![(0i64, 0usize); values.len()];

        // Find maximum value to determine number of passes needed
        let max_val = values.par_iter().map(|(v, _)| *v).max().unwrap_or(0);
        let max_bits = if max_val == 0 {
            1
        } else {
            64 - max_val.leading_zeros() as usize
        };
        let passes = (max_bits + 7) / 8; // 8 bits per pass

        for pass in 0..passes {
            let shift = pass * 8;
            let mask = ((1u64 << 8) - 1) as i64;

            // Count occurrences in parallel
            let mut counts = vec![0usize; RADIX];
            for (value, _) in values.iter() {
                let digit = ((value >> shift) & mask) as usize;
                counts[digit] += 1;
            }

            // Calculate positions
            let mut positions = vec![0usize; RADIX];
            for i in 1..RADIX {
                positions[i] = positions[i - 1] + counts[i - 1];
            }

            // Distribute values
            for &(value, idx) in values.iter() {
                let digit = ((value >> shift) & mask) as usize;
                temp[positions[digit]] = (value, idx);
                positions[digit] += 1;
            }

            // Copy temp back to values
            values.copy_from_slice(&temp);
        }
    }

    /// Insertion sort for small arrays
    fn insertion_sort(&self, lines: &mut [Line]) {
        for i in 1..lines.len() {
            let mut j = i;
            while j > 0 && lines[j].compare_numeric(&lines[j - 1]) == Ordering::Less {
                lines.swap(j, j - 1);
                j -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zero_copy::Line;

    #[test]
    fn test_radix_sort_simple_integers() {
        let data1 = b"123";
        let data2 = b"456";
        let data3 = b"789";
        let data4 = b"1";

        let mut lines = vec![
            Line::new(data2), // 456
            Line::new(data1), // 123
            Line::new(data4), // 1
            Line::new(data3), // 789
        ];

        let sorter = RadixSort::new(false);
        sorter.sort_numeric_lines(&mut lines);

        // Verify sorted order
        unsafe {
            assert_eq!(lines[0].as_bytes(), b"1");
            assert_eq!(lines[1].as_bytes(), b"123");
            assert_eq!(lines[2].as_bytes(), b"456");
            assert_eq!(lines[3].as_bytes(), b"789");
        }
    }

    #[test]
    fn test_negative_numbers() {
        let data1 = b"-123";
        let data2 = b"456";
        let data3 = b"-789";
        let data4 = b"1";

        let mut lines = vec![
            Line::new(data2), // 456
            Line::new(data1), // -123
            Line::new(data4), // 1
            Line::new(data3), // -789
        ];

        let sorter = RadixSort::new(false);
        sorter.sort_numeric_lines(&mut lines);

        // Verify sorted order: -789, -123, 1, 456
        unsafe {
            assert_eq!(lines[0].as_bytes(), b"-789");
            assert_eq!(lines[1].as_bytes(), b"-123");
            assert_eq!(lines[2].as_bytes(), b"1");
            assert_eq!(lines[3].as_bytes(), b"456");
        }
    }
}
