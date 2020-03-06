use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Inner<T> {
    count: AtomicUsize,
    value: T,
}

pub struct Arc<T> {
    ptr: NonNull<Inner<T>>,
    drop: PhantomData<Inner<T>>,
}

impl<T> Arc<T> {
    pub fn new(value: T) -> Self {
        let inner = Inner {
            count: AtomicUsize::new(1),
            value,
        };

        let layout = Layout::new::<Inner<T>>();
        let ptr = unsafe { alloc(layout) };
        let ptr = match NonNull::new(ptr) {
            Some(ptr) => ptr,
            None => handle_alloc_error(layout),
        };

        let ptr: NonNull<Inner<T>> = ptr.cast();
        unsafe {
            ptr.as_ptr().write(inner);
        }

        Self {
            ptr,
            drop: PhantomData,
        }
    }

    #[cold]
    unsafe fn drop_slow(&mut self) {
        let ptr = self.ptr.as_ptr();
        ptr.drop_in_place();
        let layout = Layout::new::<Inner<T>>();
        dealloc(ptr.cast(), layout);
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        let count = unsafe { self.ptr.as_ref().count.fetch_sub(1, Ordering::Release) };

        if count == 1 {
            unsafe {
                self.drop_slow();
            }
        }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        unsafe {
            self.ptr.as_ref().count.fetch_add(1, Ordering::Relaxed);
        }

        Self { ..*self }
    }
}

impl<T> std::ops::Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &self.ptr.as_ref().value }
    }
}

#[test]
fn arc() {
    let arc = Arc::new(0);

    let b = arc.clone();

    drop(arc);

    drop(b);
}
