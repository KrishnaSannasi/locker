use super::raw::RawExclusiveGuard;
use locker::exclusive_lock::{
    RawExclusiveLock, RawExclusiveLockDowngrade, SplittableExclusiveLock,
};
use locker::RawLockInfo;
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
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            pub const unsafe fn from_raw_parts(raw: RawExclusiveGuard<'a, L>, value: *mut T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            pub const fn raw(&self) -> &RawExclusiveGuard<'a, L> {
                &self.raw
            }

            pub const unsafe fn raw_mut(&mut self) -> &mut RawExclusiveGuard<'a, L> {
                &mut self.raw
            }
        } else {
            pub unsafe fn from_raw_parts(raw: RawExclusiveGuard<'a, L>, value: *mut T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            pub fn raw(&self) -> &RawExclusiveGuard<'a, L> {
                &self.raw
            }

            pub unsafe fn raw_mut(&mut self) -> &mut RawExclusiveGuard<'a, L> {
                &mut self.raw
            }
        }
    }

    pub fn into_raw_parts(self) -> (RawExclusiveGuard<'a, L>, *mut T) {
        (self.raw, self.value)
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
}

impl<'a, L: SplittableExclusiveLock + RawExclusiveLock + RawLockInfo, T: ?Sized, St>
    ExclusiveGuard<'a, L, T, St>
{
    pub fn split_map<U: ?Sized, V: ?Sized>(
        self,
        f: impl FnOnce(&mut T) -> (&mut U, &mut V),
    ) -> (
        ExclusiveGuard<'a, L, U, Mapped>,
        ExclusiveGuard<'a, L, V, Mapped>,
    ) {
        let (u, v) = f(unsafe { &mut *self.value });

        let u_lock = self.raw.clone();
        let v_lock = self.raw;

        (
            unsafe { ExclusiveGuard::from_raw_parts(u_lock, u) },
            unsafe { ExclusiveGuard::from_raw_parts(v_lock, v) },
        )
    }

    #[allow(clippy::type_complexity)]
    pub fn try_split_map<E, U: ?Sized, V: ?Sized>(
        self,
        f: impl FnOnce(&mut T) -> Result<(&mut U, &mut V), E>,
    ) -> Result<
        (
            ExclusiveGuard<'a, L, U, Mapped>,
            ExclusiveGuard<'a, L, V, Mapped>,
        ),
        TryMapError<E, Self>,
    > {
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
}

impl<'a, L: RawExclusiveLockDowngrade + RawLockInfo, T: ?Sized> ExclusiveGuard<'a, L, T>
where
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
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
