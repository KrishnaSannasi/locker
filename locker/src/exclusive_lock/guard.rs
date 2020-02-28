use super::{RawExclusiveGuard, RawExclusiveLock, RawExclusiveLockFair, SplittableExclusiveLock};
use crate::RawLockInfo;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub enum Pure {}
pub enum Mapped {}

pub struct TryMapError<E, G>(pub E, pub G);

pub type MappedExclusiveGuard<'a, L, T> = ExclusiveGuard<'a, L, T, Mapped>;
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

impl<'a, L: RawExclusiveLock + RawLockInfo, T: ?Sized, St> ExclusiveGuard<'a, L, T, St> {
    pub unsafe fn from_raw_parts(raw: RawExclusiveGuard<'a, L>, value: &'a mut T) -> Self {
        Self {
            raw,
            value,
            _repr: PhantomData,
        }
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn raw(&self) -> &RawExclusiveGuard<L> {
        &self.raw
    }

    pub fn map<F: FnOnce(&mut T) -> &mut U, U: ?Sized>(
        self,
        f: F,
    ) -> ExclusiveGuard<'a, L, U, Mapped> {
        let value = f(unsafe { &mut *self.value });

        unsafe { ExclusiveGuard::from_raw_parts(self.raw, value) }
    }

    pub fn try_map<F: FnOnce(&mut T) -> Result<&mut U, E>, E, U: ?Sized>(
        self,
        f: F,
    ) -> Result<ExclusiveGuard<'a, L, U, Mapped>, TryMapError<E, Self>> {
        match f(unsafe { &mut *self.value }) {
            Ok(value) => Ok(unsafe { ExclusiveGuard::from_raw_parts(self.raw, value) }),
            Err(e) => Err(TryMapError(e, self)),
        }
    }

    pub fn into_raw_parts(self) -> (RawExclusiveGuard<'a, L>, *mut T) {
        (self.raw, self.value)
    }
}

impl<'a, L: SplittableExclusiveLock + RawLockInfo, T: ?Sized, St> ExclusiveGuard<'a, L, T, St> {
    pub fn split_map<F, U: ?Sized, V: ?Sized>(
        self,
        f: F,
    ) -> (ExclusiveGuard<'a, L, U, Mapped>, ExclusiveGuard<'a, L, V, Mapped>)
    where
        F: FnOnce(&mut T) -> (&mut U, &mut V),
    {
        let (u, v) = f(unsafe { &mut *self.value });

        let u_lock = self.raw.clone();
        let v_lock = self.raw;

        (
            unsafe { ExclusiveGuard::from_raw_parts(u_lock, u) },
            unsafe { ExclusiveGuard::from_raw_parts(v_lock, v) },
        )
    }

    #[allow(clippy::type_complexity)]
    pub fn try_split_map<
        F: FnOnce(&mut T) -> Result<(&mut U, &mut V), E>,
        E,
        U: ?Sized,
        V: ?Sized,
    >(
        self,
        f: F,
    ) -> Result<(ExclusiveGuard<'a, L, U, Mapped>, ExclusiveGuard<'a, L, V, Mapped>), TryMapError<E, Self>>
    {
        match f(unsafe { &mut *self.value }) {
            Ok((u, v)) => {
                let u_lock = self.raw.clone();
                let v_lock = self.raw;

                Ok((
                    unsafe { ExclusiveGuard::from_raw_parts(u_lock, u) },
                    unsafe { ExclusiveGuard::from_raw_parts(v_lock, v) },
                ))
            }
            Err(e) => Err(TryMapError(e, self)),
        }
    }

    pub fn bump(g: &mut Self) {
        g.raw.bump()
    }

    pub fn unlocked<R>(g: &mut Self, f: impl FnOnce() -> R) -> R {
        g.raw.unlocked(f)
    }
}

impl<L: RawExclusiveLockFair + RawLockInfo, T: ?Sized, St> ExclusiveGuard<'_, L, T, St> {
    pub fn unlock_fair(g: Self) {
        g.raw.unlock_fair();
    }
    
    pub fn bump_fair(g: &mut Self) {
        g.raw.bump_fair();
    }
    
    pub fn unlocked_fair<R>(g: &mut Self, f: impl FnOnce() -> R) -> R {
        g.raw.unlocked_fair(f)
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
