use std::fmt::Display;
#[cfg(not(windows))]
use std::time::{SystemTime, UNIX_EPOCH};

/// Format the current local timestamp as `YYYY-MM-DD HH:MM:SS.mmm`.
pub fn local_timestamp() -> String {
    let (year, month, day, hour, min, sec, millis) = local_time_components();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        year, month, day, hour, min, sec, millis
    )
}

#[cfg(windows)]
fn local_time_components() -> (u16, u16, u16, u16, u16, u16, u16) {
    #[repr(C)]
    struct SystemTime {
        w_year: u16,
        w_month: u16,
        w_day_of_week: u16,
        w_day: u16,
        w_hour: u16,
        w_minute: u16,
        w_second: u16,
        w_milliseconds: u16,
    }
    unsafe extern "system" {
        fn GetLocalTime(lp_system_time: *mut SystemTime);
    }
    let mut st = SystemTime {
        w_year: 0,
        w_month: 0,
        w_day_of_week: 0,
        w_day: 0,
        w_hour: 0,
        w_minute: 0,
        w_second: 0,
        w_milliseconds: 0,
    };
    unsafe {
        GetLocalTime(&mut st);
    }
    (
        st.w_year,
        st.w_month,
        st.w_day,
        st.w_hour,
        st.w_minute,
        st.w_second,
        st.w_milliseconds,
    )
}

#[cfg(not(windows))]
fn local_time_components() -> (u16, u16, u16, u16, u16, u16, u16) {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs() as i64;
    let millis = duration.subsec_millis() as u16;

    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const i8,
    }

    unsafe extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }

    let mut tm = std::mem::MaybeUninit::<Tm>::zeroed();
    let ptr = unsafe { localtime_r(&secs, tm.as_mut_ptr()) };
    if !ptr.is_null() {
        let tm = unsafe { tm.assume_init() };
        (
            (tm.tm_year + 1900) as u16,
            (tm.tm_mon + 1) as u16,
            tm.tm_mday as u16,
            tm.tm_hour as u16,
            tm.tm_min as u16,
            tm.tm_sec as u16,
            millis,
        )
    } else {
        (1970, 1, 1, 0, 0, 0, millis)
    }
}

/// Print a raw log message with the local timestamp and tag to stderr.
pub fn raw_log(tag: &str, message: impl Display) {
    eprintln!("[{}] [{}] {}", local_timestamp(), tag, message);
}

macro_rules! log {
    ($tag:expr, $($arg:tt)*) => {
        $crate::logger::raw_log($tag, format_args!($($arg)*))
    };
}

pub(crate) use log;

#[cfg(test)]
mod tests {
    use super::local_timestamp;

    #[test]
    fn test_local_timestamp_format() {
        let ts = local_timestamp();
        assert_eq!(ts.len(), 23); // YYYY-MM-DD HH:MM:SS.mmm
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], " ");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
        assert_eq!(&ts[19..20], ".");
    }
}
