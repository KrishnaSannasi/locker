//! A wrapper around an [`RawExclusiveLock`] that allows it to be used as a
//! reentrant lock

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair, RawExclusiveLockTimed};
use crate::share_lock::{RawShareLock, RawShareLockFair, RawShareLockTimed};

use super::{counter::Scalar, ThreadInfo};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// A wrapper around a [`RawExclusiveLock`] that allows it to be used as a
        /// reentrant mutex
        pub struct ReLock<L, S = super::counter::SubWord, I = super::std_thread::StdThreadInfo> {
            inner: L,
            thread_info: I,
            owner: AtomicUsize,
            count: Cell<S>,
        }
    } else {
        /// A wrapper around a [`RawExclusiveLock`] that allows it to be used as a
        /// reentrant mutex
        pub struct ReLock<L, I> {
            inner: L,
            thread_info: I,
            owner: AtomicUsize,
            count: Cell<usize>,
        }
    }
}

unsafe impl<L: Sync + crate::mutex::RawMutex, S: Send, I: Sync> Sync for ReLock<L, S, I> {}

impl<L, I, S> ReLock<L, S, I> {
    /// # Safety
    ///
    /// `inner` must not be shared
    #[inline]
    pub const unsafe fn from_raw_parts(inner: L, thread_info: I, counter: S) -> Self {
        Self {
            inner,
            thread_info,
            owner: AtomicUsize::new(0),
            count: Cell::new(counter),
        }
    }

    /// the underlying lock
    pub fn inner(&self) -> &L {
        &self.inner
    }

    /// the underlying thread info
    pub fn thread_info(&self) -> &I {
        &self.thread_info
    }
}

unsafe impl<L: crate::mutex::RawMutex, S: Scalar, I: ThreadInfo> super::RawReentrantMutex
    for ReLock<L, S, I>
{
}

unsafe impl<L: crate::RawLockInfo, S: Scalar, I: ThreadInfo> crate::RawLockInfo
    for ReLock<L, S, I>
{
    const INIT: Self = unsafe { Self::from_raw_parts(L::INIT, I::INIT, S::ZERO) };

    type ExclusiveGuardTraits = std::convert::Infallible;
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

impl<L: RawExclusiveLock, S: Scalar, I: ThreadInfo> ReLock<L, S, I> {
    #[inline]
    fn lock_internal(&self, try_lock: impl FnOnce() -> bool) -> bool {
        let id = self.thread_info.id().get();
        let owner = self.owner.load(Ordering::Relaxed);

        if owner == id {
            unsafe { self.shr_split() }
        } else {
            if !try_lock() {
                return false;
            }

            self.owner.store(id, Ordering::Relaxed);
        }

        true
    }

    #[inline]
    fn unlock_internal(&self, unlock_slow: impl FnOnce()) {
        if let Some(count) = self.count.get().to_usize().checked_sub(1) {
            self.count.set(S::from_usize_unchecked(count));
        } else {
            self.owner.store(0, Ordering::Relaxed);
            unlock_slow()
        }
    }
}

unsafe impl<L: RawExclusiveLock, S: Scalar, I: ThreadInfo> RawShareLock for ReLock<L, S, I> {
    #[inline]
    fn shr_lock(&self) {
        self.lock_internal(|| {
            self.inner.exc_lock();
            true
        });
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        self.lock_internal(|| self.inner.exc_try_lock())
    }

    #[inline]
    unsafe fn shr_split(&self) {
        debug_assert_eq!(
            self.owner.load(Ordering::Relaxed),
            self.thread_info.id().get()
        );
        let (count, ovf) = self.count.get().to_usize().overflowing_add(1);
        assert!(!ovf && S::is_in_bounds(count), "Cannot overflow");
        self.count.set(S::from_usize_unchecked(count));
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        self.unlock_internal(
            #[cold]
            || self.inner.exc_unlock(),
        )
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        if self.count.get().to_usize() == 0 {
            self.inner.exc_bump();
        }
    }
}

unsafe impl<L: RawExclusiveLockFair, S: Scalar, I: ThreadInfo> RawShareLockFair
    for ReLock<L, S, I>
{
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        self.unlock_internal(
            #[cold]
            || self.inner.exc_unlock_fair(),
        )
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        if self.count.get().to_usize() == 0 {
            self.inner.exc_bump_fair();
        }
    }
}

impl<L: crate::RawTimedLock, S: Scalar, I: ThreadInfo> crate::RawTimedLock for ReLock<L, S, I> {
    type Instant = L::Instant;
    type Duration = L::Duration;
}

unsafe impl<L: RawExclusiveLockTimed, S: Scalar, I: ThreadInfo> RawShareLockTimed
    for ReLock<L, S, I>
{
    fn shr_try_lock_until(&self, instant: Self::Instant) -> bool {
        self.lock_internal(|| self.inner.exc_try_lock_until(instant))
    }

    fn shr_try_lock_for(&self, duration: Self::Duration) -> bool {
        self.lock_internal(|| self.inner.exc_try_lock_for(duration))
    }
}

#[cfg(test)]
mod test {
    #[test]
    #[cfg(all(feature = "std", feature = "parking_lot"))]
    fn reentrant() {
        use crate::mutex::simple::RawLock;

        type ReentrantMutex<T> = super::ReentrantMutex<ReLock<RawLock>, T>;

        let mtx = ReentrantMutex::new(Cell::new(0));

        let _lock = mtx.lock();

        assert_eq!(_lock.get(), 0);

        mtx.lock().set(10);

        assert_eq!(_lock.get(), 10);
    }

    #[test]
    #[cfg(all(feature = "std", feature = "parking_lot"))]
    fn reentrant_multi() {
        use crate::mutex::simple::RawLock;
        use crossbeam_utils::WaitGroup;

        type ReentrantMutex<T> = super::ReentrantMutex<ReLock<RawLock>, T>;

        let mtx = ReentrantMutex::new(Cell::new(0));
        let mtx = std::sync::Arc::new(mtx);

        let first = WaitGroup::new();
        let second = WaitGroup::new();

        let t = std::thread::spawn({
            let mtx = mtx.clone();
            let first = first.clone();
            let second = second.clone();
            move || {
                first.wait();
                assert!(mtx.try_lock().is_none());
                second.wait();
                let _lock = mtx.lock();

                assert!(_lock.get() == 0 || _lock.get() == 10);
            }
        });

        first.wait();
        let _lock = mtx.lock();
        second.wait();

        assert_eq!(_lock.get(), 0);

        mtx.lock().set(10);

        assert_eq!(_lock.get(), 10);

        drop(_lock);

        t.join().unwrap();
    }
}
