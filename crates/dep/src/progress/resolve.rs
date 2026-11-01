use crate::coordinate::DependencyCoordinate;
use common::concat_str;
use smol_str::SmolStr;
use std::io::{IsTerminal, Write};
use std::time::Instant;

/// Dynamic unstyled single-line progress indicator for dependency resolution.
///
/// Writes exclusively to `stderr` using `\r` and space-padding in interactive
/// terminals, and completely blanks out the progress line when finished so
/// that stdout remains pristine for list/tree rendering.
#[derive(Debug)]
pub struct ResolveProgress {
    is_tty: bool,
    last_render: Instant,
    last_len: usize,
    throttle_ms: u64,
    active: bool,
    dirty: bool,
    pending_resolved: usize,
    pending_total: usize,
    pending_coord: SmolStr,
}

impl ResolveProgress {
    pub fn new(is_tty: bool) -> Self {
        Self {
            is_tty,
            last_render: Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
            last_len: 0,
            throttle_ms: 60,
            active: false,
            dirty: false,
            pending_resolved: 0,
            pending_total: 0,
            pending_coord: SmolStr::default(),
        }
    }

    pub fn default_for_stderr() -> Self {
        Self::new(std::io::stderr().is_terminal())
    }

    pub fn with_throttle(mut self, throttle_ms: u64) -> Self {
        self.throttle_ms = throttle_ms;
        self
    }

    pub fn on_update(&mut self, resolved: usize, total: usize, coord: &DependencyCoordinate) {
        if !self.is_tty {
            return;
        }

        self.pending_resolved = resolved;
        self.pending_total = total;
        self.pending_coord = SmolStr::new(coord.to_string());
        self.dirty = true;

        let is_complete = resolved >= total;
        let is_first = !self.active;
        if is_complete
            || is_first
            || self.last_render.elapsed().as_millis() >= self.throttle_ms as u128
        {
            self.render_current();
        }
    }

    pub fn flush_if_dirty(&mut self) {
        if self.is_tty && self.dirty {
            self.render_current();
        }
    }

    fn render_current(&mut self) {
        let line = Self::format_line(
            self.pending_resolved,
            self.pending_total,
            &self.pending_coord,
        );
        let padding_len = self.last_len.saturating_sub(line.len());
        let mut output = String::with_capacity(1 + line.len() + padding_len);
        output.push('\r');
        output.push_str(&line);
        for _ in 0..padding_len {
            output.push(' ');
        }

        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(output.as_bytes());
        let _ = stderr.flush();

        self.last_len = line.len();
        self.last_render = Instant::now();
        self.active = true;
        self.dirty = false;
    }

    pub fn clear(&mut self) {
        if self.is_tty && self.active && self.last_len > 0 {
            let mut output = String::with_capacity(self.last_len + 2);
            output.push('\r');
            for _ in 0..self.last_len {
                output.push(' ');
            }
            output.push('\r');

            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(output.as_bytes());
            let _ = stderr.flush();
            self.last_len = 0;
            self.active = false;
            self.dirty = false;
        }
    }

    pub fn finish(&mut self) {
        self.clear();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn last_len(&self) -> usize {
        self.last_len
    }

    pub fn format_line(resolved: usize, total: usize, coord_str: &str) -> String {
        let total_str = total.to_string();
        let resolved_str = resolved.to_string();
        let digits = total_str.len().max(2);
        let pad_len = digits.saturating_sub(resolved_str.len()).min(20);
        let padding = &"                    "[..pad_len];
        let truncated = common::truncate_label(coord_str, 45);
        concat_str!(
            "Resolving dependencies [",
            padding,
            &resolved_str,
            "/",
            &total_str,
            "] ",
            &truncated
        )
    }
}

impl Drop for ResolveProgress {
    fn drop(&mut self) {
        self.clear();
    }
}
