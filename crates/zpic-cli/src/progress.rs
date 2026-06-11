//! Real-time upload progress for the CLI.
//!
//! The upload pipeline already exposes a byte-level callback through
//! `UploadContext::on_progress`, so the CLI only needs a thin adapter
//! that turns those callbacks into terminal updates. We intentionally
//! keep this layer small:
//!
//! - `ProgressSink` decides whether progress should render.
//! - `begin_file` creates one `cliclack` progress bar for the active file.
//! - `callback_for` maps uploader-reported byte counts onto the original
//!   file size shown to the user.
//!
//! That last point matters for uploaders like GitHub: the HTTP body is a
//! JSON payload containing base64 text, so the body size on the wire is
//! larger than the source image. The callback rescales the reported body
//! progress back to the original file size so the UI still shows a
//! sensible `file_bytes/file_bytes` bar.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cliclack::{progress_bar, spinner, ProgressBar};
use zpic_core::upload::ProgressCallback;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SinkMode {
    Enabled,
    Disabled,
}

struct ActiveProgress {
    total_bytes: u64,
    bar: ProgressBar,
}

/// Real-time upload progress for one `zpic upload` run.
pub struct ProgressSink {
    mode: SinkMode,
    uploader_name: String,
    files_done: AtomicUsize,
    files_total: AtomicUsize,
    active: Mutex<Option<ActiveProgress>>,
}

impl ProgressSink {
    /// Build a new sink.
    ///
    /// `silent` should be `true` for modes where any terminal animation
    /// would be noise, such as `--json` or `--no-progress`.
    pub fn new(silent: bool, uploader_name: &str, file_count: usize, dry_run: bool) -> Self {
        let mode = if silent || dry_run || file_count == 0 {
            SinkMode::Disabled
        } else if std::io::stderr().is_terminal() {
            SinkMode::Enabled
        } else {
            SinkMode::Disabled
        };

        Self {
            mode,
            uploader_name: uploader_name.to_string(),
            files_done: AtomicUsize::new(0),
            files_total: AtomicUsize::new(file_count),
            active: Mutex::new(None),
        }
    }

    /// Returns `true` when this sink will actually render progress.
    pub fn is_enabled(&self) -> bool {
        matches!(self.mode, SinkMode::Enabled)
    }

    /// Reserved for API compatibility with the existing caller.
    pub fn start(&self) {}

    /// Notify the sink that a new file is starting.
    pub fn begin_file(&self, label: &str, total_bytes: u64) {
        if !self.is_enabled() {
            return;
        }

        if let Some(active) = self.active.lock().unwrap().take() {
            active.bar.clear();
        }

        let current = self.files_done.load(Ordering::SeqCst) + 1;
        let total_files = self.files_total.load(Ordering::SeqCst);
        let message = progress_message(&self.uploader_name, label, current, total_files);

        let bar = if total_bytes == 0 {
            spinner()
        } else {
            progress_bar(total_bytes).with_download_template()
        };
        bar.start(message);

        *self.active.lock().unwrap() = Some(ActiveProgress { total_bytes, bar });
    }

    /// Build a progress callback for the current file.
    pub fn callback_for(&self, _label: &str, total_bytes: u64) -> ProgressCallback {
        if !self.is_enabled() {
            return Arc::new(|_, _| {});
        }

        let bar = self
            .active
            .lock()
            .unwrap()
            .as_ref()
            .map(|active| active.bar.clone());

        Arc::new(move |sent, reported_total| {
            let Some(bar) = &bar else {
                return;
            };

            if total_bytes == 0 {
                return;
            }

            let position = scale_position(sent, reported_total, total_bytes);
            bar.set_length(total_bytes);
            bar.set_position(position);
        })
    }

    /// Mark the current file as successfully uploaded.
    pub fn finish_file(&self, _label: &str, total_bytes: u64) {
        if !self.is_enabled() {
            return;
        }

        if let Some(active) = self.active.lock().unwrap().take() {
            if active.total_bytes > 0 {
                active.bar.set_length(active.total_bytes);
                active.bar.set_position(total_bytes.min(active.total_bytes));
            }
            active.bar.clear();
        }
        self.files_done.fetch_add(1, Ordering::SeqCst);
    }

    /// Mark the current file as failed.
    pub fn fail_file(&self, _label: &str) {
        if !self.is_enabled() {
            return;
        }

        if let Some(active) = self.active.lock().unwrap().take() {
            active.bar.clear();
        }
        self.files_done.fetch_add(1, Ordering::SeqCst);
    }

    /// Tear down any active progress UI. Always safe to call.
    pub fn finish(&self) {
        if !self.is_enabled() {
            return;
        }

        if let Some(active) = self.active.lock().unwrap().take() {
            active.bar.clear();
        }
    }

    /// Override the total batch size.
    pub fn set_batch_size(&self, files: usize) {
        self.files_total.store(files, Ordering::SeqCst);
    }
}

impl Drop for ProgressSink {
    fn drop(&mut self) {
        if let Some(active) = self.active.lock().unwrap().take() {
            active.bar.clear();
        }
    }
}

fn progress_message(uploader_name: &str, label: &str, current: usize, total: usize) -> String {
    if total > 1 {
        format!("Uploading {label} via {uploader_name} ({current}/{total})")
    } else {
        format!("Uploading {label} via {uploader_name}")
    }
}

fn scale_position(sent: u64, reported_total: u64, display_total: u64) -> u64 {
    if display_total == 0 {
        return 0;
    }
    if reported_total == 0 || reported_total == display_total {
        return sent.min(display_total);
    }

    let scaled = ((sent as u128) * (display_total as u128)) / (reported_total as u128);
    scaled.min(display_total as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_position_clamps_direct_progress() {
        assert_eq!(scale_position(0, 100, 100), 0);
        assert_eq!(scale_position(50, 100, 100), 50);
        assert_eq!(scale_position(150, 100, 100), 100);
    }

    #[test]
    fn scale_position_maps_body_progress_back_to_file_size() {
        assert_eq!(scale_position(0, 200, 100), 0);
        assert_eq!(scale_position(100, 200, 100), 50);
        assert_eq!(scale_position(200, 200, 100), 100);
    }

    #[test]
    fn progress_message_includes_batch_index_when_needed() {
        assert_eq!(
            progress_message("s3", "[1] cover.png", 1, 3),
            "Uploading [1] cover.png via s3 (1/3)"
        );
        assert_eq!(
            progress_message("s3", "cover.png", 1, 1),
            "Uploading cover.png via s3"
        );
    }

    #[test]
    fn disabled_sink_callback_is_noop() {
        let sink = ProgressSink::new(true, "s3", 1, false);
        assert!(!sink.is_enabled());
        let cb = sink.callback_for("cover.png", 100);
        cb(50, 100);
    }
}
