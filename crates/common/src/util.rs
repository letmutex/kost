//! String concatenation utilities avoiding unnecessary format! allocations.

use smol_str::SmolStr;

/// Helper function to convert any string-like reference into `&str`.
#[inline(always)]
pub fn as_str<T: ?Sized + AsRef<str>>(s: &T) -> &str {
    s.as_ref()
}

/// Concatenates multiple string-like slices into a single heap `String`.
pub fn concat_str_slice(parts: &[&str]) -> String {
    let total_len: usize = parts.iter().map(|s| s.len()).sum();
    let mut result = String::with_capacity(total_len);
    for part in parts {
        result.push_str(part);
    }
    result
}

/// Concatenates multiple string-like slices into a `SmolStr`.
pub fn concat_smol_str_slice(parts: &[&str]) -> SmolStr {
    let total_len: usize = parts.iter().map(|s| s.len()).sum();
    if total_len <= 22 {
        let mut buf = [0u8; 22];
        let mut offset = 0;
        for s in parts {
            let bytes = s.as_bytes();
            buf[offset..offset + bytes.len()].copy_from_slice(bytes);
            offset += bytes.len();
        }
        let s = match std::str::from_utf8(&buf[..total_len]) {
            Ok(valid) => valid,
            Err(_) => unreachable!("Concatenation of valid UTF-8 slices is always valid UTF-8"),
        };
        SmolStr::new(s)
    } else {
        let mut s = String::with_capacity(total_len);
        for part in parts {
            s.push_str(part);
        }
        SmolStr::from(s)
    }
}

/// Concatenates string-like arguments into a `String` with a single allocation.
#[macro_export]
macro_rules! concat_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

/// Concatenates string-like arguments into a `SmolStr`.
#[macro_export]
macro_rules! concat_smol_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_smol_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

/// Formats an elapsed duration into a human-readable string (e.g. "12ms", "1.23s", "<1ms").
pub fn format_elapsed(duration: std::time::Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1 {
        "<1ms".to_string()
    } else if millis < 1000 {
        concat_str!(&millis.to_string(), "ms")
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}
