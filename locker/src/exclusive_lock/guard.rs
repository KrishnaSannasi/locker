use super::{
    RawExclusiveGuard, RawExclusiveLock, RawExclusiveLockDowngrade, RawExclusiveLockFair,
    SplittableExclusiveLock,
};
use crate::RawLockInfo;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

pub use crate::guard::{Mapped, Pure, TryMapError};

/// An RAII exclusive guard guard returned by `ExclusiveGuard::map`,
/// which can point to a subfield of the protected data.
///
/// The main difference between `MappedExclusiveGuard` and `ExclusiveGuard`
/// is that the former doesn't support temporarily unlocking and re-locking,
/// since that could introduce soundness issues if the locked object is modified by another thread.
pub type MappedExclusiveGuard<'a, L, T> = ExclusiveGuard<'a, L, T, Mapped>;

/// RAII structure used to release the exclusive access of a lock when dropped.
#[must_use = "if unused the `ExclusiveGuard` will immediately unlock"]
pub struct ExclusiveGuard<'a, L: RawExclusiveLock + RawLockInfo, T: ?Sized, St = Pure> {
    raw: RawExclusiveGuard<'a, L>,
    value: *mut T,
    _repr: PhantomData<(&'a mut T, St)>,
}

unsafe impl<'a, L: RawExclusiveLock + RawLockInfo, T: ?Sized + Send, St> Send
    for ExclusiveGuard<'a, L, T, St>
where
    RawExclusiveGuard<'a, L>: Send,
{
}
unsafe impl<'a, L: RawExclusiveLock + RawLockInfo, T: ?Sized + Sync, St> Sync
    for ExclusiveGuard<'a, L, T, St>
where
    RawExclusiveGuard<'a, L>: Sync,
{
}

impl<L: RawExclusiveLockFair + RawLockInfo, T: ?Sized, St> ExclusiveGuard<'_, L, T, St> {
    /// Unlocks the guard using a fair unlocking protocol
    /// [read more](RawExclusiveLockFair#method.exc_unlock_fair)
    pub fn unlock_fair(g: Self) {
        g.raw.unlock_fair();
    }
}

impl<'a, L: RawExclusiveLock + RawLockInfo, T: ?Sized> ExclusiveGuard<'a, L, T> {
    /// Temporarily yields the lock to another thread if there is one.
    /// [read more](RawExclusiveLock#method.exc_bump)
    pub fn bump(g: &mut Self) {
        g.raw.bump()
    }

    /// Temporarily unlocks the lock to execute the given function.
    ///
    /// This is safe because &mut guarantees that there exist no other references to the data protected by the lock.
    pub fn unlocked<R>(g: &mut Self, f: impl FnOnce() -> R) -> R {
        g.raw.unlocked(f)
    }
}

impl<'a, L: RawExclusiveLockFair + RawLockInfo, T: ?Sized> ExclusiveGuard<'a, L, T> {
    /// Temporarily yields the lock to a waiting thread if there is one.
    /// [read more](RawExclusiveLockFair#method.exc_bump_fair)
    pub fn bump_fair(g: &mut Self) {
        g.raw.bump_fair();
    }

    /// Temporarily unlocks the lock to execute the given function.
    ///
    /// The lock is unlocked a fair unlock protocol.
    ///
    /// This is safe because `&mut` guarantees that there exist no other references to the data protected by the lock.
    pub fn unlocked_fair<R>(g: &mut Self, f: impl FnOnce() -> R) -> R {
        g.raw.unlocked_fair(f)
    }
}

impl<'a, L: RawExclusiveLock + RawLockInfo, T: ?Sized, St> ExclusiveGuard<'a, L, T, St> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Create a new guard from the given raw guard and pointer
            ///
            /// # Safety
            ///
            /// `value` must be valid for as long as this `ExclusiveGuard` is alive
            ///
            /// If this isn't a `MappedExclusiveGuard`, then you must also ensure that `value`
            /// is still valid if the lock is temporarily released and another thread acquires the lock
            pub const unsafe fn from_raw_parts(raw: RawExclusiveGuard<'a, L>, value: *mut T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            /// The inner `RawExclusiveGuard`
            pub const fn raw(&self) -> &RawExclusiveGuard<'a, L> {
                &self.raw
            }

            /// The inner `RawExclusiveGuard`
            ///
            /// # Safety
            ///
            /// * You must not unlock this lock temporarily if this is a mapped lock
            /// * You must not overwrite the raw guard with another raw guard
            pub const unsafe fn raw_mut(&mut self) -> &mut RawExclusiveGuard<'a, L> {
                &mut self.raw
            }
        } else {
            /// Create a new guard from the given raw guard and pointer
            ///
            /// # Safety
            ///
            /// `value` must be valid for as long as this `ExclusiveGuard` is alive
            ///
            /// If this isn't a `MappedExclusiveGuard`, then you must also ensure that `value`
            /// is still valid if the lock is temporarily released and another thread acquires the lock
            pub unsafe fn from_raw_parts(raw: RawExclusiveGuard<'a, L>, value: *mut T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            /// The inner `RawExclusiveGuard`
            pub fn raw(g: &Self) -> &RawExclusiveGuard<'a, L> {
                &g.raw
            }

            /// The inner `RawExclusiveGuard`
            ///
            /// # Safety
            ///
            /// * You must not unlock this lock temporarily if this is a mapped lock
            /// * You must not overwrite the raw guard with another raw guard
            pub unsafe fn raw_mut(g: &mut Self) -> &mut RawExclusiveGuard<'a, L> {
                &mut g.raw
            }
        }
    }

    /// Decomposes the `ExclusiveGuard` into it's raw parts
    ///
    /// Returns the [`RawExclusiveGuard`] and a pointer to the guarded value.
    ///
    /// It is safe to write using this pointer.
    ///
    /// After calling this function the caller is responsible for ensuring that
    /// the pointer is not dereferenced while unguarded by the lock. While the guard is alive
    /// it is always safe access the guarded value. While the guard is temporarily unlocked,
    /// it is not safe to access the guarded value.
    ///
    /// If the guarded value is not the original value (i.e. if mapped), then it is not safe to
    /// access the guarded value after the `RawExclusiveGuard` unlocks, even temporarily.
    pub fn into_raw_parts(g: Self) -> (RawExclusiveGuard<'a, L>, *mut T) {
        (g.raw, g.value)
    }

    /// Make a new `MappedExclusiveGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `ExclusiveGuard` passed in already locked the data.
    ///
    /// This is an associated function that needs to be used as `ExclusiveGuard::map(...)`.
    /// A method would interfere with methods of the same name on the contents of the locked data.
    pub fn map<F, U: ?Sized>(
        g: Self,
        f: impl FnOnce(&mut T) -> &mut U,
    ) -> MappedExclusiveGuard<'a, L, U> {
        let value = f(unsafe { &mut *g.value });

        unsafe { ExclusiveGuard::from_raw_parts(g.raw, value) }
    }

    /// Attempts to make a new `MappedExclusiveGuard` for a component of the locked data.
    /// The original guard is return if the closure returns `Err` as well as the error.
    ///
    /// This operation cannot fail as the `ExclusiveGuard` passed in already locked the data.
    ///
    /// This is an associated function that needs to be used as `ExclusiveGuard::try_map(...)`.
    /// A method would interfere with methods of the same name on the contents of the locked data.
    pub fn try_map<E, U: ?Sized>(
        g: Self,
        f: impl FnOnce(&mut T) -> Result<&mut U, E>,
    ) -> Result<MappedExclusiveGuard<'a, L, U>, TryMapError<E, Self>> {
        match f(unsafe { &mut *g.value }) {
            Err(e) => Err(TryMapError(e, g)),
            Ok(value) => Ok(unsafe { ExclusiveGuard::from_raw_parts(g.raw, value) }),
        }
    }
}

impl<'a, L: SplittableExclusiveLock + RawLockInfo, T: ?Sized, St> ExclusiveGuard<'a, L, T, St> {
    /// Make a two new `MappedExclusiveGuard`s for a component of the locked data.
    ///
    /// This operation cannot fail as the `ExclusiveGuard` passed in already locked the data.
    ///
    /// This is an associated function that needs to be used as `ExclusiveGuard::split_map(...)`.
    /// A method would interfere with methods of the same name on the contents of the locked data.
    pub fn split_map<U: ?Sized, V: ?Sized>(
        g: Self,
        f: impl FnOnce(&mut T) -> (&mut U, &mut V),
    ) -> (
        MappedExclusiveGuard<'a, L, U>,
        MappedExclusiveGuard<'a, L, V>,
    ) {
        let (u, v) = f(unsafe { &mut *g.value });

        let u_lock = g.raw.clone();
        let v_lock = g.raw;

        (
            unsafe { ExclusiveGuard::from_raw_parts(u_lock, u) },
            unsafe { ExclusiveGuard::from_raw_parts(v_lock, v) },
        )
    }

    /// Attempts to make two new `MappedExclusiveGuard`s for a component of the locked data.
    /// The original guard is return if the closure returns `Err` as well as the error.
    ///
    /// This operation cannot fail as the `ExclusiveGuard` passed in already locked the data.
    ///
    /// This is an associated function that needs to be used as `ExclusiveGuard::try_split_map(...)`.
    /// A method would interfere with methods of the same name on the contents of the locked data.
    #[allow(clippy::type_complexity)]
    pub fn try_split_map<E, U: ?Sized, V: ?Sized>(
        g: Self,
        f: impl FnOnce(&mut T) -> Result<(&mut U, &mut V), E>,
    ) -> Result<
        (
            ExclusiveGuard<'a, L, U, Mapped>,
            ExclusiveGuard<'a, L, V, Mapped>,
        ),
        TryMapError<E, Self>,
    > {
        match f(unsafe { &mut *g.value }) {
            Err(e) => Err(TryMapError(e, g)),
            Ok((u, v)) => {
                let u_lock = g.raw.clone();
                let v_lock = g.raw;

                Ok((
                    unsafe { ExclusiveGuard::from_raw_parts(u_lock, u) },
                    unsafe { ExclusiveGuard::from_raw_parts(v_lock, v) },
                ))
            }
        }
    }
}

impl<'a, L: RawExclusiveLockDowngrade + RawLockInfo, T: ?Sized> ExclusiveGuard<'a, L, T>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    /// Atomically downgrades a *exc lock* into a *shr lock* without allowing any new
    /// *exc locks* in the meantime.
    pub fn downgrade(g: Self) -> crate::share_lock::ShareGuard<'a, L, T> {
        unsafe { crate::share_lock::ShareGuard::from_raw_parts(g.raw.downgrade(), g.value) }
    }
}

impl<L: RawExclusiveLock + RawLockInfo, T: ?Sized, St> Deref for ExclusiveGuard<'_, L, T, St> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

impl<L: RawExclusiveLock + RawLockInfo, T: ?Sized, St> DerefMut for ExclusiveGuard<'_, L, T, St> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value }
    }
}
