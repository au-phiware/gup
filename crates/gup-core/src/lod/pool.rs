// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Worker threads for level-of-detail builds (GUP-730), native targets
//! only: `wasm32` has no threads by default and builds on the frame's
//! thread (GUP-716).
//!
//! One process-wide pool of `available_parallelism() - 1` threads (at
//! least 1, at most [`MAX_WORKERS`]), started by the first [`Task::spawn`],
//! takes jobs first in, first out. A [`Task`] is the job's handle: the
//! frame polls it ([`Task::try_take`], never blocking) or, for a headless
//! frame that must be complete, waits for it ([`Task::wait`]), running
//! queued jobs itself meanwhile so that thread builds too. A job whose
//! handle was dropped before it started (its chunk was evicted or
//! rewritten) is skipped, and drops what it held at once.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::task::Waker;

/// At most this many workers: each build holds up to 16 B a row of
/// scratch plus its gathered representatives (GUP-714), so a wide machine
/// does not multiply the peak memory without bound.
const MAX_WORKERS: usize = 8;

type Job = Box<dyn FnOnce() + Send>;

/// The jobs not yet started.
static QUEUE: Mutex<VecDeque<Job>> = Mutex::new(VecDeque::new());
/// Signalled when a job is queued.
static QUEUED: Condvar = Condvar::new();
/// The workers started (0: none could be, so jobs run where they are
/// spawned).
static WORKERS: OnceLock<usize> = OnceLock::new();

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A job's panic is caught before it can poison anything.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start the workers, once.
fn workers() -> usize {
    *WORKERS.get_or_init(|| {
        let n = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .saturating_sub(1)
            .clamp(1, MAX_WORKERS);
        (0..n)
            .take_while(|i| {
                std::thread::Builder::new()
                    .name(format!("gup-lod-{i}"))
                    .spawn(work)
                    .is_ok()
            })
            .count()
    })
}

/// A worker: run queued jobs, forever.
fn work() {
    loop {
        let job = {
            let mut queue = lock(&QUEUE);
            loop {
                if let Some(job) = queue.pop_front() {
                    break job;
                }
                queue = QUEUED.wait(queue).unwrap_or_else(|e| e.into_inner());
            }
        };
        job();
    }
}

/// What a job and its handle share.
struct Shared<T> {
    value: Mutex<Option<std::thread::Result<T>>>,
    /// Set (after `value`) when the job has run.
    finished: AtomicBool,
    /// Signalled when `finished` is set.
    done: Condvar,
}

/// The handle of one job on the pool: its result, once it has run.
pub(crate) struct Task<T> {
    shared: Arc<Shared<T>>,
}

impl<T> std::fmt::Debug for Task<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Task")
            .field("finished", &self.finished())
            .finish()
    }
}

impl<T: Send + 'static> Task<T> {
    /// Run `job` on a worker; when it has run, wake `waker` (a host's,
    /// so it draws the frame that installs the result).
    pub(crate) fn spawn(job: impl FnOnce() -> T + Send + 'static, waker: Option<Waker>) -> Self {
        let shared = Arc::new(Shared {
            value: Mutex::new(None),
            finished: AtomicBool::new(false),
            done: Condvar::new(),
        });
        let theirs = Arc::clone(&shared);
        let run: Job = Box::new(move || {
            // Nobody waits for it any more: drop what it holds now.
            if Arc::strong_count(&theirs) == 1 {
                return;
            }
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            *lock(&theirs.value) = Some(out);
            theirs.finished.store(true, Ordering::Release);
            theirs.done.notify_all();
            if let Some(w) = waker {
                w.wake();
            }
        });
        if workers() == 0 {
            run();
        } else {
            lock(&QUEUE).push_back(run);
            QUEUED.notify_one();
        }
        Self { shared }
    }
}

impl<T> Task<T> {
    /// Whether the job has run (its result is ready to take).
    pub(crate) fn finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }

    /// The job's result if it has run (`Err`: it panicked), without
    /// waiting; `None` while it runs or waits to.
    pub(crate) fn try_take(&self) -> Option<std::thread::Result<T>> {
        if !self.finished() {
            return None;
        }
        lock(&self.shared.value).take()
    }

    /// The job's result, waiting for it; the calling thread runs queued
    /// jobs (its own or others') while it waits.
    pub(crate) fn wait(&self) -> std::thread::Result<T> {
        loop {
            if let Some(out) = self.try_take() {
                return out;
            }
            let next = lock(&QUEUE).pop_front();
            match next {
                Some(job) => job(),
                None => {
                    let mut value = lock(&self.shared.value);
                    while value.is_none() {
                        value = self
                            .shared
                            .done
                            .wait(value)
                            .unwrap_or_else(|e| e.into_inner());
                    }
                    return value.take().expect("checked above");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    #[test]
    fn a_task_runs_off_the_calling_thread_and_wakes_its_waker() {
        struct Count(AtomicUsize);
        impl std::task::Wake for Count {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let count = Arc::new(Count(AtomicUsize::new(0)));
        let me = std::thread::current().id();
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let task = Task::spawn(
            move || {
                rx.recv().unwrap();
                std::thread::current().id()
            },
            Some(Waker::from(Arc::clone(&count))),
        );
        // It is blocked until told: nothing to take, and no waiting.
        std::thread::sleep(Duration::from_millis(20));
        assert!(task.try_take().is_none());
        assert_eq!(count.0.load(Ordering::SeqCst), 0);
        tx.send(()).unwrap();
        let ran_on = task.wait().unwrap();
        assert_ne!(ran_on, me, "the job ran on a worker");
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn many_tasks_all_finish_and_a_panic_is_an_error() {
        let tasks: Vec<_> = (0..32u64)
            .map(|i| Task::spawn(move || i * i, None))
            .collect();
        let sum: u64 = tasks.iter().map(|t| t.wait().unwrap()).sum();
        assert_eq!(sum, (0..32u64).map(|i| i * i).sum());
        let bad = Task::spawn(|| -> u32 { panic!("a build failed") }, None);
        assert!(bad.wait().is_err());
    }

    #[test]
    fn a_dropped_task_does_not_run() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        // Occupy every worker, so the next job queues.
        let n = workers();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let blockers: Vec<_> = (0..n)
            .map(|_| {
                let gate = Arc::clone(&gate);
                Task::spawn(
                    move || {
                        let mut open = lock(&gate.0);
                        while !*open {
                            open = gate.1.wait(open).unwrap();
                        }
                    },
                    None,
                )
            })
            .collect();
        let dropped = Task::spawn(move || tx.send(()).unwrap(), None);
        drop(dropped);
        *lock(&gate.0) = true;
        gate.1.notify_all();
        for b in &blockers {
            b.wait().unwrap();
        }
        // Behind the blockers in the queue, it was skipped: its sender
        // is dropped without sending.
        let last = Task::spawn(|| (), None);
        last.wait().unwrap();
        assert!(rx.recv().is_err());
    }
}
