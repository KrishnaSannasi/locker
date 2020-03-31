use super::RawRwLock;
use crate::{exclusive_lock::RawExclusiveGuard, share_lock::RawShareGuard, WakerSet};
use locker::rwlock::raw;

#[repr(C)]
pub struct RwLock<L, W> {
    raw: raw::RwLock<L>,
    waker_set: W,
}

impl<L: RawRwLock + locker::Init, W: WakerSet + locker::Init> Default for RwLock<L, W> {
    #[inline]
    fn default() -> Self {
        locker::Init::INIT
    }
}

impl<L, W> RwLock<L, W> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw_parts(raw: raw::RwLock<L>, waker_set: W) -> Self {
        Self { raw, waker_set }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::RwLock<L>, W) {
        (self.raw, self.waker_set)
    }

    #[inline]
    pub const fn raw_rwlock(&self) -> &raw::RwLock<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_rwlock_mut(&mut self) -> &mut raw::RwLock<L> {
                &mut self.lock
            }
        } else {
            #[inline]
            pub unsafe fn raw_rwlock_mut(&mut self) -> &mut raw::RwLock<L> {
                &mut self.raw
            }
        }
    }
}

impl<L: RawRwLock + locker::Init, W: WakerSet + locker::Init> locker::Init for RwLock<L, W> {
    const INIT: Self = unsafe { Self::from_raw_parts(locker::Init::INIT, locker::Init::INIT) };
}

impl<L: RawRwLock + locker::Init, W: WakerSet + locker::Init> RwLock<L, W> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new() -> Self {
                locker::Init::INIT
            }
        } else {
            #[inline]
            pub fn new() -> Self {
                locker::Init::INIT
            }
        }
    }
}

impl<L: RawRwLock, W: WakerSet> RwLock<L, W>
where
    L::ExclusiveGuardTraits: locker::marker::Inhabitted,
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn write(&self) -> RawExclusiveGuard<'_, L, W> {
        use crate::slab::Index;

        pub struct LockFuture<'a, L, W, I>(&'a RwLock<L, W>, Option<I>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawRwLock, W: WakerSet> std::future::Future for LockFuture<'a, L, W, W::Index>
        where
            L::ExclusiveGuardTraits: locker::marker::Inhabitted,
            L::ShareGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawExclusiveGuard<'a, L, W>;

            fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
                let Self(rwlock, opt_key) = Pin::into_inner(self);

                if let Some(key) = opt_key.take() {
                    rwlock.waker_set.remove(key);
                }

                let key = match rwlock.try_write() {
                    Some(gaurd) => return Poll::Ready(gaurd),
                    None => rwlock.waker_set.insert(ctx),
                };

                match rwlock.try_write() {
                    Some(gaurd) => {
                        rwlock.waker_set.remove(key);
                        Poll::Ready(gaurd)
                    }
                    None => {
                        *opt_key = Some(key);
                        Poll::Pending
                    }
                }
            }
        }

        LockFuture(self, None).await
    }

    #[inline]
    pub fn try_write(&self) -> Option<RawExclusiveGuard<'_, L, W>> {
        Some(RawExclusiveGuard::from_raw_parts(
            self.raw.try_write()?,
            &self.waker_set,
        ))
    }

    #[inline]
    pub async fn read(&self) -> RawShareGuard<'_, L, W> {
        pub struct LockFuture<'a, L, W, I>(&'a RwLock<L, W>, Option<I>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawRwLock, W: WakerSet> std::future::Future for LockFuture<'a, L, W, W::Index>
        where
            L::ExclusiveGuardTraits: locker::marker::Inhabitted,
            L::ShareGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawShareGuard<'a, L, W>;

            fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
                let Self(rwlock, opt_key) = Pin::into_inner(self);

                if let Some(key) = opt_key.take() {
                    rwlock.waker_set.remove(key);
                }

                let key = match rwlock.try_read() {
                    Some(gaurd) => return Poll::Ready(gaurd),
                    None => rwlock.waker_set.insert(ctx),
                };

                match rwlock.try_read() {
                    Some(gaurd) => {
                        rwlock.waker_set.remove(key);
                        Poll::Ready(gaurd)
                    }
                    None => {
                        *opt_key = Some(key);
                        Poll::Pending
                    }
                }
            }
        }

        LockFuture(self, None).await
    }

    #[inline]
    pub fn try_read(&self) -> Option<RawShareGuard<'_, L, W>> {
        Some(RawShareGuard::from_raw_parts(
            self.raw.try_read()?,
            &self.waker_set,
        ))
    }
}
