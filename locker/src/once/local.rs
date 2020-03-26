use crate::exclusive_lock::RawExclusiveLock;
use crate::mutex::local_tagged::LocalTaggedLock as Tagged;

pub type RawMutex = crate::mutex::raw::Mutex<RawLock>;
pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type Once = crate::once::Once<RawLock>;
pub type OnceCell<T> = crate::once::OnceCell<RawLock, T>;
pub type Lazy<T, F = fn() -> T> = crate::once::Lazy<RawLock, T, F, crate::once::Panic>;
pub type RertyLazy<T, F = fn() -> T> = crate::once::Lazy<RawLock, T, F, crate::once::Retry>;
pub type RacyLazy<T, F = fn() -> T> = crate::once::RacyLazy<RawLock, T, F>;

pub struct RawLock {
    inner: Tagged,
}

impl RawLock {
    const DONE_BIT: u8 = 0b01;
    const POISON_BIT: u8 = 0b10;

    pub const fn new() -> Self {
        Self {
            inner: Tagged::new(),
        }
    }

    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    pub const fn once() -> Once {
        unsafe { Once::from_raw(Self::new()) }
    }

    pub const fn once_cell<T>() -> OnceCell<T> {
        unsafe {
            OnceCell {
                once: Once::from_raw(Self::new()),
                value: super::UnsafeCell::new(super::MaybeUninit::uninit()),
            }
        }
    }

    pub const fn lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
    }

    pub const fn retry_lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
    }

    pub const fn racy_lazy<T, F>(func: F) -> RacyLazy<T, F> {
        RacyLazy {
            once: Self::once_cell(),
            func,
        }
    }
}

unsafe impl crate::once::Finish for RawLock {
    #[inline]
    fn is_done(&self) -> bool {
        self.inner.tag() & Self::DONE_BIT != 0
    }

    #[inline]
    fn mark_done(&self) {
        self.inner.or_tag(Self::DONE_BIT);
    }

    #[inline]
    fn is_poisoned(&self) -> bool {
        (self.inner.tag() & Self::POISON_BIT) != 0
    }

    #[inline]
    fn mark_poisoned(&self) {
        self.inner.or_tag(Self::POISON_BIT);
    }
}

impl crate::Init for RawLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::RawLockInfo for RawLock {
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl RawExclusiveLock for RawLock {
    #[inline]
    fn exc_lock(&self) {
        self.inner.exc_lock()
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        self.inner.exc_try_lock()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.inner.exc_unlock()
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        self.inner.exc_bump()
    }
}
