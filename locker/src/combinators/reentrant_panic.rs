use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockDowngrade, RawExclusiveLockFair};
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

use crate::mutex::RawMutex;
use crate::reentrant::ThreadInfo;
use crate::rwlock::RawRwLock;

use std::sync::atomic::{AtomicUsize, Ordering};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Wraps a lock and panics on reentrant exclusive lock, leaves the
        /// share lock untouched
        #[derive(Debug)]
        pub struct ReentrantPanic<L: ?Sized, I = crate::reentrant::std_thread::StdThreadInfo> {
            owner: AtomicUsize,
            thread_info: I,
            inner: L,
        }
    } else {
        /// Wraps a lock and panics on reentrant exclusive lock, leaves the
        /// share lock untouched
        #[derive(Debug)]
        pub struct ReentrantPanic<L: ?Sized, I> {
            owner: AtomicUsize,
            thread_info: I,
            inner: L,
        }
    }
}

impl<L, I> ReentrantPanic<L, I> {
    pub const fn wrap(inner: L, thread_info: I) -> Self {
        Self {
            inner,
            thread_info,
            owner: AtomicUsize::new(0),
        }
    }
}

unsafe impl<L: RawMutex, I: ThreadInfo> RawMutex for ReentrantPanic<L, I> {}
unsafe impl<L: RawRwLock, I: ThreadInfo> RawRwLock for ReentrantPanic<L, I> {}

unsafe impl<L: RawLockInfo, I: ThreadInfo> RawLockInfo for ReentrantPanic<L, I> {
    const INIT: Self = Self {
        inner: RawLockInfo::INIT,
        thread_info: ThreadInfo::INIT,
        owner: AtomicUsize::new(0),
    };

    type ExclusiveGuardTraits = <L as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <L as RawLockInfo>::ShareGuardTraits;
}

unsafe impl<L: ?Sized + RawExclusiveLock, I: ThreadInfo> RawExclusiveLock for ReentrantPanic<L, I> {
    fn exc_lock(&self) {
        let curr = self.thread_info.id().get();
        let owner = self.owner.load(Ordering::Acquire);

        assert_ne!(
            owner, curr,
            "tried to lock a locked exclusive lock from the same thread!"
        );

        self.inner.exc_lock();

        self.owner.store(curr, Ordering::Release);
    }

    fn exc_try_lock(&self) -> bool {
        if self.inner.exc_try_lock() {
            let curr = self.thread_info.id().get();
            self.owner.store(curr, Ordering::Release);
            true
        } else {
            false
        }
    }

    unsafe fn exc_unlock(&self) {
        self.owner.store(0, Ordering::Release);
        self.inner.exc_unlock();
    }

    unsafe fn exc_bump(&self) {
        let owner = self.owner.swap(0, Ordering::Acquire);
        self.inner.exc_bump();
        self.owner.store(owner, Ordering::Release);
    }
}

unsafe impl<L: ?Sized + RawExclusiveLockFair, I: ThreadInfo> RawExclusiveLockFair
    for ReentrantPanic<L, I>
{
    unsafe fn exc_unlock_fair(&self) {
        self.owner.store(0, Ordering::Release);
        self.inner.exc_unlock_fair();
    }

    unsafe fn exc_bump_fair(&self) {
        let owner = self.owner.swap(0, Ordering::Acquire);
        self.inner.exc_bump_fair();
        self.owner.store(owner, Ordering::Release);
    }
}

unsafe impl<L: ?Sized + RawExclusiveLockDowngrade, I: ThreadInfo> RawExclusiveLockDowngrade
    for ReentrantPanic<L, I>
{
    unsafe fn downgrade(&self) {
        self.owner.store(0, Ordering::Release);
        self.inner.downgrade();
    }
}

unsafe impl<L: ?Sized + RawShareLock, I: ThreadInfo> RawShareLock for ReentrantPanic<L, I> {
    fn shr_lock(&self) {
        self.inner.shr_lock()
    }

    fn shr_try_lock(&self) -> bool {
        self.inner.shr_try_lock()
    }

    unsafe fn shr_split(&self) {
        self.inner.shr_split()
    }

    unsafe fn shr_unlock(&self) {
        self.inner.shr_unlock()
    }

    unsafe fn shr_bump(&self) {
        self.inner.shr_bump()
    }
}

unsafe impl<L: ?Sized + RawShareLockFair, I: ThreadInfo> RawShareLockFair for ReentrantPanic<L, I> {
    unsafe fn shr_unlock_fair(&self) {
        self.inner.shr_unlock_fair()
    }

    unsafe fn shr_bump_fair(&self) {
        self.inner.shr_bump_fair()
    }
}

#[cfg(feature = "std")]
#[test]
#[should_panic = "tried to lock a locked exclusive lock from the same thread!"]
fn reentrant_panic() {
    let mtx = crate::mutex::Mutex::<ReentrantPanic<crate::mutex::simple::RawLock>, _>::new(10);

    let mut _guard = mtx.lock();

    assert!(mtx.try_lock().is_none());

    crate::exclusive_lock::ExclusiveGuard::bump(&mut _guard);

    mtx.lock();

    drop(_guard);
}
