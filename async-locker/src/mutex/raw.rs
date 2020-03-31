use crate::{exclusive_lock::raw::RawExclusiveGuard, WakerSet};

use locker::mutex::{raw, RawMutex};

pub struct Mutex<L, W> {
    raw: raw::Mutex<L>,
    waker_set: W,
}

impl<L: RawMutex + locker::Init, W: WakerSet + locker::Init> Default for Mutex<L, W> {
    #[inline]
    fn default() -> Self {
        locker::Init::INIT
    }
}

impl<L, W> Mutex<L, W> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw_parts(raw: raw::Mutex<L>, waker_set: W) -> Self {
        Self { raw, waker_set }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::Mutex<L>, W) {
        (self.raw, self.waker_set)
    }

    #[inline]
    pub const fn raw_mutex(&self) -> &raw::Mutex<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_mutex_mut(&mut self) -> &mut raw::Mutex<L> {
                &mut self.lock
            }
        } else {
            #[inline]
            pub unsafe fn raw_mutex_mut(&mut self) -> &mut raw::Mutex<L> {
                &mut self.raw
            }
        }
    }
}

impl<L: RawMutex + locker::Init, W: WakerSet + locker::Init> locker::Init for Mutex<L, W> {
    const INIT: Self = unsafe { Self::from_raw_parts(locker::Init::INIT, locker::Init::INIT) };
}

impl<L: RawMutex + locker::Init, W: WakerSet + locker::Init> Mutex<L, W> {
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

impl<L: RawMutex, W: WakerSet> Mutex<L, W>
where
    L::ExclusiveGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn lock(&self) -> RawExclusiveGuard<'_, L, W> {
        pub struct LockFuture<'a, L, W, I>(&'a Mutex<L, W>, Option<I>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawMutex, W: WakerSet> std::future::Future for LockFuture<'a, L, W, W::Index>
        where
            L::ExclusiveGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawExclusiveGuard<'a, L, W>;

            fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
                let Self(mutex, opt_key) = Pin::into_inner(self);

                if let Some(key) = opt_key.take() {
                    mutex.waker_set.remove(key);
                }

                let key = match mutex.try_lock() {
                    Some(gaurd) => return Poll::Ready(gaurd),
                    None => mutex.waker_set.insert(ctx),
                };

                match mutex.try_lock() {
                    Some(gaurd) => {
                        mutex.waker_set.remove(key);
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
    pub fn try_lock(&self) -> Option<RawExclusiveGuard<'_, L, W>> {
        let guard = self.raw.try_lock()?;

        Some(RawExclusiveGuard::from_raw_parts(guard, &self.waker_set))
    }
}
