//! A wrapper around an [`RawExclusiveLock`] that allows it to be used as a
//! reentrant lock

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair, RawExclusiveLockTimed};
use crate::share_lock::{RawShareLock, RawShareLockFair, RawShareLockTimed};

use super::ThreadInfo;

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// A wrapper around a [`RawExclusiveLock`] that allows it to be used as a
        /// reentrant mutex
        pub struct ReLock<L, I = super::std_thread::StdThreadInfo> {
            inner: L,
            thread_info: I,
            owner: AtomicUsize,
            count: Cell<usize>,
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

unsafe impl<L: Sync + crate::mutex::RawMutex, I: Sync> Sync for ReLock<L, I> {}

impl<L, I> ReLock<L, I> {
    /// # Safety
    ///
    /// `inner` must not be shared
    #[inline]
    pub const unsafe fn from_raw_parts(inner: L, thread_info: I) -> Self {
        Self {
            inner,
            thread_info,
            owner: AtomicUsize::new(0),
            count: Cell::new(0),
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

unsafe impl<L: crate::mutex::RawMutex, I: ThreadInfo> super::RawReentrantMutex for ReLock<L, I> {}
unsafe impl<L: crate::RawLockInfo, I: ThreadInfo> crate::RawLockInfo for ReLock<L, I> {
    const INIT: Self = unsafe { Self::from_raw_parts(L::INIT, I::INIT) };

    type ExclusiveGuardTraits = std::convert::Infallible;
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

impl<L: RawExclusiveLock, I: ThreadInfo> ReLock<L, I> {
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
            debug_assert_eq!(self.count.get(), 0);
        }

        true
    }

    #[inline]
    fn unlock_internal(&self, unlock_slow: impl FnOnce()) {
        if let Some(count) = self.count.get().checked_sub(1) {
            self.count.set(count);
        } else {
            self.owner.store(0, Ordering::Relaxed);
            unlock_slow()
        }
    }
}

unsafe impl<L: RawExclusiveLock, I: ThreadInfo> RawShareLock for ReLock<L, I> {
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
        #[cfg(debug_assertions)]
        {
            assert_eq!(
                self.owner.load(Ordering::Relaxed),
                self.thread_info.id().get()
            );
        }

        self.count.set(
            self.count
                .get()
                .checked_add(1)
                .expect("tried to create too many reentrant locks"),
        );
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
        if self.count.get() == 0 {
            self.inner.exc_bump();
        }
    }
}

unsafe impl<L: RawExclusiveLockFair, I: ThreadInfo> RawShareLockFair for ReLock<L, I> {
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        self.unlock_internal(
            #[cold]
            || self.inner.exc_unlock_fair(),
        )
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        if self.count.get() == 0 {
            self.inner.exc_bump_fair();
        }
    }
}

unsafe impl<L: crate::RawTimedLock, I: ThreadInfo> crate::RawTimedLock for ReLock<L, I> {
    type Instant = L::Instant;
    type Duration = L::Duration;
}

unsafe impl<L: RawExclusiveLockTimed, I: ThreadInfo> RawShareLockTimed for ReLock<L, I> {
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
