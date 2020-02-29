pub mod guard;
#[doc(hidden)]
pub mod raw;

pub use guard::{ExclusiveGuard, MappedExclusiveGuard};
pub use raw::RawExclusiveGuard;

/// # Safety
///
/// * `exc_unlock` must be called before before `exc_lock`,
/// `exc_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented)
pub unsafe trait RawExclusiveLock {
    /// exc locks the lock
    ///
    /// blocks until lock is acquired
    fn exc_lock(&self);

    /// attempts to exc lock the lock
    ///
    /// returns true on success
    fn exc_try_lock(&self) -> bool;

    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_unlock(&self);

    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_bump(&self) {
        self.exc_unlock();
        self.exc_lock();
    }
}

/// # Safety
///
/// * `exc_unlock` must be called `n` times before `exc_lock`,
/// `exc_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented), where `n` is the number of times
/// `exc_lock` and `exc_split` are called combined
pub unsafe trait SplittableExclusiveLock: RawExclusiveLock {
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_split(&self);
}

/// # Safety
///
/// same safety notes about `exc_unlock` apply to `exc_unlock_fair`
/// same safety notes about `exc_bump` apply to `exc_bump_fair`
pub unsafe trait RawExclusiveLockFair: RawExclusiveLock {
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_unlock_fair(&self);

    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_bump_fair(&self) {
        self.exc_unlock_fair();
        self.exc_lock();
    }
}

pub unsafe trait RawExclusiveLockDowngrade:
    RawExclusiveLock + crate::share_lock::RawShareLock
{
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn downgrade(&self);
}

// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for &L {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for &mut L {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Box<L> {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Arc<L> {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Rc<L> {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for &L {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for &mut L {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair
//     for crate::alloc_prelude::Box<L>
// {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair
//     for crate::alloc_prelude::Arc<L>
// {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for crate::alloc_prelude::Rc<L> {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// unsafe impl<L: ?Sized + SplittableExclusiveLock> SplittableExclusiveLock for &L {
//     unsafe fn exc_split(&self) {
//         L::exc_split(self)
//     }
// }

// unsafe impl<L: ?Sized + SplittableExclusiveLock> SplittableExclusiveLock for &mut L {
//     unsafe fn exc_split(&self) {
//         L::exc_split(self)
//     }
// }
