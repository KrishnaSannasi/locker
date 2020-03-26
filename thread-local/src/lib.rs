use locker::{once::simple::OnceCell, Init};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::thread::ThreadId;

type Lock = locker::rwlock::default::DefaultLock;
type RwLock = locker::rwlock::raw::RwLock<Lock>;

#[doc(hidden)]
pub use std::boxed::Box;

#[macro_export]
macro_rules! thread_local {
    () => {};
    (#[raw] $(#[$meta:meta])* $v:vis static $name:ident: $type:ty = $expr:expr; $($rest:tt)*) => {
        $(#[$meta])*
        $v static $name: $crate::LocalKey<$type> = unsafe { $crate::LocalKey::new(move || $expr) };

        $crate::thread_local! { $($rest)* }
    };
    ($(#[$meta:meta])* $v:vis static $name:ident: $type:ty = $expr:expr; $($rest:tt)*) => {
        $(#[$meta])*
        $v static $name: $crate::LocalKey<$type> = unsafe { $crate::LocalKey::new(move || $crate::Box::from($expr)) };

        $crate::thread_local! { $($rest)* }
    };
}

use std::sync::atomic::{AtomicUsize, Ordering};

static COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    pub static FOO: [u32] = vec![0; COUNT.fetch_add(1, Ordering::Relaxed)];
}

pub fn get() -> &'static [u32] {
    &*FOO
}

pub struct LocalKey<T: ?Sized, F = fn() -> Box<T>> {
    inner: OnceCell<ThreadLocal<T>>,
    init: F,
}

unsafe impl<T: ?Sized, F: Send> Send for LocalKey<T, F> {}
unsafe impl<T: ?Sized, F: Sync> Sync for LocalKey<T, F> {}

impl<T: ?Sized, F> LocalKey<T, F> {
    #[doc(hidden)]
    pub const unsafe fn new(init: F) -> Self {
        Self {
            inner: Init::INIT,
            init,
        }
    }
}

impl<T: ?Sized, F: Fn() -> Box<T>> std::ops::Deref for LocalKey<T, F> {
    type Target = T;

    fn deref(&self) -> &T {
        let inner = self.inner.get_or_init(ThreadLocal::new);
        inner.get_or_insert_with(&self.init)
    }
}

pub struct ThreadLocal<T: ?Sized> {
    lock: RwLock,
    inner: UnsafeCell<HashMap<ThreadId, Box<T>>>,
}

unsafe impl<T: Send> Sync for ThreadLocal<T> {}

impl<T: ?Sized> Default for ThreadLocal<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized> ThreadLocal<T> {
    pub fn new() -> Self {
        Self {
            lock: RwLock::default(),
            inner: UnsafeCell::default(),
        }
    }

    pub fn get(&self) -> Option<&T> {
        let thread_id = std::thread::current().id();
        let _lock = self.lock.read();
        let inner = unsafe { &*self.inner.get() };
        Some(inner.get(&thread_id)? as _)
    }

    pub fn get_or_insert_with<F: FnOnce() -> V, V: Into<Box<T>>>(&self, value: F) -> &T {
        let thread_id = std::thread::current().id();
        let _lock = self.lock.read();

        unsafe {
            let inner = &*self.inner.get();

            if let Some(item) = inner.get(&thread_id) {
                return item;
            }
        }

        let mut value = Some(value);
        let value = &mut move || Ok::<_, std::convert::Infallible>(value.take().unwrap()().into());
        match self.try_insert(_lock, thread_id, value) {
            Ok(x) => x,
            Err(x) => match x {},
        }
    }

    pub fn get_or_try_insert_with<F: FnOnce() -> Result<V, E>, E, V: Into<Box<T>>>(
        &self,
        value: F,
    ) -> Result<&T, E> {
        let thread_id = std::thread::current().id();
        let _lock = self.lock.read();

        unsafe {
            let inner = &*self.inner.get();

            if let Some(item) = inner.get(&thread_id) {
                return Ok(item);
            }
        }

        let mut value = Some(value);
        let value = &mut move || value.take().unwrap()().map(V::into);
        self.try_insert(_lock, thread_id, value)
    }

    #[cold]
    fn try_insert<E>(
        &self,
        _lock: locker::share_lock::RawShareGuard<Lock>,
        thread_id: ThreadId,
        value: &mut dyn FnMut() -> Result<Box<T>, E>,
    ) -> Result<&T, E> {
        use std::collections::hash_map::Entry;
        let _lock = _lock.upgrade();

        let inner = unsafe { &mut *self.inner.get() };

        Ok(match inner.entry(thread_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(value()?),
        })
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        unsafe {
            IterMut {
                inner: (*self.inner.get()).iter_mut(),
            }
        }
    }
}

impl<T> ThreadLocal<T> {
    pub fn get_or_insert(&self, value: T) -> &T {
        self.get_or_insert_with(move || value)
    }
}

pub struct IterMut<'a, T: ?Sized> {
    inner: std::collections::hash_map::IterMut<'a, ThreadId, Box<T>>,
}

pub struct IntoIter<T: ?Sized> {
    inner: std::collections::hash_map::IntoIter<ThreadId, Box<T>>,
}

impl<'a, T: ?Sized> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, item) = self.inner.next()?;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T: ?Sized> Iterator for IntoIter<T> {
    type Item = Box<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, item) = self.inner.next()?;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T: ?Sized> IntoIterator for &'a mut ThreadLocal<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a, T: ?Sized> IntoIterator for ThreadLocal<T> {
    type Item = Box<T>;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.inner.into_inner().into_iter(),
        }
    }
}
