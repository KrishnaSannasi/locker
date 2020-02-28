pub mod guard;
#[doc(hidden)]
pub mod raw;

pub use guard::{MappedUniqueGuard, UniqueGuard};
pub use raw::RawUniqueGuard;

use crate::RawLockInfo;

pub trait RawUniqueLockExt: RawUniqueLock + RawLockInfo + Sized {
    fn raw_uniq_lock(&self) -> RawUniqueGuard<Self>;

    fn try_raw_uniq_lock(&self) -> Option<RawUniqueGuard<Self>>;
}

impl<L: RawUniqueLock + RawLockInfo> RawUniqueLockExt for L
where
    Self::UniqueGuardTraits: crate::Inhabitted,
{
    fn raw_uniq_lock(&self) -> RawUniqueGuard<Self> {
        RawUniqueGuard::new(self, unsafe { std::mem::zeroed() })
    }

    fn try_raw_uniq_lock(&self) -> Option<RawUniqueGuard<Self>> {
        RawUniqueGuard::try_new(self, unsafe { std::mem::zeroed() })
    }
}

/// # Safety
///
/// * `uniq_unlock` must be called before before `uniq_lock`,
/// `uniq_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented)
pub unsafe trait RawUniqueLock {
    /// uniq locks the lock
    ///
    /// blocks until lock is acquired
    fn uniq_lock(&self);

    /// attempts to uniq lock the lock
    ///
    /// returns true on success
    fn uniq_try_lock(&self) -> bool;

    /// # Safety
    ///
    /// * This lock must be uniq locked before calling this function
    /// * the lock must not have been moved since it was locked
    unsafe fn uniq_unlock(&self);
}

/// # Safety
///
/// * `uniq_unlock` must be called `n` times before `uniq_lock`,
/// `uniq_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented), where `n` is the number of times
/// `uniq_lock` and `uniq_split` are called combined
pub unsafe trait SplittableUniqueLock: RawUniqueLock {
    /// # Safety
    ///
    /// * the caller must own a unique lock
    /// * the lock must not have been moved since it was locked
    unsafe fn uniq_split(&self);
}

unsafe impl<L: ?Sized + RawUniqueLock> RawUniqueLock for &L {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }
}

unsafe impl<L: ?Sized + RawUniqueLock> RawUniqueLock for &mut L {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawUniqueLock> RawUniqueLock for crate::alloc_prelude::Box<L> {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawUniqueLock> RawUniqueLock for crate::alloc_prelude::Arc<L> {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawUniqueLock> RawUniqueLock for crate::alloc_prelude::Rc<L> {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }
}

unsafe impl<L: ?Sized + SplittableUniqueLock> SplittableUniqueLock for &L {
    unsafe fn uniq_split(&self) {
        L::uniq_split(self)
    }
}

unsafe impl<L: ?Sized + SplittableUniqueLock> SplittableUniqueLock for &mut L {
    unsafe fn uniq_split(&self) {
        L::uniq_split(self)
    }
}
