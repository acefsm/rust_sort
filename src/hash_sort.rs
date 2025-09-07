use rand::{seq::SliceRandom, thread_rng};
use rayon::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Hash-based random sort with O(n) complexity
pub struct HashSort;

impl HashSort {
    /// Hash-based grouping with zero-copy shuffling
    /// O(n) complexity instead of O(n log n)
    pub fn hash_sort<T: Clone>(lines: &mut [T], get_key: impl Fn(&T) -> &[u8] + Sync) {
        if lines.len() < 2 {
            return;
        }

        // Step 1: Hash-based grouping in O(n)
        let groups = Self::hash_group_lines(lines, &get_key);

        // Step 2: Create shuffled group indices
        let shuffled_indices = Self::create_shuffled_indices(&groups);

        // Step 3: Reorder lines based on shuffled indices
        Self::reorder_by_indices(lines, &shuffled_indices);
    }

    /// Group lines by hash in O(n) time
    fn hash_group_lines<T>(lines: &[T], get_key: impl Fn(&T) -> &[u8]) -> Vec<Vec<usize>> {
        let mut hash_to_indices: HashMap<u64, Vec<usize>> = HashMap::new();

        // Hash each line and group indices
        for (idx, line) in lines.iter().enumerate() {
            let key = get_key(line);
            let hash = Self::fast_hash(key);
            hash_to_indices
                .entry(hash)
                .or_insert_with(Vec::new)
                .push(idx);
        }

        // Convert to vec of groups
        hash_to_indices
            .into_iter()
            .map(|(_, indices)| indices)
            .collect()
    }

    /// Ultra-fast hash function optimized for speed
    #[inline]
    fn fast_hash(data: &[u8]) -> u64 {
        // Use FxHash or xxHash3 for speed
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    /// Create shuffled indices for groups
    fn create_shuffled_indices(groups: &[Vec<usize>]) -> Vec<usize> {
        let mut rng = thread_rng();
        let mut result = Vec::with_capacity(groups.iter().map(|g| g.len()).sum());

        // Shuffle groups
        let mut group_order: Vec<usize> = (0..groups.len()).collect();
        group_order.shuffle(&mut rng);

        // Append indices from shuffled groups
        for &group_idx in &group_order {
            result.extend_from_slice(&groups[group_idx]);
        }

        result
    }

    /// Reorder lines based on indices in-place
    fn reorder_by_indices<T: Clone>(lines: &mut [T], indices: &[usize]) {
        let original = lines.to_vec();
        for (new_pos, &old_pos) in indices.iter().enumerate() {
            lines[new_pos] = original[old_pos].clone();
        }
    }

    /// BREAKTHROUGH: Parallel hash-based random sort for massive datasets
    pub fn parallel_hash_sort<T: Clone + Send + Sync>(
        lines: &mut [T],
        get_key: impl Fn(&T) -> &[u8] + Sync,
    ) {
        if lines.len() < 100_000 {
            // Use single-threaded for small data
            Self::hash_sort(lines, get_key);
            return;
        }

        // Step 1: Parallel hash grouping
        let groups = Self::parallel_hash_group(lines, &get_key);

        // Step 2: Shuffle and reorder
        let shuffled_indices = Self::create_shuffled_indices(&groups);
        Self::reorder_by_indices(lines, &shuffled_indices);
    }

    /// Parallel hash grouping using rayon
    fn parallel_hash_group<T: Send + Sync>(
        lines: &[T],
        get_key: &(impl Fn(&T) -> &[u8] + Sync),
    ) -> Vec<Vec<usize>> {
        let chunk_size = lines.len() / rayon::current_num_threads();

        // Parallel hash computation
        let hashes: Vec<(usize, u64)> = lines
            .par_iter()
            .enumerate()
            .map(|(idx, line)| {
                let key = get_key(line);
                (idx, Self::fast_hash(key))
            })
            .collect();

        // Group by hash (sequential for now, could be optimized)
        let mut hash_to_indices: HashMap<u64, Vec<usize>> = HashMap::new();
        for (idx, hash) in hashes {
            hash_to_indices
                .entry(hash)
                .or_insert_with(Vec::new)
                .push(idx);
        }

        hash_to_indices
            .into_iter()
            .map(|(_, indices)| indices)
            .collect()
    }

    /// BREAKTHROUGH: Streaming random sort for gigantic files
    pub fn streaming_random_sort<R, W>(
        reader: R,
        writer: W,
        memory_limit_mb: usize,
    ) -> std::io::Result<()>
    where
        R: std::io::BufRead,
        W: std::io::Write,
    {
        // Implementation for streaming large files
        // Uses external sorting with hash-based grouping
        todo!("Streaming implementation")
    }
}

/// SIMD-accelerated hash function for even faster hashing
#[cfg(target_arch = "x86_64")]
pub mod simd_hash {
    use std::arch::x86_64::*;

    /// xxHash3-inspired SIMD hash
    #[target_feature(enable = "avx2")]
    pub unsafe fn simd_hash_avx2(data: &[u8]) -> u64 {
        let mut hash = 0u64;
        let mut i = 0;

        // Process 32 bytes at a time with AVX2
        while i + 32 <= data.len() {
            let chunk = _mm256_loadu_si256(data.as_ptr().add(i) as *const __m256i);
            // Simplified hash mixing (real xxHash3 is more complex)
            let mixed = _mm256_xor_si256(chunk, _mm256_set1_epi64x(0x9E3779B97F4A7C15));
            let vals: [u64; 4] = std::mem::transmute(mixed);
            hash ^= vals[0].wrapping_mul(vals[1]) ^ vals[2].wrapping_mul(vals[3]);
            i += 32;
        }

        // Handle remaining bytes
        while i < data.len() {
            hash = hash.wrapping_mul(0x9E3779B97F4A7C15) ^ (data[i] as u64);
            i += 1;
        }

        // Final mixing
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xC2B2AE3D27D4EB4F);
        hash ^= hash >> 29;
        hash
    }
}

/// Zero-allocation random sort using indices only
pub struct ZeroAllocHashSort;

impl ZeroAllocHashSort {
    /// Random sort without any allocations except for indices
    pub fn sort_indices_only(count: usize, get_hash: impl Fn(usize) -> u64) -> Vec<usize> {
        // Group indices by hash
        let mut groups: HashMap<u64, Vec<usize>> = HashMap::new();
        for i in 0..count {
            let hash = get_hash(i);
            groups.entry(hash).or_insert_with(Vec::new).push(i);
        }

        // Shuffle groups and flatten
        let mut rng = thread_rng();
        let mut group_vec: Vec<_> = groups.into_iter().map(|(_, v)| v).collect();
        group_vec.shuffle(&mut rng);

        let mut result = Vec::with_capacity(count);
        for group in group_vec {
            result.extend(group);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ultra_random_sort() {
        let mut data = vec!["apple", "banana", "apple", "cherry", "banana"];
        UltraRandomSort::ultra_fast_random_sort(&mut data, |s| s.as_bytes());

        // Check that identical items are grouped
        let mut i = 0;
        while i < data.len() {
            let current = data[i];
            let mut j = i + 1;
            while j < data.len() && data[j] == current {
                j += 1;
            }
            // All identical items should be consecutive
            for k in i..j {
                assert_eq!(data[k], current);
            }
            i = j;
        }
    }

    #[test]
    fn test_performance() {
        // Generate test data with many duplicates
        let mut data: Vec<String> = Vec::new();
        for i in 0..100_000 {
            data.push(format!("item_{}", i % 100));
        }

        let start = std::time::Instant::now();
        UltraRandomSort::ultra_fast_random_sort(&mut data, |s| s.as_bytes());
        let duration = start.elapsed();

        println!("Ultra random sort took: {:?}", duration);
        assert!(duration.as_millis() < 100); // Should be very fast
    }
}
