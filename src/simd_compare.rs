/// SIMD-accelerated comparison functions for ultra-fast string operations
/// Uses vectorized instructions to process 32-64 bytes at once
use std::cmp::Ordering;

/// SIMD-accelerated string comparison
pub struct SIMDCompare;

impl SIMDCompare {
    /// Vectorized byte comparison using SIMD when available
    #[inline]
    pub fn compare_bytes_simd(a: &[u8], b: &[u8]) -> Ordering {
        // For small strings, use direct comparison
        if a.len() <= 16 || b.len() <= 16 {
            return a.cmp(b);
        }

        // Use SIMD for larger strings
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                return Self::compare_avx2(a, b);
            } else if is_x86_feature_detected!("sse4.2") {
                return Self::compare_sse42(a, b);
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if std::arch::is_aarch64_feature_detected!("neon") {
                return Self::compare_neon(a, b);
            }
        }

        // Fallback to standard comparison
        a.cmp(b)
    }

    /// Vectorized case-insensitive comparison
    #[inline]
    pub fn compare_case_insensitive_simd(a: &[u8], b: &[u8]) -> Ordering {
        let min_len = a.len().min(b.len());
        
        // Process in chunks of 32 bytes for AVX2
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") && min_len >= 32 {
                return Self::compare_case_insensitive_avx2(a, b);
            }
        }

        // Fallback to byte-by-byte comparison
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

    /// AVX2-accelerated byte comparison
    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn compare_avx2(a: &[u8], b: &[u8]) -> Ordering {
        use std::arch::x86_64::*;
        
        let min_len = a.len().min(b.len());
        let chunk_size = 32; // AVX2 processes 32 bytes at once
        let chunks = min_len / chunk_size;
        
        // Safety check: ensure we have enough data for SIMD
        if chunks == 0 {
            return a.cmp(b);  // Fallback to standard comparison
        }
        
        unsafe {
            for i in 0..chunks {
                let offset = i * chunk_size;
                
                // Load 32 bytes from each array
                // SAFETY: We use unaligned loads (_loadu) which are safe for any alignment
                // The offset is guaranteed to be within bounds by the chunks calculation
                let va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i);
                let vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i);
                
                // Compare vectors
                let cmp = _mm256_cmpeq_epi8(va, vb);
                let mask = _mm256_movemask_epi8(cmp) as u32;
                
                // If not all bytes are equal, find first difference
                if mask != 0xFFFFFFFF {
                    let diff_pos = (!mask).trailing_zeros() as usize;
                    let abs_pos = offset + diff_pos;
                    return a[abs_pos].cmp(&b[abs_pos]);
                }
            }
        }
        
        // Compare remaining bytes
        let remaining_start = chunks * chunk_size;
        for i in remaining_start..min_len {
            match a[i].cmp(&b[i]) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        
        a.len().cmp(&b.len())
    }

    /// SSE4.2-accelerated byte comparison
    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn compare_sse42(a: &[u8], b: &[u8]) -> Ordering {
        use std::arch::x86_64::*;
        
        let min_len = a.len().min(b.len());
        let chunk_size = 16; // SSE processes 16 bytes at once
        let chunks = min_len / chunk_size;
        
        unsafe {
            for i in 0..chunks {
                let offset = i * chunk_size;
                
                // Load 16 bytes from each array
                let va = _mm_loadu_si128(a.as_ptr().add(offset) as *const __m128i);
                let vb = _mm_loadu_si128(b.as_ptr().add(offset) as *const __m128i);
                
                // Compare vectors
                let cmp = _mm_cmpeq_epi8(va, vb);
                let mask = _mm_movemask_epi8(cmp) as u16;
                
                // If not all bytes are equal, find first difference
                if mask != 0xFFFF {
                    let diff_pos = (!mask).trailing_zeros() as usize;
                    let abs_pos = offset + diff_pos;
                    return a[abs_pos].cmp(&b[abs_pos]);
                }
            }
        }
        
        // Compare remaining bytes
        let remaining_start = chunks * chunk_size;
        for i in remaining_start..min_len {
            match a[i].cmp(&b[i]) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        
        a.len().cmp(&b.len())
    }

    /// ARM NEON-accelerated byte comparison
    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn compare_neon(a: &[u8], b: &[u8]) -> Ordering {
        use std::arch::aarch64::*;
        
        let min_len = a.len().min(b.len());
        let chunk_size = 16; // NEON processes 16 bytes at once
        let chunks = min_len / chunk_size;
        
        unsafe {
            for i in 0..chunks {
                let offset = i * chunk_size;
                
                // Load 16 bytes from each array
                let va = vld1q_u8(a.as_ptr().add(offset));
                let vb = vld1q_u8(b.as_ptr().add(offset));
                
                // Compare vectors
                let cmp = vceqq_u8(va, vb);
                
                // Check if all lanes are equal
                let all_equal = vminvq_u8(cmp) == 0xFF;
                if !all_equal {
                    // Find first difference
                    for j in 0..16 {
                        let pos = offset + j;
                        if a[pos] != b[pos] {
                            return a[pos].cmp(&b[pos]);
                        }
                    }
                }
            }
        }
        
        // Compare remaining bytes
        let remaining_start = chunks * chunk_size;
        for i in remaining_start..min_len {
            match a[i].cmp(&b[i]) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        
        a.len().cmp(&b.len())
    }

    /// AVX2-accelerated case-insensitive comparison
    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn compare_case_insensitive_avx2(a: &[u8], b: &[u8]) -> Ordering {
        use std::arch::x86_64::*;
        
        let min_len = a.len().min(b.len());
        let chunk_size = 32;
        let chunks = min_len / chunk_size;
        
        unsafe {
            // Broadcast constants for case conversion
            let upper_a = _mm256_set1_epi8(b'A' as i8);
            let upper_z = _mm256_set1_epi8(b'Z' as i8);
            let case_diff = _mm256_set1_epi8(32);
            
            for i in 0..chunks {
                let offset = i * chunk_size;
                
                // Load 32 bytes from each array
                let mut va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i);
                let mut vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i);
                
                // Convert to lowercase using SIMD
                let a_is_upper = _mm256_and_si256(
                    _mm256_cmpgt_epi8(va, _mm256_sub_epi8(upper_a, _mm256_set1_epi8(1))),
                    _mm256_cmpgt_epi8(_mm256_add_epi8(upper_z, _mm256_set1_epi8(1)), va)
                );
                let b_is_upper = _mm256_and_si256(
                    _mm256_cmpgt_epi8(vb, _mm256_sub_epi8(upper_a, _mm256_set1_epi8(1))),
                    _mm256_cmpgt_epi8(_mm256_add_epi8(upper_z, _mm256_set1_epi8(1)), vb)
                );
                
                va = _mm256_add_epi8(va, _mm256_and_si256(a_is_upper, case_diff));
                vb = _mm256_add_epi8(vb, _mm256_and_si256(b_is_upper, case_diff));
                
                // Compare converted vectors
                let cmp = _mm256_cmpeq_epi8(va, vb);
                let mask = _mm256_movemask_epi8(cmp) as u32;
                
                // If not all bytes are equal, find first difference
                if mask != 0xFFFFFFFF {
                    let diff_pos = (!mask).trailing_zeros() as usize;
                    let abs_pos = offset + diff_pos;
                    return a[abs_pos].to_ascii_lowercase().cmp(&b[abs_pos].to_ascii_lowercase());
                }
            }
        }
        
        // Compare remaining bytes
        let remaining_start = chunks * chunk_size;
        for i in remaining_start..min_len {
            let a_char = a[i].to_ascii_lowercase();
            let b_char = b[i].to_ascii_lowercase();
            match a_char.cmp(&b_char) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        
        a.len().cmp(&b.len())
    }

    /// Fast numeric comparison using SIMD digit detection
    #[inline]
    pub fn is_all_digits_simd(bytes: &[u8]) -> bool {
        if bytes.is_empty() { return true; }
        
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") && bytes.len() >= 32 {
                return Self::is_all_digits_avx2(bytes);
            }
        }
        
        // Fallback
        bytes.iter().all(|&b| b.is_ascii_digit())
    }

    /// AVX2-accelerated digit detection
    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn is_all_digits_avx2(bytes: &[u8]) -> bool {
        use std::arch::x86_64::*;
        
        let chunk_size = 32;
        let chunks = bytes.len() / chunk_size;
        
        unsafe {
            let min_digit = _mm256_set1_epi8(b'0' as i8);
            let max_digit = _mm256_set1_epi8(b'9' as i8);
            
            for i in 0..chunks {
                let offset = i * chunk_size;
                let v = _mm256_loadu_si256(bytes.as_ptr().add(offset) as *const __m256i);
                
                // Check if all bytes are in range '0'..'9'
                let ge_min = _mm256_cmpgt_epi8(v, _mm256_sub_epi8(min_digit, _mm256_set1_epi8(1)));
                let le_max = _mm256_cmpgt_epi8(_mm256_add_epi8(max_digit, _mm256_set1_epi8(1)), v);
                let is_digit = _mm256_and_si256(ge_min, le_max);
                
                let mask = _mm256_movemask_epi8(is_digit) as u32;
                if mask != 0xFFFFFFFF {
                    return false;
                }
            }
        }
        
        // Check remaining bytes
        let remaining_start = chunks * chunk_size;
        bytes[remaining_start..].iter().all(|&b| b.is_ascii_digit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_comparison() {
        let a = b"hello world this is a test";
        let b = b"hello world this is a different test";
        
        let result = SIMDCompare::compare_bytes_simd(a, b);
        let expected = a.cmp(b);
        
        assert_eq!(result, expected);
    }

    #[test]
    fn test_simd_case_insensitive() {
        let a = b"Hello World";
        let b = b"HELLO WORLD";
        
        let result = SIMDCompare::compare_case_insensitive_simd(a, b);
        assert_eq!(result, Ordering::Equal);
    }

    #[test]
    fn test_simd_digit_detection() {
        assert!(SIMDCompare::is_all_digits_simd(b"123456789"));
        assert!(!SIMDCompare::is_all_digits_simd(b"123a456"));
        assert!(SIMDCompare::is_all_digits_simd(b""));
    }
}