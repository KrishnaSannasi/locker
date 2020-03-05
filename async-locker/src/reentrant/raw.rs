use super::RawReentrantMutex;
use crate::{share_lock::RawShareGuard, waker_set::WakerSet};
use locker::reentrant::raw;

#[repr(C)]
pub struct ReentrantMutex<L> {
    raw: raw::ReentrantMutex<L>,
    waker_set: WakerSet,
}

impl<L: RawReentrantMutex> Default for ReentrantMutex<L> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<L> ReentrantMutex<L> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw(raw: raw::ReentrantMutex<L>) -> Self {
        Self {
            raw,
            waker_set: WakerSet::new(),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::ReentrantMutex<L>, WakerSet) {
        (self.raw, self.waker_set)
    }

    #[inline]
    pub const fn inner(&self) -> &raw::ReentrantMutex<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn inner_mut(&mut self) -> &mut raw::ReentrantMutex<L> {
                &mut self.raw
            }
        } else {
            #[inline]
            pub unsafe fn inner_mut(&mut self) -> &mut raw::ReentrantMutex<L> {
                &mut self.raw
            }
        }
    }
}

impl<L: RawReentrantMutex> ReentrantMutex<L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new() -> Self {
                unsafe { Self::from_raw_parts(raw::ReentrantMutex::new()) }
            }
        } else {
            #[inline]
            pub fn new() -> Self {
                unsafe { Self::from_raw(raw::ReentrantMutex::new()) }
            }
        }
    }
}

impl<L: RawReentrantMutex> ReentrantMutex<L>
where
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn lock(&self) -> RawShareGuard<'_, L> {
        use crate::slab::Index;

        pub struct LockFuture<'a, L>(&'a ReentrantMutex<L>, Option<Index>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawReentrantMutex> std::future::Future for LockFuture<'a, L>
        where
            L::ShareGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawShareGuard<'a, L>;

            fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
                let Self(rwlock, opt_key) = Pin::into_inner(self);

                if let Some(key) = opt_key.take() {
                    rwlock.waker_set.remove(key);
                }

                let key = match rwlock.try_lock() {
                    Some(gaurd) => return Poll::Ready(gaurd),
                    None => rwlock.waker_set.insert(ctx),
                };

                match rwlock.try_lock() {
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
    pub fn try_lock(&self) -> Option<RawShareGuard<'_, L>> {
        Some(RawShareGuard::from_raw_parts(
            self.raw.try_lock()?,
            &self.waker_set,
        ))
    }
}
