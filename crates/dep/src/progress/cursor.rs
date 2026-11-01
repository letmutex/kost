use std::io::Write;

/// RAII guard that hides the terminal cursor on creation and restores it on drop.
pub struct CursorGuard;

impl CursorGuard {
    pub fn new() -> Self {
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(b"\x1b[?25l");
        let _ = stderr.flush();
        Self
    }
}

impl Default for CursorGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CursorGuard {
    fn drop(&mut self) {
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(b"\x1b[?25h");
        let _ = stderr.flush();
    }
}
