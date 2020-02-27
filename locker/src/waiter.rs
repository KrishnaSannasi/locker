use parking_lot_core::{self, SpinWait, DEFAULT_PARK_TOKEN, DEFAULT_UNPARK_TOKEN};

use std::mem::MaybeUninit;

use std::time::{Duration, Instant};

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Waiter<T: ?Sized = MaybeUninit<u8>> {
    _private: (),
    pub inner: T,
}

pub struct Timeout;

impl Waiter {
    pub const fn new() -> Self {
        unsafe { Self::with_value(MaybeUninit::uninit()) }
    }
}

pub trait SpinWaitOutput {
    fn sleep() -> Self;

    fn is_finished(&self) -> bool;
}

impl SpinWaitOutput for bool {
    fn sleep() -> Self {
        false
    }

    fn is_finished(&self) -> bool {
        *self
    }
}

impl<T> SpinWaitOutput for Option<T> {
    fn sleep() -> Self {
        None
    }

    fn is_finished(&self) -> bool {
        self.is_some()
    }
}

pub fn spin_wait<T: ?Sized, R: SpinWaitOutput, F: FnMut(&T) -> R>(mut f: F) -> impl FnMut(&T) -> R {
    move |value| {
        let mut spin = SpinWait::new();
        while spin.spin() {
            let value = f(value);

            if value.is_finished() {
                return value;
            }
        }

        R::sleep()
    }
}

#[cold]
#[inline(never)]
unsafe fn wait(key: usize, timeout: Option<Instant>) -> bool {
    let validate = || true;
    let before_sleep = || {};
    let timed_out = |_key, _was_last| {};

    parking_lot_core::park(
        key,
        validate,
        before_sleep,
        timed_out,
        DEFAULT_PARK_TOKEN,
        timeout,
    )
    .is_unparked()
}

impl<T> Waiter<T> {
    /// # Safety
    ///
    /// The `Waiter` must not share it's address with anything that calls into `parking_lot_core`
    #[inline(always)]
    #[allow(clippy::unnecessary_operation)]
    pub const unsafe fn with_value(value: T) -> Self {
        [()][(std::mem::size_of::<T>() == 0) as usize];

        Self {
            inner: value,
            _private: (),
        }
    }
}

impl<T: ?Sized> Waiter<T> {
    fn key(&self) -> usize {
        self as *const Self as *const () as usize
    }

    #[inline(always)]
    fn sleep(&self, timeout: Option<Instant>) -> bool {
        unsafe { wait(self.key(), timeout) }
    }

    #[inline(always)]
    fn sleep_while(&self, timeout: Option<Instant>, func: &mut dyn FnMut(&T) -> bool) -> bool {
        while func(&self.inner) {
            if !self.sleep(timeout) {
                return false;
            }
        }

        // if the function returned true, return true
        true
    }

    #[inline(always)]
    fn sleep_with<R>(
        &self,
        timeout: Option<Instant>,
        func: &mut dyn FnMut(&T) -> Option<R>,
    ) -> Result<R, Timeout> {
        let mut value = None;

        self.sleep_while(timeout, &mut |inner| {
            value = func(inner);
            value.is_some()
        });

        value.ok_or(Timeout)
    }

    #[inline]
    pub fn notify_one(&self) -> bool {
        let key = self.key();
        let callback = |_result| DEFAULT_UNPARK_TOKEN;

        unsafe { parking_lot_core::unpark_one(key, callback).unparked_threads > 0 }
    }

    #[inline]
    pub fn notify_all(&self) -> usize {
        unsafe { parking_lot_core::unpark_all(self.key(), DEFAULT_UNPARK_TOKEN) }
    }

    #[inline(always)]
    pub fn wait(&self) {
        self.sleep(None);
    }

    #[inline(always)]
    pub fn wait_until(&self, timeout: Instant) -> bool {
        self.sleep(Some(timeout))
    }

    #[inline(always)]
    pub fn wait_for(&self, duration: Duration) -> bool {
        self.sleep(Instant::now().checked_add(duration))
    }

    #[inline(always)]
    pub fn wait_while<F: FnMut(&T) -> bool>(&self, mut callback: F) {
        self.sleep_while(None, &mut callback);
    }

    #[inline(always)]
    pub fn wait_while_until<F: FnMut(&T) -> bool>(
        &self,
        timeout: Instant,
        mut callback: F,
    ) -> bool {
        self.sleep_while(Some(timeout), &mut callback)
    }

    #[inline(always)]
    pub fn wait_while_for<F: FnMut(&T) -> bool>(
        &self,
        duration: Duration,
        mut callback: F,
    ) -> bool {
        self.sleep_while(Instant::now().checked_add(duration), &mut callback)
    }

    #[inline(always)]
    pub fn wait_with<R, F: FnMut(&T) -> Option<R>>(&self, mut callback: F) -> R {
        match self.sleep_with(None, &mut callback) {
            Ok(x) => x,
            Err(Timeout) => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    #[inline(always)]
    pub fn wait_with_until<R, F: FnMut(&T) -> Option<R>>(
        &self,
        timeout: Instant,
        mut callback: F,
    ) -> Result<R, Timeout> {
        self.sleep_with(Some(timeout), &mut callback)
    }

    #[inline(always)]
    pub fn wait_with_for<R, F: FnMut(&T) -> Option<R>>(
        &self,
        duration: Duration,
        mut callback: F,
    ) -> Result<R, Timeout> {
        self.sleep_with(Instant::now().checked_add(duration), &mut callback)
    }
}

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub struct WaitGroup(Arc<Waiter<AtomicUsize>>);

impl Default for WaitGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl WaitGroup {
    #[inline]
    pub fn new() -> Self {
        unsafe { Self(Arc::new(Waiter::with_value(AtomicUsize::new(1)))) }
    }

    #[inline]
    fn acquire_resource(&self) {
        self.0.inner.fetch_add(1, Ordering::Relaxed);
    }

    pub fn wait(self) {
        let inner = self.0.clone();
        drop(self);
        inner.wait_while(spin_wait(|inner: &AtomicUsize| {
            inner.load(Ordering::Acquire) > 0
        }))
    }

    pub fn wait_until(mut self, timeout: Instant) -> Result<(), Self> {
        let inner = self.0.clone();
        drop(self);

        let has_completed = inner.wait_while_until(
            timeout,
            spin_wait(|inner: &AtomicUsize| inner.load(Ordering::Acquire) > 0),
        );

        if has_completed {
            Ok(())
        } else {
            self = WaitGroup(inner);
            self.acquire_resource();
            Err(self)
        }
    }

    pub fn wait_for(mut self, duration: Duration) -> Result<(), Self> {
        let inner = self.0.clone();
        drop(self);

        let has_completed = inner.wait_while_for(
            duration,
            spin_wait(|inner: &AtomicUsize| inner.load(Ordering::Acquire) > 0),
        );

        if has_completed {
            Ok(())
        } else {
            self = WaitGroup(inner);
            self.acquire_resource();
            Err(self)
        }
    }
}

impl Clone for WaitGroup {
    #[inline]
    fn clone(&self) -> Self {
        self.acquire_resource();
        Self(self.0.clone())
    }
}

impl Drop for WaitGroup {
    #[inline]
    fn drop(&mut self) {
        self.0.inner.fetch_sub(1, Ordering::Relaxed);
        self.0.notify_all();
    }
}

use std::fmt;

impl fmt::Debug for WaitGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaitGroup")
            .field("waiters", &self.0.inner.load(Ordering::Relaxed))
            .finish()
    }
}
