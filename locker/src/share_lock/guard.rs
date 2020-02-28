use super::{RawShareGuard, RawShareLock, RawShareLockFair};
use crate::RawLockInfo;
use std::marker::PhantomData;
use std::ops::Deref;

pub enum Pure {}
pub enum Mapped {}

pub struct TryMapError<E, G>(pub E, pub G);

pub type MappedShareGuard<'a, L, T> = ShareGuard<'a, L, T, Mapped>;
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

impl<'a, L: RawShareLock + RawLockInfo, T: ?Sized, St> ShareGuard<'a, L, T, St> {
    pub unsafe fn from_raw_parts(raw: RawShareGuard<'a, L>, value: &'a T) -> Self {
        Self {
            raw,
            value,
            _repr: PhantomData,
        }
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn raw(&self) -> &RawShareGuard<'a, L> {
        &self.raw
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn raw_mut(&mut self) -> &mut RawShareGuard<'a, L> {
        &mut self.raw
    }

    pub fn map<F: FnOnce(&T) -> &U, U: ?Sized>(self, f: F) -> ShareGuard<'a, L, U, Mapped> {
        let value = f(unsafe { &*self.value });

        unsafe { ShareGuard::from_raw_parts(self.raw, value) }
    }

    pub fn try_map<F: FnOnce(&T) -> Result<&U, E>, E, U: ?Sized>(
        self,
        f: F,
    ) -> Result<ShareGuard<'a, L, U, Mapped>, TryMapError<E, Self>> {
        match f(unsafe { &*self.value }) {
            Ok(value) => Ok(unsafe { ShareGuard::from_raw_parts(self.raw, value) }),
            Err(e) => Err(TryMapError(e, self)),
        }
    }

    pub fn into_raw_parts(self) -> (RawShareGuard<'a, L>, *const T) {
        (self.raw, self.value)
    }

    pub fn split_map<F, U: ?Sized, V: ?Sized>(
        self,
        f: F,
    ) -> (ShareGuard<'a, L, U, Mapped>, ShareGuard<'a, L, V, Mapped>)
    where
        F: FnOnce(&T) -> (&U, &V),
    {
        let (u, v) = f(unsafe { &*self.value });

        let u_lock = self.raw.clone();
        let v_lock = self.raw;

        (
            unsafe { ShareGuard::from_raw_parts(u_lock, u) },
            unsafe { ShareGuard::from_raw_parts(v_lock, v) },
        )
    }

    #[allow(clippy::type_complexity)]
    pub fn try_split_map<F: FnOnce(&T) -> Result<(&U, &V), E>, E, U: ?Sized, V: ?Sized>(
        self,
        f: F,
    ) -> Result<(ShareGuard<'a, L, U, Mapped>, ShareGuard<'a, L, V, Mapped>), TryMapError<E, Self>>
    {
        match f(unsafe { &*self.value }) {
            Ok((u, v)) => {
                let u_lock = self.raw.clone();
                let v_lock = self.raw;

                Ok((
                    unsafe { ShareGuard::from_raw_parts(u_lock, u) },
                    unsafe { ShareGuard::from_raw_parts(v_lock, v) },
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

impl<L: RawShareLockFair + RawLockInfo, T: ?Sized, St> ShareGuard<'_, L, T, St> {
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
