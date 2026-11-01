use smol_str::SmolStr;
use std::time::Duration;

/// Events transmitted from worker download tasks to the progress coordinator.
#[derive(Debug)]
pub enum ProgressEvent {
    Started {
        id: u64,
        label: SmolStr,
        total_bytes: Option<u64>,
    },
    Chunk {
        id: u64,
        bytes: u64,
    },
    SetTotal {
        id: u64,
        total_bytes: u64,
    },
    Reset {
        id: u64,
    },
    Finished {
        id: u64,
        label: SmolStr,
        total_bytes: u64,
        elapsed: Duration,
    },
    Failed {
        id: u64,
        label: SmolStr,
        error: SmolStr,
    },
}
