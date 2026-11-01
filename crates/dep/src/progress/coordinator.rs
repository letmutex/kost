use ahash::AHashMap;
use common::color::{BOLD_CYAN, BOLD_GREEN, BOLD_RED, wrap_styled};
use common::{format_bytes, format_progress_bar, truncate_label};
use smol_str::SmolStr;
use std::io::Write;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

use super::cursor::CursorGuard;
use super::event::ProgressEvent;
use super::format_duration;

pub(crate) struct ActiveTaskState {
    pub label: SmolStr,
    pub downloaded: u64,
    pub total_bytes: Option<u64>,
}

/// Coordinates terminal progress display across multiple concurrent download tasks.
pub struct ProgressCoordinator {
    pub is_tty: bool,
    pub colorize: bool,
    pub total_count: usize,
    pub completed_count: usize,
    pub(crate) active_tasks: AHashMap<u64, ActiveTaskState>,
    pub(crate) active_keys: Vec<u64>,
    pub(crate) pending_completed: Vec<String>,
    pub rendered_lines: usize,
    pub dirty: bool,
}

impl ProgressCoordinator {
    pub fn new(total_count: usize, is_tty: bool, colorize: bool) -> Self {
        Self {
            is_tty,
            colorize,
            total_count,
            completed_count: 0,
            active_tasks: AHashMap::default(),
            active_keys: Vec::new(),
            pending_completed: Vec::new(),
            rendered_lines: 0,
            dirty: true,
        }
    }

    pub fn active_tasks_count(&self) -> usize {
        self.active_tasks.len()
    }

    pub fn on_started(&mut self, id: u64, label: SmolStr, total_bytes: Option<u64>) {
        self.active_tasks.insert(
            id,
            ActiveTaskState {
                label,
                downloaded: 0,
                total_bytes,
            },
        );
        if !self.active_keys.contains(&id) {
            self.active_keys.push(id);
        }
        self.dirty = true;
    }

    pub fn on_chunk(&mut self, id: u64, bytes: u64) {
        if let Some(task) = self.active_tasks.get_mut(&id) {
            task.downloaded += bytes;
            self.dirty = true;
        }
    }

    pub fn on_set_total(&mut self, id: u64, total_bytes: u64) {
        if let Some(task) = self.active_tasks.get_mut(&id) {
            task.total_bytes = Some(total_bytes);
            self.dirty = true;
        }
    }

    pub fn on_reset(&mut self, id: u64) {
        if let Some(task) = self.active_tasks.get_mut(&id) {
            task.downloaded = 0;
            self.dirty = true;
        }
    }

    fn finish_task(&mut self, id: u64, fallback_label: &SmolStr) -> SmolStr {
        self.completed_count += 1;
        self.active_keys.retain(|k| *k != id);
        self.dirty = true;
        self.active_tasks
            .remove(&id)
            .map(|t| t.label)
            .unwrap_or_else(|| fallback_label.clone())
    }

    pub fn on_finished(&mut self, id: u64, label: SmolStr, total_bytes: u64, elapsed: Duration) {
        let label = self.finish_task(id, &label);
        let status = wrap_styled("  Downloaded", BOLD_GREEN, self.colorize);
        let size_str = format_bytes(total_bytes);
        let time_str = format_duration(elapsed);
        let msg = common::concat_str!(
            &status, " ", &label, " (", &size_str, " in ", &time_str, ")"
        );

        if self.is_tty {
            self.pending_completed.push(msg);
        } else {
            println!("{msg}");
        }
    }

    pub fn on_failed(&mut self, id: u64, label: SmolStr, error: SmolStr) {
        let label = self.finish_task(id, &label);
        let status = wrap_styled("      Failed", BOLD_RED, self.colorize);
        let msg = common::concat_str!(&status, " ", &label, ": ", &error);

        if self.is_tty {
            self.pending_completed.push(msg);
        } else {
            println!("{msg}");
        }
    }

    pub fn clear_active_lines(&mut self) {
        if !self.is_tty {
            for msg in self.pending_completed.drain(..) {
                println!("{msg}");
            }
            return;
        }

        let mut buf = String::with_capacity(64);
        if self.rendered_lines > 1 {
            use std::fmt::Write;
            let _ = write!(buf, "\r\x1b[{}A", self.rendered_lines - 1);
        } else if self.rendered_lines == 1 {
            buf.push('\r');
        }

        for msg in self.pending_completed.drain(..) {
            buf.push_str(&msg);
            buf.push_str("\x1b[K\n");
        }

        if self.rendered_lines > 0 {
            buf.push_str("\x1b[J");
        }

        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(buf.as_bytes());
        let _ = stderr.flush();
        self.rendered_lines = 0;
    }

    fn format_task_line(&self, task: &ActiveTaskState, status: &str) -> String {
        let label = truncate_label(&task.label, 30);
        let downloaded = format_bytes(task.downloaded);

        if let Some(total) = task.total_bytes
            && total > 0
        {
            let bar = format_progress_bar(task.downloaded, total, 10);
            let total_str = format_bytes(total);
            common::concat_str!(
                status,
                " ",
                &label,
                " ",
                &bar,
                " ",
                &downloaded,
                "/",
                &total_str
            )
        } else {
            common::concat_str!(status, " ", &label, " ", &downloaded)
        }
    }

    pub fn render_active_lines(&mut self) {
        if !self.is_tty {
            for msg in self.pending_completed.drain(..) {
                println!("{msg}");
            }
            return;
        }

        let mut buf = String::with_capacity(1024);

        if self.rendered_lines > 1 {
            use std::fmt::Write;
            let _ = write!(buf, "\r\x1b[{}A", self.rendered_lines - 1);
        } else if self.rendered_lines == 1 {
            buf.push('\r');
        }

        for msg in self.pending_completed.drain(..) {
            buf.push_str(&msg);
            buf.push_str("\x1b[K\n");
        }

        let status_text = " Downloading";
        let status = wrap_styled(status_text, BOLD_CYAN, self.colorize);
        let mut lines = Vec::with_capacity(self.active_keys.len() + 1);

        for &task_id in &self.active_keys {
            if let Some(task) = self.active_tasks.get(&task_id) {
                lines.push(self.format_task_line(task, &status));
            }
        }

        if self.total_count > 0 && self.completed_count < self.total_count {
            let total_str = self.total_count.to_string();
            let completed_str = self.completed_count.to_string();
            let digits = total_str.len().max(2);
            let pad_len = digits.saturating_sub(completed_str.len()).min(20);
            let padding = &"                    "[..pad_len];
            let bar = format_progress_bar(self.completed_count as u64, self.total_count as u64, 25);
            let progress_text =
                common::concat_str!("[", padding, &completed_str, "/", &total_str, "]",);
            let pad_len = status_text.len().saturating_sub(progress_text.len());
            let padding = &"          "[..pad_len];
            let line = common::concat_str!(
                padding,
                wrap_styled(&progress_text, BOLD_CYAN, self.colorize),
                " ",
                &bar,
            );
            lines.push(line);
        }

        if lines.is_empty() {
            buf.push_str("\x1b[J");
            self.rendered_lines = 0;
        } else {
            for (idx, line) in lines.iter().enumerate() {
                buf.push_str(line);
                buf.push_str("\x1b[K");
                if idx + 1 < lines.len() {
                    buf.push('\n');
                }
            }

            if lines.len() < self.rendered_lines {
                let leftover = self.rendered_lines - lines.len();
                for _ in 0..leftover {
                    buf.push_str("\n\x1b[K");
                }
                use std::fmt::Write;
                let _ = write!(buf, "\x1b[{}A", leftover);
            }

            self.rendered_lines = lines.len();
        }

        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(buf.as_bytes());
        let _ = stderr.flush();
    }

    pub fn handle_event(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::Started {
                id,
                label,
                total_bytes,
            } => {
                self.on_started(id, label, total_bytes);
            }
            ProgressEvent::Chunk { id, bytes } => {
                self.on_chunk(id, bytes);
            }
            ProgressEvent::SetTotal { id, total_bytes } => {
                self.on_set_total(id, total_bytes);
            }
            ProgressEvent::Reset { id } => {
                self.on_reset(id);
            }
            ProgressEvent::Finished {
                id,
                label,
                total_bytes,
                elapsed,
            } => {
                self.on_finished(id, label, total_bytes, elapsed);
            }
            ProgressEvent::Failed { id, label, error } => {
                self.on_failed(id, label, error);
            }
        }
    }
}

/// Runs the background coordinator event loop with throttled refresh and clean shutdown.
pub async fn run_coordinator(
    mut coordinator: ProgressCoordinator,
    mut rx: UnboundedReceiver<ProgressEvent>,
) {
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let _cursor_guard = if coordinator.is_tty {
        Some(CursorGuard::new())
    } else {
        None
    };

    loop {
        tokio::select! {
            event = rx.recv() => {
                let Some(first_event) = event else { break };
                coordinator.handle_event(first_event);

                while let Ok(next_event) = rx.try_recv() {
                    coordinator.handle_event(next_event);
                }

                if coordinator.is_tty && coordinator.dirty {
                    coordinator.render_active_lines();
                    coordinator.dirty = false;
                }
            }
            _ = ticker.tick() => {
                if coordinator.is_tty
                    && coordinator.dirty
                    && (!coordinator.active_tasks.is_empty()
                        || !coordinator.pending_completed.is_empty()
                        || coordinator.completed_count < coordinator.total_count)
                {
                    coordinator.render_active_lines();
                    coordinator.dirty = false;
                }
            }
        }
    }

    coordinator.clear_active_lines();
}
