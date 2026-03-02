//! Priority job queue with cancellation tokens.
//!
//! Jobs enter with a priority (Visible > Prefetch) and the consumer
//! always pops the highest-priority item first.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering as AtomOrd};
use std::sync::Arc;

use parking_lot::Mutex;

// ─── Priority ───────────────────────────────────────────────────────────────

/// Decode / resize priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Currently visible on screen — highest priority.
    Visible  = 0,
    /// Nearby but off-screen — lower priority.
    Prefetch = 1,
}

impl Priority {
    /// Lower numeric value = higher priority.
    fn rank(self) -> u8 {
        self as u8
    }
}

// ─── Cancel token ───────────────────────────────────────────────────────────

/// Shared boolean flag — set to `true` to cancel a job.
#[derive(Debug, Clone)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, AtomOrd::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(AtomOrd::Acquire)
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Job ────────────────────────────────────────────────────────────────────

/// A request to decode + resize one image into the atlas.
#[derive(Debug, Clone)]
pub struct ThumbnailJob {
    /// Application-level identifier for the source image.
    pub source_id: u64,
    /// Path to the JPEG file.
    pub source_path: PathBuf,
    /// Decode / resize priority.
    pub priority: Priority,
    /// Set to cancel this job before or during processing.
    pub cancel: CancelToken,
}

impl PartialEq for ThumbnailJob {
    fn eq(&self, other: &Self) -> bool {
        self.source_id == other.source_id
    }
}
impl Eq for ThumbnailJob {}

impl PartialOrd for ThumbnailJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// BinaryHeap is a *max*-heap, so we reverse the comparison:
/// lower rank (= higher priority) compares as *greater*.
impl Ord for ThumbnailJob {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .rank()
            .cmp(&self.priority.rank())
            .then_with(|| self.source_id.cmp(&other.source_id))
    }
}

// ─── Queue ──────────────────────────────────────────────────────────────────

/// Thread-safe priority queue backed by `BinaryHeap`.
pub struct JobQueue {
    inner: Mutex<BinaryHeap<ThumbnailJob>>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(BinaryHeap::new()),
        }
    }

    /// Enqueue a job.
    pub fn push(&self, job: ThumbnailJob) {
        self.inner.lock().push(job);
    }

    /// Pop the highest-priority non-cancelled job.
    /// Silently drops cancelled jobs.
    pub fn pop(&self) -> Option<ThumbnailJob> {
        let mut heap = self.inner.lock();
        while let Some(job) = heap.pop() {
            if !job.cancel.is_cancelled() {
                return Some(job);
            }
            // Cancelled — discard and keep popping.
        }
        None
    }

    /// Cancel all jobs whose `source_id` matches.
    pub fn cancel_by_source(&self, source_id: u64) {
        let heap = self.inner.lock();
        for job in heap.iter() {
            if job.source_id == source_id {
                job.cancel.cancel();
            }
        }
    }

    /// Drain everything (e.g. on scroll jump).
    pub fn clear(&self) {
        self.inner.lock().clear();
    }

    /// Number of enqueued (including possibly cancelled) jobs.
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}
