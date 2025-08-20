//! Locale-aware string comparison support for LC_COLLATE
//! 
//! This module provides locale-aware string comparison using the system's
//! strcoll function, respecting the LC_COLLATE environment variable.

use std::cmp::Ordering;
use std::ffi::CString;
use std::sync::OnceLock;
use std::env;

/// Global locale configuration
static LOCALE_CONFIG: OnceLock<LocaleConfig> = OnceLock::new();

/// Locale configuration for string comparison
#[derive(Debug, Clone)]
pub struct LocaleConfig {
    /// Whether locale-aware comparison is enabled
    pub enabled: bool,
    /// The current locale name
    pub locale_name: String,
    /// Whether the locale is UTF-8
    pub is_utf8: bool,
}

impl LocaleConfig {
    /// Initialize locale configuration from environment
    pub fn init() -> Self {
        // Get LC_COLLATE or LC_ALL or LANG
        let locale = env::var("LC_COLLATE")
            .or_else(|_| env::var("LC_ALL"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_else(|_| "C".to_string());
        
        // Check if locale is C or POSIX (byte comparison)
        let enabled = !locale.is_empty() && locale != "C" && locale != "POSIX";
        let is_utf8 = locale.contains("UTF-8") || locale.contains("utf8");
        
        // Set locale for strcoll
        if enabled {
            unsafe {
                let locale_cstr = CString::new(locale.clone()).unwrap_or_else(|_| CString::new("C").unwrap());
                libc::setlocale(libc::LC_COLLATE, locale_cstr.as_ptr());
            }
        }
        
        Self {
            enabled,
            locale_name: locale,
            is_utf8,
        }
    }
    
    /// Get the global locale configuration
    pub fn get() -> &'static LocaleConfig {
        LOCALE_CONFIG.get_or_init(Self::init)
    }
    
    /// Check if locale-aware comparison is enabled
    pub fn is_enabled() -> bool {
        Self::get().enabled
    }
}

/// Locale-aware string comparison using strcoll
pub fn strcoll_compare(a: &[u8], b: &[u8]) -> Ordering {
    // Fast path for identical strings
    if a == b {
        return Ordering::Equal;
    }
    
    // Convert to null-terminated C strings
    // For non-UTF8 locales, we need to handle invalid sequences
    let a_str = match std::str::from_utf8(a) {
        Ok(s) => s,
        Err(_) => {
            // Fallback to byte comparison for invalid UTF-8
            return a.cmp(b);
        }
    };
    
    let b_str = match std::str::from_utf8(b) {
        Ok(s) => s,
        Err(_) => {
            // Fallback to byte comparison for invalid UTF-8
            return a.cmp(b);
        }
    };
    
    // Create C strings
    let a_cstr = match CString::new(a_str) {
        Ok(s) => s,
        Err(_) => {
            // String contains null bytes, fallback to byte comparison
            return a.cmp(b);
        }
    };
    
    let b_cstr = match CString::new(b_str) {
        Ok(s) => s,
        Err(_) => {
            // String contains null bytes, fallback to byte comparison
            return a.cmp(b);
        }
    };
    
    // Call strcoll for locale-aware comparison
    unsafe {
        let result = libc::strcoll(a_cstr.as_ptr(), b_cstr.as_ptr());
        match result {
            x if x < 0 => Ordering::Less,
            x if x > 0 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

/// Case-insensitive locale-aware comparison using strcasecoll (if available)
/// Falls back to lowercasing + strcoll if strcasecoll is not available
pub fn strcasecoll_compare(a: &[u8], b: &[u8]) -> Ordering {
    // Fast path for identical strings
    if a == b {
        return Ordering::Equal;
    }
    
    // Convert to strings
    let a_str = match std::str::from_utf8(a) {
        Ok(s) => s,
        Err(_) => return case_insensitive_byte_compare(a, b),
    };
    
    let b_str = match std::str::from_utf8(b) {
        Ok(s) => s,
        Err(_) => return case_insensitive_byte_compare(a, b),
    };
    
    // Convert to lowercase for case-insensitive comparison
    let a_lower = a_str.to_lowercase();
    let b_lower = b_str.to_lowercase();
    
    // Use strcoll on lowercased strings
    strcoll_compare(a_lower.as_bytes(), b_lower.as_bytes())
}

/// Fallback case-insensitive byte comparison
fn case_insensitive_byte_compare(a: &[u8], b: &[u8]) -> Ordering {
    let len = a.len().min(b.len());
    
    for i in 0..len {
        let ca = a[i].to_ascii_lowercase();
        let cb = b[i].to_ascii_lowercase();
        match ca.cmp(&cb) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    
    a.len().cmp(&b.len())
}

/// Smart comparison that chooses between locale-aware and byte comparison
pub fn smart_compare(a: &[u8], b: &[u8], ignore_case: bool) -> Ordering {
    if LocaleConfig::is_enabled() {
        if ignore_case {
            strcasecoll_compare(a, b)
        } else {
            strcoll_compare(a, b)
        }
    } else {
        // Fast path: byte comparison for C/POSIX locale
        if ignore_case {
            case_insensitive_byte_compare(a, b)
        } else {
            a.cmp(b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_c_locale() {
        env::set_var("LC_COLLATE", "C");
        let config = LocaleConfig::init();
        assert!(!config.enabled);
        assert_eq!(config.locale_name, "C");
    }
    
    #[test]
    fn test_utf8_locale() {
        env::set_var("LC_COLLATE", "en_US.UTF-8");
        let config = LocaleConfig::init();
        assert!(config.enabled);
        assert!(config.is_utf8);
        assert_eq!(config.locale_name, "en_US.UTF-8");
    }
    
    #[test]
    fn test_strcoll_basic() {
        // Test basic ASCII comparison
        let a = b"apple";
        let b = b"banana";
        assert_eq!(strcoll_compare(a, b), Ordering::Less);
        assert_eq!(strcoll_compare(b, a), Ordering::Greater);
        assert_eq!(strcoll_compare(a, a), Ordering::Equal);
    }
    
    #[test]
    fn test_case_insensitive() {
        let a = b"Apple";
        let b = b"apple";
        assert_eq!(strcasecoll_compare(a, b), Ordering::Equal);
        
        let a = b"ZEBRA";
        let b = b"aardvark";
        assert_eq!(strcasecoll_compare(a, b), Ordering::Greater);
    }
}