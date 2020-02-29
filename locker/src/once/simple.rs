use crate::mutex::tagged::RawLock as Tagged;
use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use std::sync::atomic::Ordering;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type Once = crate::once::Once<RawLock>;
pub type OnceCell<T> = crate::once::OnceCell<RawLock, T>;
pub type Lazy<T, F> = crate::once::Lazy<RawLock, T, F, crate::once::Panic>;
pub type RertyLazy<T, F> = crate::once::Lazy<RawLock, T, F, crate::once::Retry>;

pub struct RawLock {
    inner: Tagged,
}

unsafe impl crate::once::Finish for RawLock {
    #[inline]
    fn is_done(&self) -> bool {
        self.inner.tag(Ordering::Relaxed) & Self::DONE_BIT != 0
    }

    #[inline]
    fn mark_done(&self) {
        self.inner.or_tag(Self::DONE_BIT, Ordering::Relaxed);
    }

    #[inline]
    fn get_and_mark_poisoned(&self) -> bool {
        let state = self.inner.or_tag(Self::POISON_BIT, Ordering::Relaxed);

        state & Self::POISON_BIT != 0
    }
}

impl RawLock {
    const DONE_BIT: u8 = 0b01;
    const POISON_BIT: u8 = 0b10;

    pub const fn new() -> Self {
        Self {
            inner: Tagged::new(),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }

    pub const fn once() -> Once {
        unsafe { Once::from_raw(Self::new()) }
    }

    pub const fn once_cell<T>() -> OnceCell<T> {
        unsafe { OnceCell::from_once(Self::once()) }
    }

    pub const fn lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
    }

    pub const fn retry_lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
    }
}

unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();
    type ExclusiveGuardTraits = <Tagged as crate::RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <Tagged as crate::RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawExclusiveLock for RawLock {
    #[inline]
    fn exc_lock(&self) {
        self.inner.exc_lock();
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        self.inner.exc_try_lock()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.inner.exc_unlock_fair();
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        self.inner.exc_bump_fair();
    }
}

unsafe impl RawExclusiveLockFair for RawLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        self.inner.exc_unlock_fair();
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        self.inner.exc_bump_fair();
    }
}
