use crate::exclusive_lock::RawExclusiveLock;
use crate::RawLockInfo;

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;

use core::ops::{Deref, DerefMut};

#[cfg(feature = "parking_lot_core")]
pub mod local;
#[cfg(feature = "parking_lot_core")]
pub mod simple;

pub trait AsRawExclusiveLock {
    fn as_raw_exclusive_lock(&self) -> &dyn RawExclusiveLock;
}

impl<L: RawExclusiveLock> AsRawExclusiveLock for L {
    fn as_raw_exclusive_lock(&self) -> &dyn RawExclusiveLock {
        self
    }
}

pub unsafe trait Finish: RawExclusiveLock + AsRawExclusiveLock {
    fn is_done(&self) -> bool;

    fn mark_done(&self);

    fn is_poisoned(&self) -> bool;

    fn mark_poisoned(&self);
}

pub struct Once<L> {
    lock: L,
}

#[cfg(feature = "std")]
impl<L: Finish> std::panic::RefUnwindSafe for Once<L> {}

impl<L: RawLockInfo + crate::Init> Default for Once<L> {
    #[inline]
    fn default() -> Self {
        crate::Init::INIT
    }
}

impl<L: crate::Init> crate::Init for Once<L> {
    const INIT: Self = unsafe { Once::from_raw(crate::Init::INIT) };
}

impl<L> Once<L> {
    /// # Safety
    ///
    /// * `lock` must not be shared, and must be freshly created
    #[inline]
    pub const unsafe fn from_raw(lock: L) -> Self {
        Self { lock }
    }
}

pub struct OnceState(bool);

impl OnceState {
    #[inline]
    pub const fn is_poisoned(&self) -> bool {
        self.0
    }
}

#[inline(always)]
fn panic_on_poison(f: impl FnOnce()) -> impl FnOnce(&OnceState) {
    #[cold]
    fn handle_poison() {
        panic!("tried to call `call_once*` on a poisoned `Once`");
    }

    move |once_state| {
        if once_state.is_poisoned() {
            handle_poison()
        }

        f()
    }
}

#[cold]
#[inline(never)]
fn run_once_unchecked<F: ?Sized + Finish>(lock: &F, f: impl FnOnce(&OnceState)) {
    struct Poison<'a, F: ?Sized + Finish>(&'a F);

    impl<F: ?Sized + Finish> Drop for Poison<'_, F> {
        fn drop(&mut self) {
            self.0.mark_poisoned();
        }
    }

    let is_poisoned = lock.is_poisoned();
    let poison = Poison(lock);

    f(&OnceState(is_poisoned));

    core::mem::forget(poison);

    lock.mark_done();
}

#[cold]
#[inline(never)]
fn force_call_once_slow(lock: &dyn Finish, f: &mut dyn FnMut(&OnceState)) {
    struct LocalGuard<'a>(&'a dyn RawExclusiveLock);

    impl Drop for LocalGuard<'_> {
        fn drop(&mut self) {
            unsafe { self.0.exc_unlock() }
        }
    }

    lock.exc_lock();
    let _guard = LocalGuard(lock.as_raw_exclusive_lock());

    if !lock.is_done() {
        run_once_unchecked(lock, f)
    }
}

impl<L: Finish> Once<L> {
    #[inline]
    pub fn call_once(&self, f: impl FnOnce()) {
        self.force_call_once(panic_on_poison(f))
    }

    #[inline]
    pub fn call_once_mut(&mut self, f: impl FnOnce()) {
        self.force_call_once_mut(panic_on_poison(f))
    }

    #[inline]
    pub fn force_call_once(&self, f: impl FnOnce(&OnceState)) {
        if !self.lock.is_done() {
            let mut f = Some(f);

            let mut f = move |once_state: &OnceState| f.take().unwrap()(once_state);

            force_call_once_slow(&self.lock, &mut f);
        }
    }

    #[inline]
    pub fn force_call_once_mut(&mut self, f: impl FnOnce(&OnceState)) {
        if !self.lock.is_done() {
            run_once_unchecked(&self.lock, f);
        }
    }
}

pub struct OnceCell<L: Finish, T> {
    once: Once<L>,
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<L: Finish, T: Send + Sync> Sync for OnceCell<L, T> where Once<L>: Sync {}

impl<L: Finish, T> Drop for OnceCell<L, T> {
    fn drop(&mut self) {
        if core::mem::needs_drop::<T>() && self.once.lock.is_done() {
            unsafe { self.value.get().cast::<T>().drop_in_place() }
        }
    }
}

impl<L: Finish + crate::Init, T> Default for OnceCell<L, T> {
    #[inline]
    fn default() -> Self {
        Self {
            once: crate::Init::INIT,
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

impl<L: Finish + crate::Init, T> crate::Init for OnceCell<L, T> {
    const INIT: Self = Self {
        once: crate::Init::INIT,
        value: UnsafeCell::new(MaybeUninit::uninit()),
    };
}

#[cfg(feature = "nightly")]
impl<L: Finish + crate::Init, T> OnceCell<L, T> {
    #[inline]
    pub const fn new() -> Self {
        crate::Init::INIT
    }
}

impl<L: Finish, T> OnceCell<L, T> {
    #[inline]
    pub fn get(&self) -> Option<&T> {
        if self.once.lock.is_done() {
            unsafe { Some(self.get_unchecked()) }
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.once.lock.is_done() {
            unsafe { Some(self.get_unchecked_mut()) }
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// The `OnceCell` must have be initialized
    #[inline]
    pub unsafe fn get_unchecked(&self) -> &T {
        &*self.value.get().cast::<T>()
    }

    /// # Safety
    ///
    /// The `OnceCell` must have be initialized
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self) -> &mut T {
        &mut *self.value.get().cast::<T>()
    }

    #[inline]
    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        let ptr = self.value.get().cast::<T>();

        self.once
            .force_call_once(move |_once_state| unsafe { ptr.write(f()) });

        unsafe { &*ptr }
    }

    #[inline]
    pub fn get_or_init_mut(&mut self, f: impl FnOnce() -> T) -> &mut T {
        let ptr = self.value.get().cast::<T>();

        if !self.once.lock.is_done() {
            let value = f();

            run_once_unchecked(&self.once.lock, move |_once_state| unsafe {
                ptr.write(value)
            });
        }

        unsafe { &mut *ptr }
    }

    #[inline]
    pub fn get_or_init_racy(&self, f: impl FnOnce() -> T) -> &T {
        let ptr = self.value.get().cast::<T>();

        if !self.once.lock.is_done() {
            let value = f();

            self.once
                .force_call_once(move |_once_state| unsafe { ptr.write(value) });
        }

        unsafe { &*ptr }
    }
}

enum LazyInner<F, T> {
    Func(F),
    Value(T),
    Empty,
}

pub enum Panic {}
pub enum Retry {}

pub struct Lazy<L, T, F, S> {
    once: Once<L>,
    inner: UnsafeCell<LazyInner<F, T>>,
    strategy: PhantomData<S>,
}

unsafe impl<L, F: Send + Sync, T: Send + Sync, S> Sync for Lazy<L, T, F, S> where Once<L>: Sync {}

impl<L: Finish + crate::Init, T, F: FnOnce() -> T> Lazy<L, T, F, Panic> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(func: F) -> Self {
                unsafe { Self::from_raw_parts(crate::Init::INIT, func) }
            }
        } else {
            #[inline]
            pub fn new(func: F) -> Self {
                unsafe { Self::from_raw_parts(crate::Init::INIT, func) }
            }
        }
    }
}

impl<L: Finish + crate::Init, T, F: FnMut() -> T> Lazy<L, T, F, Retry> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new_retry(func: F) -> Self {
                unsafe { Self::from_raw_parts(crate::Init::INIT, func) }
            }
        } else {
            #[inline]
            pub fn new_retry(func: F) -> Self {
                unsafe { Self::from_raw_parts(crate::Init::INIT, func) }
            }
        }
    }
}

impl<L, F, T, S> Lazy<L, T, F, S> {
    /// # Safety
    ///
    /// * `once` must be a freshly created `Once`
    #[inline]
    pub const unsafe fn from_raw_parts(once: Once<L>, func: F) -> Self {
        Self {
            once,
            strategy: PhantomData,
            inner: UnsafeCell::new(LazyInner::Func(func)),
        }
    }

    /// # Safety
    ///
    /// `Lazy::force` or `Lazy::force_mut` mut have been called before this
    #[inline]
    #[allow(unreachable_code)]
    pub unsafe fn get_unchecked(this: &Self) -> &T {
        if let LazyInner::Value(ref value) = *this.inner.get() {
            value
        } else {
            #[cfg(debug_assertions)]
            unreachable!("soundness hole");
            core::hint::unreachable_unchecked()
        }
    }

    /// # Safety
    ///
    /// `Lazy::force` or `Lazy::force_mut` mut have been called before this
    #[inline]
    #[allow(unreachable_code)]
    pub unsafe fn get_unchecked_mut(this: &mut Self) -> &mut T {
        if let LazyInner::Value(ref mut value) = *this.inner.get() {
            value
        } else {
            #[cfg(debug_assertions)]
            unreachable!("soundness hole");
            core::hint::unreachable_unchecked()
        }
    }
}

impl<L: Finish, F: FnOnce() -> T, T> Lazy<L, T, F, Panic> {
    #[inline]
    pub fn force(this: &Self) -> &T {
        let inner = this.inner.get();

        this.once.call_once(move || {
            let inner = unsafe { &mut *inner };
            let func = core::mem::replace(inner, LazyInner::Empty);

            if let LazyInner::Func(func) = func {
                *inner = LazyInner::Value(func());
            }
        });

        unsafe { Self::get_unchecked(this) }
    }

    #[inline]
    pub fn force_mut(this: &mut Self) -> &mut T {
        let inner = this.inner.get();

        this.once.call_once(move || {
            let inner = unsafe { &mut *inner };
            let func = core::mem::replace(inner, LazyInner::Empty);

            if let LazyInner::Func(func) = func {
                *inner = LazyInner::Value(func());
            }
        });

        unsafe { Self::get_unchecked_mut(this) }
    }
}

impl<L: Finish, F: FnMut(&OnceState) -> T, T> Lazy<L, T, F, Retry> {
    #[inline]
    pub fn force(this: &Self) -> &T {
        let inner = this.inner.get();

        this.once.force_call_once(move |once_state| {
            let inner = unsafe { &mut *inner };

            if let LazyInner::Func(ref mut func) = *inner {
                *inner = LazyInner::Value(func(once_state));
            }
        });

        unsafe { Self::get_unchecked(this) }
    }

    #[inline]
    pub fn force_mut(this: &mut Self) -> &mut T {
        let inner = this.inner.get();

        this.once.force_call_once_mut(move |once_state| {
            let inner = unsafe { &mut *inner };

            if let LazyInner::Func(ref mut func) = *inner {
                *inner = LazyInner::Value(func(once_state));
            }
        });

        unsafe { Self::get_unchecked_mut(this) }
    }
}

impl<L: Finish, F: FnOnce() -> T, T> Deref for Lazy<L, T, F, Panic> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        Self::force(self)
    }
}

impl<L: Finish, F: FnOnce() -> T, T> DerefMut for Lazy<L, T, F, Panic> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        Self::force_mut(self)
    }
}

impl<L: Finish, F: FnMut(&OnceState) -> T, T> Deref for Lazy<L, T, F, Retry> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        Self::force(self)
    }
}

impl<L: Finish, F: FnMut(&OnceState) -> T, T> DerefMut for Lazy<L, T, F, Retry> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        Self::force_mut(self)
    }
}

pub struct RacyLazy<L: Finish, T, F = fn() -> T> {
    once: OnceCell<L, T>,
    func: F,
}

unsafe impl<L: Finish, F: Send + Sync, T: Send + Sync> Sync for RacyLazy<L, T, F> where Once<L>: Sync
{}

impl<L: Finish + crate::Init, T, F: Fn() -> T> RacyLazy<L, T, F> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(func: F) -> Self {
                Self { once: crate::Init::INIT, func }
            }
        } else {
            #[inline]
            pub fn new(func: F) -> Self {
                Self { once: crate::Init::INIT, func }
            }
        }
    }
}

impl<L: Finish, F, T> RacyLazy<L, T, F> {
    /// # Safety
    ///
    /// `Lazy::force` or `Lazy::force_mut` mut have been called before this
    #[inline]
    #[allow(unreachable_code)]
    pub unsafe fn get_unchecked(this: &Self) -> &T {
        this.once.get_unchecked()
    }

    /// # Safety
    ///
    /// `Lazy::force` or `Lazy::force_mut` mut have been called before this
    #[inline]
    #[allow(unreachable_code)]
    pub unsafe fn get_unchecked_mut(this: &mut Self) -> &mut T {
        this.once.get_unchecked_mut()
    }
}

impl<L: Finish, F: Fn() -> T, T> RacyLazy<L, T, F> {
    #[inline]
    pub fn force(this: &Self) -> &T {
        this.once.get_or_init_racy(&this.func)
    }

    #[inline]
    pub fn force_mut(this: &mut Self) -> &mut T {
        this.once.get_or_init_mut(&this.func)
    }
}

impl<L: Finish, F: Fn() -> T, T> Deref for RacyLazy<L, T, F> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        Self::force(self)
    }
}

impl<L: Finish, F: Fn() -> T, T> DerefMut for RacyLazy<L, T, F> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        Self::force_mut(self)
    }
}
