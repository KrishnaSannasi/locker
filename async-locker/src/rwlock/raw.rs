use super::RawRwLock;
use crate::{exclusive_lock::RawExclusiveGuard, share_lock::RawShareGuard, waker_set::WakerSet};
use locker::rwlock::raw;

#[repr(C)]
pub struct RwLock<L> {
    raw: raw::RwLock<L>,
    waker_set: WakerSet,
}

impl<L: RawRwLock> Default for RwLock<L> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<L> RwLock<L> {
    /// # Safety
    ///
    /// You must pass `RawUniueLock::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw_rwlock(raw: raw::RwLock<L>) -> Self {
        Self {
            raw,
            waker_set: WakerSet::new(),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::RwLock<L>, WakerSet) {
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

impl<L: RawRwLock> RwLock<L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new() -> Self {
                unsafe { Self::from_raw(L::INIT) }
            }
        } else {
            #[inline]
            pub fn new() -> Self {
                unsafe { Self::from_raw_rwlock(raw::RwLock::new()) }
            }
        }
    }
}

impl<L: RawRwLock> RwLock<L>
where
    L::ExclusiveGuardTraits: locker::marker::Inhabitted,
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn write(&self) -> RawExclusiveGuard<'_, L> {
        use crate::slab::Index;

        pub struct LockFuture<'a, L>(&'a RwLock<L>, Option<Index>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawRwLock> std::future::Future for LockFuture<'a, L>
        where
            L::ExclusiveGuardTraits: locker::marker::Inhabitted,
            L::ShareGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawExclusiveGuard<'a, L>;

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
    pub fn try_write(&self) -> Option<RawExclusiveGuard<'_, L>> {
        Some(RawExclusiveGuard::from_raw_parts(
            self.raw.try_write()?,
            &self.waker_set,
        ))
    }

    #[inline]
    pub async fn read(&self) -> RawShareGuard<'_, L> {
        use crate::slab::Index;

        pub struct LockFuture<'a, L>(&'a RwLock<L>, Option<Index>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawRwLock> std::future::Future for LockFuture<'a, L>
        where
            L::ExclusiveGuardTraits: locker::marker::Inhabitted,
            L::ShareGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawShareGuard<'a, L>;

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
    pub fn try_read(&self) -> Option<RawShareGuard<'_, L>> {
        Some(RawShareGuard::from_raw_parts(
            self.raw.try_read()?,
            &self.waker_set,
        ))
    }
}
