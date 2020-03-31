//! A common utility for building synchronization primitives.
//!
//! When an async operation is blocked, it needs to register itself somewhere so that it can be
//! notified later on. The `AsyncStdWakerSet` type helps with keeping track of such async operations and
//! notifying them when they may make progress.

use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;
use std::task::{Context, Waker};

pub use crate::slab::Index;
use crate::slab::Slab;
use locker::mutex::tagged_default::TaggedDefaultLock;

type Mutex<T> = locker::mutex::Mutex<TaggedDefaultLock, T>;

/// Set when there is at least one entry that has already been notified.
const NOTIFIED: u8 = 0b01;

/// Set when there is at least one notifiable entry.
const NOTIFIABLE: u8 = 0b10;

/// Inner representation of `AsyncStdWakerSet`.
struct Inner {
    /// A list of entries in the set.
    ///
    /// Each entry has an optional waker associated with the task that is executing the operation.
    /// If the waker is set to `None`, that means the task has been woken up but hasn't removed
    /// itself from the `AsyncStdWakerSet` yet.
    ///
    /// The key of each entry is its index in the `Slab`.
    entries: Slab<Option<Waker>>,

    /// The number of notifiable entries.
    notifiable: usize,
}

/// A set holding wakers.
pub struct AsyncStdWakerSet {
    /// Holds 2 bits: `NOTIFY_ONE`, and `NOTIFY_ALL`.
    inner: Mutex<Inner>,
}

impl AsyncStdWakerSet {
    /// Creates a new `AsyncStdWakerSet`.
    #[inline]
    pub const fn new() -> AsyncStdWakerSet {
        AsyncStdWakerSet {
            inner: TaggedDefaultLock::mutex(Inner {
                entries: Slab::new(),
                notifiable: 0,
            }),
        }
    }

    fn flag(&self) -> u8 {
        // Use `Acquire` ordering to synchronize with `Lock::drop()`.
        self.inner.raw().inner().tag(Ordering::Acquire)
    }

    /// Notifies blocked operations, either one or all of them.
    ///
    /// Returns `true` if at least one operation was notified.
    #[cold]
    fn notify(&self, n: Notify) -> bool {
        let mut inner = &mut *self.lock();
        let mut notified = false;

        for (_, opt_waker) in inner.entries.iter_mut() {
            // If there is no waker in this entry, that means it was already woken.
            if let Some(w) = opt_waker.take() {
                w.wake();
                inner.notifiable -= 1;
                notified = true;

                if n == Notify::One {
                    break;
                }
            }

            if n == Notify::Any {
                break;
            }
        }

        notified
    }

    /// Locks the list of entries.
    fn lock(&self) -> Lock<'_> {
        Lock {
            waker_set: self.inner.lock(),
        }
    }
}

impl crate::WakerSet for AsyncStdWakerSet {
    type Index = Index;

    fn is_empty(&self) -> bool {
        self.inner.lock().entries.is_empty()
    }

    /// Inserts a waker for a blocked operation and returns a key associated with it.
    #[cold]
    fn insert(&self, cx: &mut Context<'_>) -> Index {
        let w = cx.waker().clone();
        let mut inner = self.lock();

        let key = inner.entries.insert(Some(w));
        inner.notifiable += 1;
        key
    }

    /// Removes the waker of an operation.
    #[cold]
    fn remove(&self, key: Index) {
        let mut inner = self.lock();

        if inner.entries.remove(key).is_some() {
            inner.notifiable -= 1;
        }
    }

    /// Removes the waker of a cancelled operation.
    ///
    /// Returns `true` if another blocked operation from the set was notified.
    #[cold]
    fn cancel(&self, key: Index) -> bool {
        let mut inner = self.lock();

        match inner.entries.remove(key) {
            Some(_) => inner.notifiable -= 1,
            None => {
                // The operation was cancelled and notified so notify another operation instead.
                for (_, opt_waker) in inner.entries.iter_mut() {
                    // If there is no waker in this entry, that means it was already woken.
                    if let Some(w) = opt_waker.take() {
                        w.wake();
                        inner.notifiable -= 1;
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Notifies a blocked operation if none have been notified already.
    ///
    /// Returns `true` if an operation was notified.
    #[inline]
    fn notify_any(&self) -> bool {
        let flag = self.flag();

        if flag & NOTIFIED == 0 && flag & NOTIFIABLE != 0 {
            self.notify(Notify::Any)
        } else {
            false
        }
    }

    /// Notifies all blocked operations.
    ///
    /// Returns `true` if at least one operation was notified.
    #[inline]
    fn notify_all(&self) -> bool {
        if self.flag() & NOTIFIABLE != 0 {
            self.notify(Notify::All)
        } else {
            false
        }
    }
}

/// A guard holding a `AsyncStdWakerSet` locked.
struct Lock<'a> {
    waker_set: locker::exclusive_lock::ExclusiveGuard<'a, TaggedDefaultLock, Inner>,
}

impl Drop for Lock<'_> {
    #[inline]
    fn drop(&mut self) {
        let mut flag = 0;

        // Set the `NOTIFIED` flag if there is at least one notified entry.
        if self.entries.len() - self.notifiable > 0 {
            flag |= NOTIFIED;
        }

        // Set the `NOTIFIABLE` flag if there is at least one notifiable entry.
        if self.notifiable > 0 {
            flag |= NOTIFIABLE;
        }

        locker::exclusive_lock::ExclusiveGuard::raw(&self.waker_set)
            .inner()
            .swap_tag(flag, Ordering::Relaxed);
    }
}

impl Deref for Lock<'_> {
    type Target = Inner;

    #[inline]
    fn deref(&self) -> &Inner {
        &self.waker_set
    }
}

impl DerefMut for Lock<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Inner {
        &mut self.waker_set
    }
}

/// Notification strategy.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Notify {
    /// Make sure at least one entry is notified.
    Any,
    /// Notify one additional entry.
    One,
    /// Notify all entries.
    All,
}
