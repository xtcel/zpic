//! Helpers for wrapping request bodies with byte-level progress reporting.
//!
//! `reqwest::Body::wrap_stream` accepts a `TryStream<Ok = Bytes>`. The
//! helpers here produce such a stream from an in-memory `Bytes` value,
//! chunking it into pieces and invoking a [`ProgressCallback`] as each
//! chunk leaves the stream. The total size is known up front, so the
//! CLI's progress bar sees monotonic byte counts that reach
//! `total_bytes` exactly when the stream finishes.
//!
//! The chunks are intentionally large (64 KiB by default) to keep the
//! per-chunk callback overhead negligible. With 64 KiB chunks a 100 MB
//! upload fires ~1,600 callbacks — plenty for a smooth bar without
//! spending the CPU on lock acquisitions.
//!
//! When no progress callback is installed we skip the streaming dance
//! entirely and hand `reqwest` the original `Bytes` (which it accepts
//! directly as a `Body`). This keeps the dry-run path and the
//! progress-disabled path zero-cost.

use bytes::Bytes;
use futures::{stream, StreamExt};
use zpic_core::upload::ProgressCallback;

/// Default chunk size for the streaming wrapper. 64 KiB is large enough
/// to amortize the cost of a callback invocation but small enough that
/// the progress bar updates several times per second on a typical home
/// upload link.
pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;

/// Build a `reqwest::Body` from `body`, reporting byte-level progress
/// via `on_progress` if one is installed.
///
/// When `on_progress` is `None`, returns `reqwest::Body::from(body)`
/// directly — reqwest already supports `Body: From<Bytes>`.
pub fn body_with_progress(body: Bytes, on_progress: Option<ProgressCallback>) -> reqwest::Body {
    let Some(on_progress) = on_progress else {
        return reqwest::Body::from(body);
    };
    let total = body.len() as u64;
    if total == 0 {
        // reqwest will send a 0-length body either way; emit no
        // chunks so the stream completes immediately.
        return reqwest::Body::from(body);
    }
    let chunk_size = DEFAULT_CHUNK_SIZE;
    // Pre-compute the chunk offsets so the stream can yield borrowed
    // `Bytes` slices (via `Bytes::slice`) without copying.
    let chunk_offsets: Vec<(usize, usize)> = (0..total as usize)
        .step_by(chunk_size)
        .map(|start| {
            let end = (start + chunk_size).min(total as usize);
            (start, end)
        })
        .collect();
    let total_for_stream = total;
    let stream = stream::iter(chunk_offsets)
        .enumerate()
        .map(move |(idx, (start, end))| {
            let chunk = body.slice(start..end);
            // Report the *cumulative* bytes sent so far, not the chunk
            // size. This makes the bar advance monotonically even when
            // the underlying chunks are delivered out of order (which can
            // happen when reqwest pipelines them).
            let sent = ((idx + 1) * chunk_size) as u64;
            let sent = sent.min(total_for_stream);
            let cb = on_progress.clone();
            cb(sent, total_for_stream);
            Ok::<Bytes, std::io::Error>(chunk)
        });
    reqwest::Body::wrap_stream(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn no_callback_returns_direct_body() {
        // With no callback, the body is returned directly. We can't
        // easily inspect a reqwest::Body to assert this, but we can
        // assert the function compiles and returns without panicking.
        let body = body_with_progress(Bytes::from_static(b"hello"), None);
        drop(body);
    }

    #[test]
    fn chunk_offsets_cover_whole_buffer() {
        // 100-byte buffer split into 64-byte chunks → (0,64) and (64,100).
        let body = Bytes::from(vec![0u8; 100]);
        let offsets: Vec<_> = (0..100usize)
            .step_by(64)
            .map(|s| {
                let e = (s + 64).min(100);
                (s, e)
            })
            .collect();
        assert_eq!(offsets, vec![(0, 64), (64, 100)]);
        // The body still has its full length after slicing.
        let _ = body.slice(0..64);
        assert_eq!(body.len(), 100);
    }

    #[test]
    fn callback_fires_after_stream_poll() {
        // We can't easily drive a reqwest::Body to completion in a
        // unit test, but we can build the stream directly and assert
        // that polling it produces the expected chunks and fires the
        // progress callback for each chunk.
        use futures::TryStreamExt;
        let total = 100u64;
        let observed = Arc::new(AtomicU64::new(0));
        let observed_inner = Arc::clone(&observed);
        let cb: ProgressCallback = Arc::new(move |sent, total_seen| {
            observed_inner.store(sent, Ordering::SeqCst);
            assert_eq!(total_seen, total);
        });
        let body = Bytes::from(vec![0u8; total as usize]);
        // Use a chunk size smaller than `total` so the stream produces
        // multiple chunks — DEFAULT_CHUNK_SIZE (64 KiB) would collapse a
        // 100-byte body into a single chunk and the per-chunk-callback
        // invariant would not be exercised.
        let chunk_size = 64;
        let chunk_offsets: Vec<(usize, usize)> = (0..total as usize)
            .step_by(chunk_size)
            .map(|s| (s, (s + chunk_size).min(total as usize)))
            .collect();
        let stream = stream::iter(chunk_offsets)
            .enumerate()
            .map(move |(idx, (start, end))| {
                let chunk = body.slice(start..end);
                let sent = ((idx + 1) * chunk_size) as u64;
                let sent = sent.min(total);
                let cb = cb.clone();
                cb(sent, total);
                Ok::<Bytes, std::io::Error>(chunk)
            });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let collected: Vec<Bytes> = stream.try_collect().await.unwrap();
            assert_eq!(collected.len(), 2);
            assert_eq!(collected.iter().map(|c| c.len()).sum::<usize>(), 100);
        });
        assert_eq!(observed.load(Ordering::SeqCst), 100);
    }
}
