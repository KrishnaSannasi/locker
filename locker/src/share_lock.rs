pub mod guard;
#[doc(hidden)]
pub mod raw;

pub use guard::{MappedShareGuard, ShareGuard};
pub use raw::RawShareGuard;

/// # Safety
///
/// * `shr_unlock` must be called `n` times before `uniq_lock`,
/// `uniq_try_lock` can succeed (provided that `RawExclusiveLock` is implemented),
/// where `n` is the number of times `shr_lock` and `shr_split` are called combined
pub unsafe trait RawShareLock {
    /// shr locks the lock
    ///
    /// blocks until lock is acquired
    fn shr_lock(&self);

    /// attempts to shr lock the lock
    ///
    /// returns true on success
    fn shr_try_lock(&self) -> bool;

    /// # Safety
    ///
    /// * the caller must own a shr lock
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_split(&self);

    /// # Safety
    ///
    /// * the caller must own a shr lock
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_unlock(&self);

    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_bump(&self) {
        self.shr_unlock();
        self.shr_lock();
    }
}

/// # Safety
///
/// same safety notes about `shr_unlock` apply to `shr_unlock_fair`
/// same safety notes about `shr_bump` apply to `shr_bump_fair`
pub unsafe trait RawShareLockFair: RawShareLock {
    /// # Safety
    ///
    /// * the caller must own a shr lock
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_unlock_fair(&self);

    /// # Safety
    ///
    /// * the caller must own a shr lock
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_bump_fair(&self) {
        self.shr_unlock_fair();
        self.shr_lock();
    }
}

unsafe impl<L: ?Sized + RawShareLock> RawShareLock for &L {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }

    #[inline(always)]
    unsafe fn shr_bump(&self) {
        L::shr_bump(self)
    }
}

unsafe impl<L: ?Sized + RawShareLock> RawShareLock for &mut L {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }

    #[inline(always)]
    unsafe fn shr_bump(&self) {
        L::shr_bump(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Box<L> {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }

    #[inline(always)]
    unsafe fn shr_bump(&self) {
        L::shr_bump(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Arc<L> {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }

    #[inline(always)]
    unsafe fn shr_bump(&self) {
        L::shr_bump(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Rc<L> {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }

    #[inline(always)]
    unsafe fn shr_bump(&self) {
        L::shr_bump(self)
    }
}

unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for &L {
    #[inline(always)]
    unsafe fn shr_unlock_fair(&self) {
        L::shr_unlock_fair(self)
    }

    #[inline(always)]
    unsafe fn shr_bump_fair(&self) {
        L::shr_bump_fair(self)
    }
}

unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for &mut L {
    #[inline(always)]
    unsafe fn shr_unlock_fair(&self) {
        L::shr_unlock_fair(self)
    }

    #[inline(always)]
    unsafe fn shr_bump_fair(&self) {
        L::shr_bump_fair(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for crate::alloc_prelude::Box<L> {
    #[inline(always)]
    unsafe fn shr_unlock_fair(&self) {
        L::shr_unlock_fair(self)
    }

    #[inline(always)]
    unsafe fn shr_bump_fair(&self) {
        L::shr_bump_fair(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for crate::alloc_prelude::Arc<L> {
    #[inline(always)]
    unsafe fn shr_unlock_fair(&self) {
        L::shr_unlock_fair(self)
    }

    #[inline(always)]
    unsafe fn shr_bump_fair(&self) {
        L::shr_bump_fair(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for crate::alloc_prelude::Rc<L> {
    #[inline(always)]
    unsafe fn shr_unlock_fair(&self) {
        L::shr_unlock_fair(self)
    }

    #[inline(always)]
    unsafe fn shr_bump_fair(&self) {
        L::shr_bump_fair(self)
    }
}
