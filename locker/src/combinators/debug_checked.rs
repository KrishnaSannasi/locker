use crate::exclusive_lock::{
    RawExclusiveLock, RawExclusiveLockDowngrade, RawExclusiveLockFair, SplittableExclusiveLock,
};
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

use crate::mutex::RawMutex;
use crate::remutex::RawReentrantMutex;
use crate::rwlock::RawRwLock;

#[cfg(not(debug_assertions))]
use std::marker::PhantomData;

/// On debug mode this wraps a lock and defers to the lock
/// On release mode, this holds no lock and every operation on it is a no-op, and this type becomes zero-sized
///
/// This is useful when you need to performance in your code, but would like to easily check if your code does
/// indeed have any problems
///
/// This should only be used on `local` locks because it is impossible to enforce the guarantees of the lock traits
/// in a multi-threaded environment without actually holding a lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DebugChecked<L: ?Sized> {
    #[cfg(not(debug_assertions))]
    inner: PhantomData<L>,
    #[cfg(debug_assertions)]
    inner: L,
}

unsafe impl<L: RawMutex + RawExclusiveLockFair> RawMutex for DebugChecked<L> {}
unsafe impl<L: RawRwLock + RawExclusiveLockFair + RawShareLockFair> RawRwLock for DebugChecked<L> {}
unsafe impl<L: RawReentrantMutex + RawShareLockFair> RawReentrantMutex for DebugChecked<L> {}

impl<L> DebugChecked<L> {
    /// # Safety
    ///
    /// It must be impossible to invalidate the invariants of the lock traits for the entire lifetime of this lock
    #[allow(unused_variables)]
    pub unsafe fn from_lock(inner: L) -> Self {
        Self {
            #[cfg(not(debug_assertions))]
            inner: PhantomData,

            #[cfg(debug_assertions)]
            inner,
        }
    }
}

impl<L: crate::Init> DebugChecked<L> {
    /// # Safety
    ///
    /// It must be impossible to invalidate the invariants of the lock traits for the entire lifetime of this lock
    pub unsafe fn new() -> Self {
        Self {
            #[cfg(not(debug_assertions))]
            inner: PhantomData,

            #[cfg(debug_assertions)]
            inner: crate::Init::INIT,
        }
    }
}

unsafe impl<L: RawLockInfo + ?Sized> RawLockInfo for DebugChecked<L> {
    type ExclusiveGuardTraits = <L as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <L as RawLockInfo>::ShareGuardTraits;
}

unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLock for DebugChecked<L> {
    fn exc_lock(&self) {
        #[cfg(debug_assertions)]
        self.inner.exc_lock()
    }

    fn exc_try_lock(&self) -> bool {
        #[cfg(debug_assertions)]
        {
            self.inner.exc_try_lock()
        }

        #[cfg(not(debug_assertions))]
        {
            true
        }
    }

    unsafe fn exc_unlock(&self) {
        #[cfg(debug_assertions)]
        self.inner.exc_unlock_fair()
    }

    unsafe fn exc_bump(&self) {
        #[cfg(debug_assertions)]
        self.inner.exc_bump_fair()
    }
}

unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for DebugChecked<L> {
    unsafe fn exc_unlock_fair(&self) {
        #[cfg(debug_assertions)]
        self.inner.exc_unlock_fair()
    }

    unsafe fn exc_bump_fair(&self) {
        #[cfg(debug_assertions)]
        self.inner.exc_bump_fair()
    }
}

unsafe impl<L: ?Sized> RawExclusiveLockDowngrade for DebugChecked<L>
where
    L: RawExclusiveLockDowngrade + RawExclusiveLockFair + RawShareLockFair,
{
    unsafe fn downgrade(&self) {
        #[cfg(debug_assertions)]
        self.inner.downgrade()
    }
}

unsafe impl<L: ?Sized + SplittableExclusiveLock + RawExclusiveLockFair> SplittableExclusiveLock
    for DebugChecked<L>
{
    unsafe fn exc_split(&self) {
        #[cfg(debug_assertions)]
        self.inner.exc_split()
    }
}

unsafe impl<L: ?Sized + RawShareLockFair> RawShareLock for DebugChecked<L> {
    fn shr_lock(&self) {
        #[cfg(debug_assertions)]
        self.inner.shr_lock()
    }

    fn shr_try_lock(&self) -> bool {
        #[cfg(debug_assertions)]
        {
            self.inner.shr_try_lock()
        }
        #[cfg(not(debug_assertions))]
        false
    }

    unsafe fn shr_split(&self) {
        #[cfg(debug_assertions)]
        self.inner.shr_split()
    }

    unsafe fn shr_unlock(&self) {
        #[cfg(debug_assertions)]
        self.inner.shr_unlock_fair()
    }

    unsafe fn shr_bump(&self) {
        #[cfg(debug_assertions)]
        self.inner.shr_bump_fair()
    }
}

unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for DebugChecked<L> {
    unsafe fn shr_unlock_fair(&self) {
        #[cfg(debug_assertions)]
        self.inner.shr_unlock_fair()
    }

    unsafe fn shr_bump_fair(&self) {
        #[cfg(debug_assertions)]
        self.inner.shr_bump_fair()
    }
}
