use super::{RawShareGuard, RawShareLock, RawShareLockFair};
use crate::RawLockInfo;
use std::marker::PhantomData;
use std::ops::Deref;

pub use crate::guard::{Mapped, Pure, TryMapError};

/// An RAII exclusive guard guard returned by `ShareGuard::map`,
/// which can point to a subfield of the protected data.
///
/// The main difference between `MappedShareGuard` and `ShareGuard`
/// is that the former doesn't support temporarily unlocking and re-locking,
/// since that could introduce soundness issues if the locked object is modified by another thread.
pub type MappedShareGuard<'a, L, T> = ShareGuard<'a, L, T, Mapped>;

/// RAII structure used to release the shared access of a lock when dropped.
#[must_use = "if unused the `ShareGuard` will immediately unlock"]
pub struct ShareGuard<'a, L: RawShareLock + RawLockInfo, T: ?Sized, St = Pure> {
    raw: RawShareGuard<'a, L>,
    value: *const T,
    _repr: PhantomData<(&'a T, St)>,
}

unsafe impl<'a, L: RawShareLock + RawLockInfo, T: ?Sized + Sync, St> Send
    for ShareGuard<'a, L, T, St>
where
    RawShareGuard<'a, L>: Send,
{
}
unsafe impl<'a, L: RawShareLock + RawLockInfo, T: ?Sized + Sync, St> Sync
    for ShareGuard<'a, L, T, St>
where
    RawShareGuard<'a, L>: Sync,
{
}

impl<L: RawShareLockFair + RawLockInfo, T: ?Sized, St> ShareGuard<'_, L, T, St> {
    /// Unlocks the guard using a fair unlocking protocol
    /// [read more](RawShareLockFair#method.shr_unlock_fair)
    pub fn unlock_fair(g: Self) {
        g.raw.unlock_fair();
    }
}

impl<'a, L: RawShareLock + RawLockInfo, T: ?Sized> ShareGuard<'a, L, T> {
    /// Temporarily yields the lock to another thread if there is one.
    /// [read more](RawShareLock#method.shr_bump)
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

impl<'a, L: RawShareLockFair + RawLockInfo, T: ?Sized> ShareGuard<'a, L, T> {
    /// Temporarily yields the lock to a waiting thread if there is one.
    /// [read more](RawShareLockFair#method.shr_bump_fair)
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

impl<'a, L: RawShareLock + RawLockInfo, T: ?Sized, St> ShareGuard<'a, L, T, St> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Create a new guard from the given raw guard and pointer
            ///
            /// # Safety
            ///
            /// `value` must be valid for as long as this `ShareGuard` is alive
            ///
            /// If this isn't a `MappedShareGuard`, then you must also ensure that `value`
            /// is still valid if the lock is temporarily released and another thread acquires the lock
            pub const unsafe fn from_raw_parts(raw: RawShareGuard<'a, L>, value: *const T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            /// The inner `RawShareGuard`
            pub const fn raw(&self) -> &RawShareGuard<'a, L> {
                &self.raw
            }

            /// The inner `RawShareGuard`
            ///
            /// # Safety
            ///
            /// * You must not unlock this lock temporarily if this is a mapped lock
            /// * You must not overwrite the raw guard with another raw guard
            pub const unsafe fn raw_mut(&mut self) -> &mut RawShareGuard<'a, L> {
                &mut self.raw
            }
        } else {
            /// Create a new guard from the given raw guard and pointer
            ///
            /// # Safety
            ///
            /// `value` must be valid for as long as this `ShareGuard` is alive
            ///
            /// If this isn't a `MappedShareGuard`, then you must also ensure that `value`
            /// is still valid if the lock is temporarily released and another thread acquires the lock
            pub unsafe fn from_raw_parts(raw: RawShareGuard<'a, L>, value: *const T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            /// The inner `RawShareGuard`
            pub fn raw(g: &Self) -> &RawShareGuard<'a, L> {
                &g.raw
            }

            /// The inner `RawShareGuard`
            ///
            /// # Safety
            ///
            /// * You must not unlock this lock temporarily if this is a mapped lock
            /// * You must not overwrite the raw guard with another raw guard
            pub unsafe fn raw_mut(g: &mut Self) -> &mut RawShareGuard<'a, L> {
                &mut g.raw
            }
        }
    }

    /// Decomposes the `ShareGuard` into it's raw parts
    ///
    /// Returns the [`RawShareGuard`] and a pointer to the guarded value.
    ///
    /// It is not safe to write using this pointer.
    ///
    /// After calling this function the caller is responsible for ensuring that
    /// the pointer is not dereferenced while unguarded by the lock. While the guard is alive
    /// it is always safe access the guarded value. While the guard is temporarily unlocked,
    /// it is not safe to access the guarded value.
    ///
    /// If the guarded value is not the original value (i.e. if mapped), then it is not safe to
    /// access the guarded value after the `RawShareGuard` unlocks, even temporarily.
    pub fn into_raw_parts(g: Self) -> (RawShareGuard<'a, L>, *const T) {
        (g.raw, g.value)
    }

    /// Make a new `MappedExclusiveGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `ExclusiveGuard` passed in already locked the data.
    ///
    /// This is an associated function that needs to be used as `ExclusiveGuard::map(...)`.
    /// A method would interfere with methods of the same name on the contents of the locked data.
    pub fn map<F, U: ?Sized>(g: Self, f: impl FnOnce(&T) -> &U) -> ShareGuard<'a, L, U, Mapped> {
        let value = f(unsafe { &*g.value });

        unsafe { ShareGuard::from_raw_parts(g.raw, value) }
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
        f: impl FnOnce(&T) -> Result<&U, E>,
    ) -> Result<ShareGuard<'a, L, U, Mapped>, TryMapError<E, Self>> {
        match f(unsafe { &*g.value }) {
            Err(e) => Err(TryMapError(e, g)),
            Ok(value) => Ok(unsafe { ShareGuard::from_raw_parts(g.raw, value) }),
        }
    }

    /// Make a two new `MappedExclusiveGuard`s for a component of the locked data.
    ///
    /// This operation cannot fail as the `ExclusiveGuard` passed in already locked the data.
    ///
    /// This is an associated function that needs to be used as `ExclusiveGuard::split_map(...)`.
    /// A method would interfere with methods of the same name on the contents of the locked data.
    pub fn split_map<U: ?Sized, V: ?Sized>(
        g: Self,
        f: impl FnOnce(&T) -> (&U, &V),
    ) -> (ShareGuard<'a, L, U, Mapped>, ShareGuard<'a, L, V, Mapped>) {
        let (u, v) = f(unsafe { &*g.value });

        let u_lock = g.raw.clone();
        let v_lock = g.raw;

        (unsafe { ShareGuard::from_raw_parts(u_lock, u) }, unsafe {
            ShareGuard::from_raw_parts(v_lock, v)
        })
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
        f: impl FnOnce(&T) -> Result<(&U, &V), E>,
    ) -> Result<(ShareGuard<'a, L, U, Mapped>, ShareGuard<'a, L, V, Mapped>), TryMapError<E, Self>>
    {
        match f(unsafe { &*g.value }) {
            Err(e) => Err(TryMapError(e, g)),
            Ok((u, v)) => {
                let u_lock = g.raw.clone();
                let v_lock = g.raw;

                Ok((unsafe { ShareGuard::from_raw_parts(u_lock, u) }, unsafe {
                    ShareGuard::from_raw_parts(v_lock, v)
                }))
            }
        }
    }
}

impl<L: RawShareLock + RawLockInfo, T: ?Sized, St> Deref for ShareGuard<'_, L, T, St> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

impl<L: RawShareLock + RawLockInfo, T: ?Sized, St> Clone for ShareGuard<'_, L, T, St> {
    fn clone(&self) -> Self {
        unsafe { Self::from_raw_parts(self.raw.clone(), &*self.value) }
    }
}
