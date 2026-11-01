pub mod coordinator;
pub mod cursor;
pub mod event;
pub mod resolve;

pub use coordinator::{MAX_DISPLAYED_TASKS, ProgressCoordinator, run_coordinator};
pub use cursor::CursorGuard;
pub use event::ProgressEvent;
pub use resolve::ResolveProgress;

use smol_str::SmolStr;
use std::io::IsTerminal;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// Format a duration into a human-readable seconds string (e.g. "0.34s").
pub fn format_duration(duration: Duration) -> SmolStr {
    let secs = duration.as_secs_f64();
    let rounded = (secs * 100.0).round() / 100.0;
    common::concat_smol_str!(&rounded.to_string(), "s")
}

/// Cloneable handle for worker tasks to register and update downloads.
#[derive(Clone)]
pub struct ProgressHandle {
    sender: UnboundedSender<ProgressEvent>,
    next_id: Arc<AtomicU64>,
}

impl ProgressHandle {
    pub fn new(sender: UnboundedSender<ProgressEvent>) -> Self {
        Self {
            sender,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn create_task(&self, label: impl Into<SmolStr>, total_bytes: Option<u64>) -> TaskTracker {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let label_str = label.into();
        TaskTracker {
            id,
            label: label_str,
            sender: self.sender.clone(),
            downloaded: 0,
            total_bytes,
            start_time: std::time::Instant::now(),
            started: false,
            finished: false,
        }
    }
}

/// Per-artifact download tracker attached to HTTP stream reading.
pub struct TaskTracker {
    id: u64,
    label: SmolStr,
    sender: UnboundedSender<ProgressEvent>,
    downloaded: u64,
    total_bytes: Option<u64>,
    start_time: std::time::Instant,
    started: bool,
    finished: bool,
}

impl TaskTracker {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn start(&mut self) {
        if !self.started && !self.finished {
            self.started = true;
            self.start_time = std::time::Instant::now();
            let _ = self.sender.send(ProgressEvent::Started {
                id: self.id,
                label: self.label.clone(),
                total_bytes: self.total_bytes,
            });
        }
    }

    pub fn set_total_bytes(&mut self, total_bytes: u64) {
        self.total_bytes = Some(total_bytes);
        if self.started {
            let _ = self.sender.send(ProgressEvent::SetTotal {
                id: self.id,
                total_bytes,
            });
        }
    }

    pub fn update(&mut self, bytes: u64) {
        self.start();
        self.downloaded += bytes;
        let _ = self
            .sender
            .send(ProgressEvent::Chunk { id: self.id, bytes });
    }

    pub fn reset(&mut self) {
        self.downloaded = 0;
        if self.started {
            let _ = self.sender.send(ProgressEvent::Reset { id: self.id });
        }
    }

    pub fn finish(mut self) {
        self.finished = true;
        let _ = self.sender.send(ProgressEvent::Finished {
            id: self.id,
            label: self.label.clone(),
            total_bytes: self.downloaded,
            elapsed: self.start_time.elapsed(),
        });
    }

    pub fn fail(mut self, error: &str) {
        self.finished = true;
        let _ = self.sender.send(ProgressEvent::Failed {
            id: self.id,
            label: self.label.clone(),
            error: SmolStr::new(error),
        });
    }
}

impl Drop for TaskTracker {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.sender.send(ProgressEvent::Failed {
                id: self.id,
                label: self.label.clone(),
                error: SmolStr::new("Download aborted"),
            });
        }
    }
}

/// Starts the progress coordinator in the background if not silenced.
pub fn start_progress_coordinator(
    total_count: usize,
    silent: bool,
) -> (ProgressHandle, tokio::task::JoinHandle<()>) {
    let (tx, rx) = unbounded_channel();
    let handle = ProgressHandle::new(tx);

    let join_handle = tokio::spawn(async move {
        if !silent {
            let is_tty = std::io::stderr().is_terminal();
            let colorize = common::color::should_colorize_stderr();
            let coordinator = ProgressCoordinator::new(total_count, is_tty, colorize);
            run_coordinator(coordinator, rx).await;
        }
    });

    (handle, join_handle)
}
