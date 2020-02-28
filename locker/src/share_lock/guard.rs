use super::{RawShareGuard, RawShareLock};
use crate::RawLockInfo;
use std::marker::PhantomData;
use std::ops::Deref;

pub enum Pure {}
pub enum Mapped {}

pub struct TryMapError<E, G>(pub E, pub G);

pub type MappedShareGuard<'a, T> = ShareGuard<'a, T, Mapped>;
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
    pub fn new(raw: RawShareGuard<'a, L>, value: &'a T) -> Self {
        Self {
            raw,
            value,
            _repr: PhantomData,
        }
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn raw(&self) -> &RawShareGuard<L> {
        &self.raw
    }

    pub fn map<F: FnOnce(&T) -> &U, U: ?Sized>(self, f: F) -> ShareGuard<'a, L, U, Mapped> {
        let value = f(unsafe { &*self.value });

        ShareGuard::new(self.raw, value)
    }

    pub fn try_map<F: FnOnce(&T) -> Result<&U, E>, E, U: ?Sized>(
        self,
        f: F,
    ) -> Result<ShareGuard<'a, L, U, Mapped>, TryMapError<E, Self>> {
        match f(unsafe { &*self.value }) {
            Ok(value) => Ok(ShareGuard::new(self.raw, value)),
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

        (ShareGuard::new(u_lock, u), ShareGuard::new(v_lock, v))
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

                Ok((ShareGuard::new(u_lock, u), ShareGuard::new(v_lock, v)))
            }
            Err(e) => Err(TryMapError(e, self)),
        }
    }
}

impl<L: RawShareLock + RawLockInfo, T: ?Sized> Deref for ShareGuard<'_, L, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

impl<L: RawShareLock + RawLockInfo, T: ?Sized> Clone for ShareGuard<'_, L, T> {
    fn clone(&self) -> Self {
        unsafe { Self::new(self.raw.clone(), &*self.value) }
    }
}
