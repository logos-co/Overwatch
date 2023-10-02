//! Overwatch is a framework to easily construct applications that requires of several independent
//! parts that needs communication between them.
//! Everything is self contained and it matches somewhat the advantages of microservices.
//!
//! ## Design Goals
//!
//! - Modularity:
//!     - Components should be self-contained (as possible)
//!     - Communication relations between components should be specifically defined
//!     - Components should be mockable. This is rather important for measurements and testing.
//!
//! - Single responsibility:
//!     - It is easier to isolate problems
//!     - Minimal sharing when unavoidable
//!
//! - Debuggeability
//!     - Easy to track workflow
//!     - Easy to test
//!     - Easy to measure
//!     - Asynchronous Communication
//!
//! ## Main components
//!
//! - Overwatch: the main messenger relay component (internal communications). It is also be responsible of managing other components lifecycle and handling configuration updates.
//! - Services (handled by the *overwatch*)

use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use futures::task::AtomicWaker;

pub mod overwatch;
pub mod services;
pub mod utils;

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct SignalWaiter<'a>(&'a Trigger);

impl<'a> std::future::Future for SignalWaiter<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.0.refs.load(Ordering::Acquire) {
            0 => Poll::Ready(()),
            _ => {
                self.0.waker.register(cx.waker());
                Poll::Pending
            }
        }
    }
}

pub struct Trigger {
    tx: async_channel::Sender<()>,
    waker: AtomicWaker,
    refs: Arc<AtomicUsize>,
}

impl Trigger {
    /// Returns true if this call has closed the channel and it was not closed already
    pub fn close(&self) -> bool {
        self.tx.close()
    }

    pub fn wait(&self) -> SignalWaiter<'_> {
        SignalWaiter(self)
    }
}

#[derive(Debug)]
pub struct Signal {
    rx: async_channel::Receiver<()>,
    refs: Arc<AtomicUsize>,
}

impl std::future::Future for Signal {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.rx.is_closed() {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}

impl Clone for Signal {
    fn clone(&self) -> Self {
        self.refs.fetch_add(1, Ordering::Release);
        Self {
            rx: self.rx.clone(),
            refs: self.refs.clone(),
        }
    }
}

impl Drop for Signal {
    fn drop(&mut self) {
        self.refs.fetch_sub(1, Ordering::Release);
    }
}

pub fn shutdown_signal() -> (Trigger, Signal) {
    let (tx, rx) = async_channel::bounded(1);
    let refs = Arc::new(AtomicUsize::new(1));
    let trigger = Trigger {
        tx,
        waker: AtomicWaker::new(),
        refs: refs.clone(),
    };
    let signal = Signal { rx, refs };
    (trigger, signal)
}
