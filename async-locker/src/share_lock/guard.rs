use super::raw::RawShareGuard;
use crate::WakerSet;
use locker::share_lock::RawShareLock;
use locker::RawLockInfo;
use std::marker::PhantomData;
use std::ops::Deref;

pub enum Pure {}
pub enum Mapped {}

pub struct TryMapError<E, G>(pub E, pub G);

pub type MappedShareGuard<'a, L, W, T> = ShareGuard<'a, L, W, T, Mapped>;
pub struct ShareGuard<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized, T: ?Sized, St = Pure>
{
    raw: RawShareGuard<'a, L, W>,
    value: *const T,
    _repr: PhantomData<(&'a T, St)>,
}

unsafe impl<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized, T: ?Sized + Send, St> Send
    for ShareGuard<'a, L, W, T, St>
where
    RawShareGuard<'a, L, W>: Send,
{
}
unsafe impl<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized, T: ?Sized + Sync, St> Sync
    for ShareGuard<'a, L, W, T, St>
where
    RawShareGuard<'a, L, W>: Sync,
{
}

impl<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized, T: ?Sized, St>
    ShareGuard<'a, L, W, T, St>
{
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            pub const unsafe fn from_raw_parts(raw: RawShareGuard<'a, L>, value: *const T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            pub const fn raw(&self) -> &RawShareGuard<'a, L> {
                &self.raw
            }

            pub const unsafe fn raw_mut(&mut self) -> &mut RawShareGuard<'a, L> {
                &mut self.raw
            }
        } else {
            pub unsafe fn from_raw_parts(raw: RawShareGuard<'a, L, W>, value: *const T) -> Self {
                Self {
                    raw,
                    value,
                    _repr: PhantomData,
                }
            }

            pub fn raw(&self) -> &RawShareGuard<'a, L, W> {
                &self.raw
            }

            pub unsafe fn raw_mut(&mut self) -> &mut RawShareGuard<'a, L, W> {
                &mut self.raw
            }
        }
    }

    pub fn into_raw_parts(self) -> (RawShareGuard<'a, L, W>, *const T) {
        (self.raw, self.value)
    }

    pub async fn bump(g: &mut Self) {
        g.raw.bump().await
    }

    pub fn map<F: FnOnce(&T) -> &U, U: ?Sized>(self, f: F) -> ShareGuard<'a, L, W, U, Mapped> {
        let value = f(unsafe { &*self.value });

        unsafe { ShareGuard::from_raw_parts(self.raw, value) }
    }

    pub fn try_map<F: FnOnce(&T) -> Result<&U, E>, E, U: ?Sized>(
        self,
        f: F,
    ) -> Result<ShareGuard<'a, L, W, U, Mapped>, TryMapError<E, Self>> {
        match f(unsafe { &*self.value }) {
            Ok(value) => Ok(unsafe { ShareGuard::from_raw_parts(self.raw, value) }),
            Err(e) => Err(TryMapError(e, self)),
        }
    }
}

impl<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized, T: ?Sized, St>
    ShareGuard<'a, L, W, T, St>
{
    pub fn split_map<U: ?Sized, V: ?Sized>(
        self,
        f: impl FnOnce(&T) -> (&U, &V),
    ) -> (
        ShareGuard<'a, L, W, U, Mapped>,
        ShareGuard<'a, L, W, V, Mapped>,
    ) {
        let (u, v) = f(unsafe { &*self.value });

        let u_lock = self.raw.clone();
        let v_lock = self.raw;

        (unsafe { ShareGuard::from_raw_parts(u_lock, u) }, unsafe {
            ShareGuard::from_raw_parts(v_lock, v)
        })
    }

    #[allow(clippy::type_complexity)]
    pub fn try_split_map<E, U: ?Sized, V: ?Sized>(
        self,
        f: impl FnOnce(&T) -> Result<(&U, &V), E>,
    ) -> Result<
        (
            ShareGuard<'a, L, W, U, Mapped>,
            ShareGuard<'a, L, W, V, Mapped>,
        ),
        TryMapError<E, Self>,
    > {
        match f(unsafe { &*self.value }) {
            Ok((u, v)) => {
                let u_lock = self.raw.clone();
                let v_lock = self.raw;

                Ok((unsafe { ShareGuard::from_raw_parts(u_lock, u) }, unsafe {
                    ShareGuard::from_raw_parts(v_lock, v)
                }))
            }
            Err(e) => Err(TryMapError(e, self)),
        }
    }
}

impl<L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized, T: ?Sized, St> Deref
    for ShareGuard<'_, L, W, T, St>
{
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}
