// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cross-thread append handle (RFC-001 S12a).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::task::Waker;

/// Appends rows to a chart from any thread. Rows pushed between two
/// frames are evaluated and uploaded together at the next resolve.
pub struct Appender<T> {
    tx: Sender<Vec<T>>,
    shared: Arc<Feed>,
}

/// The chart that owned an [`Appender`] was dropped.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Closed;

impl std::fmt::Display for Closed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the chart was dropped")
    }
}

impl std::error::Error for Closed {}

/// The state shared by a feed's receiver and its appenders.
#[derive(Default)]
pub(crate) struct Feed {
    /// Rows queued and not yet drained.
    pending: AtomicUsize,
    waker: Mutex<Option<Waker>>,
}

impl Feed {
    fn lock_waker(&self) -> std::sync::MutexGuard<'_, Option<Waker>> {
        self.waker.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The receiving half of a feed, owned by the chart (or layer).
pub(crate) struct FeedRx<T> {
    // `Receiver` is not `Sync`; only the owner drains, so the lock is free.
    rx: Mutex<Receiver<Vec<T>>>,
    shared: Arc<Feed>,
    tx: Option<Sender<Vec<T>>>,
}

impl<T> FeedRx<T> {
    pub(crate) fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            rx: Mutex::new(rx),
            shared: Arc::new(Feed::default()),
            tx: Some(tx),
        }
    }

    pub(crate) fn appender(&self) -> Appender<T> {
        Appender {
            tx: self.tx.clone().expect("the feed's sender is kept"),
            shared: self.shared.clone(),
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.shared.pending.load(Ordering::Acquire) > 0
    }

    pub(crate) fn set_waker(&self, waker: Option<&Waker>) {
        *self.shared.lock_waker() = waker.cloned();
    }

    /// Every queued row, in push order; resets the pending count.
    pub(crate) fn drain(&self) -> Vec<T> {
        let rx = self.rx.lock().unwrap_or_else(|e| e.into_inner());
        let mut rows = Vec::new();
        while let Ok(mut batch) = rx.try_recv() {
            self.shared.pending.fetch_sub(batch.len(), Ordering::AcqRel);
            rows.append(&mut batch);
        }
        rows
    }
}

impl<T> Clone for Appender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            shared: self.shared.clone(),
        }
    }
}

impl<T> std::fmt::Debug for Appender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Appender").finish_non_exhaustive()
    }
}

impl<T: Send + 'static> Appender<T> {
    /// Queue one row. `Err(Closed)` once the chart is dropped.
    pub fn push(&self, row: T) -> Result<(), Closed> {
        self.send(vec![row])
    }

    /// Queue `rows` as one batch (one wake, one draw-order batch).
    pub fn extend(&self, rows: impl IntoIterator<Item = T>) -> Result<(), Closed> {
        self.send(rows.into_iter().collect())
    }

    fn send(&self, rows: Vec<T>) -> Result<(), Closed> {
        let n = rows.len();
        if n == 0 {
            return Ok(());
        }
        self.shared.pending.fetch_add(n, Ordering::AcqRel);
        if self.tx.send(rows).is_err() {
            self.shared.pending.fetch_sub(n, Ordering::AcqRel);
            return Err(Closed);
        }
        if let Some(w) = self.shared.lock_waker().as_ref() {
            w.wake_by_ref();
        }
        Ok(())
    }
}
