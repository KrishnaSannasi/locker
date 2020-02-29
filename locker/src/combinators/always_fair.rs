use crate::exclusive_lock::{
    RawExclusiveLock, RawExclusiveLockDowngrade, RawExclusiveLockFair, SplittableExclusiveLock,
};
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

use crate::mutex::RawMutex;
use crate::reentrant::RawReentrantMutex;
use crate::rwlock::RawRwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fair<L: ?Sized>(pub L);

unsafe impl<L: RawMutex + RawExclusiveLockFair> RawMutex for Fair<L> {}
unsafe impl<L: RawRwLock + RawExclusiveLockFair + RawShareLockFair> RawRwLock for Fair<L> {}
unsafe impl<L: RawReentrantMutex + RawShareLockFair> RawReentrantMutex for Fair<L> {}

unsafe impl<L: RawLockInfo> RawLockInfo for Fair<L> {
    const INIT: Self = Self(RawLockInfo::INIT);

    type ExclusiveGuardTraits = <L as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <L as RawLockInfo>::ShareGuardTraits;
}

unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLock for Fair<L> {
    fn exc_lock(&self) {
        self.0.exc_lock()
    }

    fn exc_try_lock(&self) -> bool {
        self.0.exc_try_lock()
    }

    unsafe fn exc_unlock(&self) {
        self.0.exc_unlock_fair()
    }

    unsafe fn exc_bump(&self) {
        self.0.exc_bump_fair()
    }
}

unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for Fair<L> {
    unsafe fn exc_unlock_fair(&self) {
        self.0.exc_unlock_fair()
    }

    unsafe fn exc_bump_fair(&self) {
        self.0.exc_bump_fair()
    }
}

unsafe impl<L: ?Sized> RawExclusiveLockDowngrade for Fair<L>
where
    L: RawExclusiveLockDowngrade + RawExclusiveLockFair + RawShareLockFair,
{
    unsafe fn downgrade(&self) {
        self.0.downgrade()
    }
}

unsafe impl<L: ?Sized + SplittableExclusiveLock + RawExclusiveLockFair> SplittableExclusiveLock
    for Fair<L>
{
    unsafe fn exc_split(&self) {
        self.0.exc_split()
    }
}

unsafe impl<L: ?Sized + RawShareLockFair> RawShareLock for Fair<L> {
    fn shr_lock(&self) {
        self.0.shr_lock()
    }

    fn shr_try_lock(&self) -> bool {
        self.0.shr_try_lock()
    }

    unsafe fn shr_split(&self) {
        self.0.shr_split()
    }

    unsafe fn shr_unlock(&self) {
        self.0.shr_unlock_fair()
    }

    unsafe fn shr_bump(&self) {
        self.0.shr_bump_fair()
    }
}

unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for Fair<L> {
    unsafe fn shr_unlock_fair(&self) {
        self.0.shr_unlock_fair()
    }

    unsafe fn shr_bump_fair(&self) {
        self.0.shr_bump_fair()
    }
}
