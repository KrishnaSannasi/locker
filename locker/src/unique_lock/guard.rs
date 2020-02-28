use super::{RawUniqueGuard, RawUniqueLock, RawUniqueLockFair, SplittableUniqueLock};
use crate::RawLockInfo;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub enum Pure {}
pub enum Mapped {}

pub struct TryMapError<E, G>(pub E, pub G);

pub type MappedUniqueGuard<'a, T> = UniqueGuard<'a, T, Mapped>;
pub struct UniqueGuard<'a, L: RawUniqueLock + RawLockInfo, T: ?Sized, St = Pure> {
    raw: RawUniqueGuard<'a, L>,
    value: *mut T,
    _repr: PhantomData<(&'a mut T, St)>,
}

unsafe impl<'a, L: RawUniqueLock + RawLockInfo, T: ?Sized + Send, St> Send
    for UniqueGuard<'a, L, T, St>
where
    RawUniqueGuard<'a, L>: Send,
{
}
unsafe impl<'a, L: RawUniqueLock + RawLockInfo, T: ?Sized + Sync, St> Sync
    for UniqueGuard<'a, L, T, St>
where
    RawUniqueGuard<'a, L>: Sync,
{
}

impl<'a, L: RawUniqueLock + RawLockInfo, T: ?Sized, St> UniqueGuard<'a, L, T, St> {
    pub fn new(raw: RawUniqueGuard<'a, L>, value: &'a mut T) -> Self {
        Self {
            raw,
            value,
            _repr: PhantomData,
        }
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn raw(&self) -> &RawUniqueGuard<L> {
        &self.raw
    }

    pub fn map<F: FnOnce(&mut T) -> &mut U, U: ?Sized>(
        self,
        f: F,
    ) -> UniqueGuard<'a, L, U, Mapped> {
        let value = f(unsafe { &mut *self.value });

        UniqueGuard::new(self.raw, value)
    }

    pub fn try_map<F: FnOnce(&mut T) -> Result<&mut U, E>, E, U: ?Sized>(
        self,
        f: F,
    ) -> Result<UniqueGuard<'a, L, U, Mapped>, TryMapError<E, Self>> {
        match f(unsafe { &mut *self.value }) {
            Ok(value) => Ok(UniqueGuard::new(self.raw, value)),
            Err(e) => Err(TryMapError(e, self)),
        }
    }

    pub fn into_raw_parts(self) -> (RawUniqueGuard<'a, L>, *mut T) {
        (self.raw, self.value)
    }
}

impl<'a, L: SplittableUniqueLock + RawLockInfo, T: ?Sized, St> UniqueGuard<'a, L, T, St> {
    pub fn split_map<F, U: ?Sized, V: ?Sized>(
        self,
        f: F,
    ) -> (UniqueGuard<'a, L, U, Mapped>, UniqueGuard<'a, L, V, Mapped>)
    where
        F: FnOnce(&mut T) -> (&mut U, &mut V),
    {
        let (u, v) = f(unsafe { &mut *self.value });

        let u_lock = self.raw.clone();
        let v_lock = self.raw;

        (UniqueGuard::new(u_lock, u), UniqueGuard::new(v_lock, v))
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
    ) -> Result<(UniqueGuard<'a, L, U, Mapped>, UniqueGuard<'a, L, V, Mapped>), TryMapError<E, Self>>
    {
        match f(unsafe { &mut *self.value }) {
            Ok((u, v)) => {
                let u_lock = self.raw.clone();
                let v_lock = self.raw;

                Ok((UniqueGuard::new(u_lock, u), UniqueGuard::new(v_lock, v)))
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

impl<L: RawUniqueLockFair + RawLockInfo, T: ?Sized> UniqueGuard<'_, L, T> {
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

impl<L: RawUniqueLock + RawLockInfo, T: ?Sized> Deref for UniqueGuard<'_, L, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

impl<L: RawUniqueLock + RawLockInfo, T: ?Sized> DerefMut for UniqueGuard<'_, L, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value }
    }
}
