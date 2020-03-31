use super::RawReentrantMutex;
use crate::{share_lock::RawShareGuard, WakerSet};
use locker::remutex::raw;

#[repr(C)]
pub struct ReentrantMutex<L, W> {
    raw: raw::ReentrantMutex<L>,
    waker_set: W,
}

impl<L: RawReentrantMutex + locker::Init, W: WakerSet + locker::Init> Default
    for ReentrantMutex<L, W>
{
    #[inline]
    fn default() -> Self {
        locker::Init::INIT
    }
}

impl<L, W> ReentrantMutex<L, W> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw_parts(raw: raw::ReentrantMutex<L>, waker_set: W) -> Self {
        Self { raw, waker_set }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::ReentrantMutex<L>, W) {
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

impl<L: RawReentrantMutex + locker::Init, W: WakerSet + locker::Init> locker::Init
    for ReentrantMutex<L, W>
{
    const INIT: Self = unsafe { Self::from_raw_parts(locker::Init::INIT, locker::Init::INIT) };
}

impl<L: RawReentrantMutex + locker::Init, W: WakerSet + locker::Init> ReentrantMutex<L, W> {
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

impl<L: RawReentrantMutex, W: WakerSet> ReentrantMutex<L, W>
where
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn lock(&self) -> RawShareGuard<'_, L, W> {
        pub struct LockFuture<'a, L, W, I>(&'a ReentrantMutex<L, W>, Option<I>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawReentrantMutex, W: WakerSet> std::future::Future for LockFuture<'a, L, W, W::Index>
        where
            L::ShareGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawShareGuard<'a, L, W>;

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
    pub fn try_lock(&self) -> Option<RawShareGuard<'_, L, W>> {
        Some(RawShareGuard::from_raw_parts(
            self.raw.try_lock()?,
            &self.waker_set,
        ))
    }
}
