use crate::spin_wait::SpinWait;

use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, Thread};

const LOCK_BIT: usize = 0b01;
const QUEUE_LOCK_BIT: usize = 0b10;
const QUEUE_MASK: usize = !(LOCK_BIT | QUEUE_LOCK_BIT);

use std::cell::Cell;

#[repr(align(4))]
struct ThreadData {
    thread: Thread,
    prev: Cell<*const ThreadData>,
    next: Cell<*const ThreadData>,
}

pub struct WordLock {
    state: AtomicUsize,
}

unsafe impl crate::mutex::RawMutex for WordLock {}
unsafe impl crate::RawLockInfo for WordLock {
    const INIT: Self = WordLock {
        state: AtomicUsize::new(0),
    };

    /// A type that will remove auto-trait implementations for the `*ExclusiveGuard` types
    type ExclusiveGuardTraits = ();

    /// A type that will remove auto-trait implementations for the `*ShareGuard` types
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for WordLock {
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.lock_slow();
        }
    }

    fn exc_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);

        state & LOCK_BIT == 0
            && state
                == self
                    .state
                    .compare_and_swap(state, state | LOCK_BIT, Ordering::Acquire)
    }

    unsafe fn exc_unlock(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        while state == LOCK_BIT {
            if let Err(x) =
                self.state
                    .compare_exchange_weak(LOCK_BIT, 0, Ordering::Release, Ordering::Relaxed)
            {
                state = x;
            } else {
                return;
            }
        }

        self.unlock_slow();
    }

    unsafe fn exc_bump(&self) {
        if self.state.load(Ordering::Relaxed) & QUEUE_MASK != 0 {
            self.bump_slow();
        }
    }
}

impl WordLock {
    #[cold]
    #[inline(never)]
    fn lock_slow(&self) {
        let mut state = self.state.load(Ordering::Relaxed);
        let mut wait = SpinWait::new();

        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state & LOCK_BIT == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    state | LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    state = x;
                } else {
                    return;
                }

                continue;
            }

            // If there is no queue, try spinning a few times
            if state & QUEUE_MASK != 0 || !wait.spin() {
                break;
            }
        }

        let thread_data = &ThreadData {
            thread: thread::current(),
            prev: Cell::new(ptr::null()),
            next: Cell::new(ptr::null()),
        };

        self.enqueue(thread_data);

        wait.reset();
        let remove_on_drop = RemoveOnDrop(self, thread_data);
        state = self.state.load(Ordering::Acquire);

        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it

            if state & LOCK_BIT == 0 {
                if let Err(x) = self.state.compare_exchange(
                    state,
                    state | LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    state = x;
                    continue;
                } else {
                    break;
                }
            }

            if wait.spin() {
                continue;
            }

            std::thread::park_timeout(std::time::Duration::from_micros(100));

            wait.reset();
            state = self.state.load(Ordering::Acquire);
        }

        drop(remove_on_drop);
    }

    fn lock_queue(&self) -> Lock<'_> {
        let mut state = self.state.load(Ordering::Acquire);
        let mut wait = SpinWait::new();

        loop {
            wait.spin();

            if state & QUEUE_LOCK_BIT == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    state | QUEUE_LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    state = x;
                } else {
                    break Lock(self);
                }
            } else {
                state = self.state.load(Ordering::Acquire);
            }
        }
    }

    #[cold]
    #[inline(never)]
    fn unlock_slow(&self) {
        std::mem::forget(self.lock_queue());

        // because there may be only one *exc lock* at any given time
        // (this lock is not splittable), this thread must be the only thread
        // that can modify state.

        // the queue lock bit is set, so no thread can park itself
        // the lock bit is set so no thread acquire a lock
        let state = self.state.load(Ordering::Relaxed);

        if state & QUEUE_MASK == 0 {
            // clear the lock bit, and the queue lock bit
            self.state.store(0, Ordering::Release);
        } else {
            {
                // pop head off of the queue
                let thread_data = (state & QUEUE_MASK) as *const ThreadData;
                unsafe { (*thread_data).thread.unpark() }

                // clear the lock bit, and the queue lock bit
                self.state.store(thread_data as usize, Ordering::Release);
            }
        }
    }

    #[cold]
    fn bump_slow(&self) {
        use crate::exclusive_lock::RawExclusiveLock;
        self.unlock_slow();
        self.exc_lock();
    }

    fn enqueue(&self, thread_data: &ThreadData) {
        let queue_lock = self.lock_queue();

        let mut state = self.state.load(Ordering::Relaxed);

        let head = (state & QUEUE_MASK) as *const ThreadData;

        if head.is_null() {
            while let Err(x) = self.state.compare_exchange_weak(
                state,
                state | (thread_data as *const ThreadData as usize),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                state = x;
            }
        } else {
            let head = unsafe { &*head };

            let tail = unsafe {
                let mut tail = head;

                while let Some(next) = tail.next.get().as_ref() {
                    tail = next;
                }

                tail
            };

            tail.next.set(thread_data);
            thread_data.prev.set(tail);
        }

        drop(queue_lock);
    }
}

struct Lock<'a>(&'a WordLock);

impl Drop for Lock<'_> {
    fn drop(&mut self) {
        let state = &self.0.state;

        state.fetch_and(!QUEUE_LOCK_BIT, Ordering::Release);
    }
}

struct RemoveOnDrop<'a>(&'a WordLock, &'a ThreadData);

impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) {
        let &mut RemoveOnDrop(lock, thread_data) = self;
        let queue_lock = lock.lock_queue();

        let mut state = lock.state.load(Ordering::Relaxed);

        if let Some(prev) = unsafe { thread_data.prev.get().as_ref() } {
            let next = thread_data.next.get();
            if let Some(next) = unsafe { next.as_ref() } {
                next.prev.set(prev);
            }

            prev.next.set(next);
        } else {
            // if head

            unsafe {
                if let Some(next) = thread_data.next.get().as_ref() {
                    next.prev.set(ptr::null());
                }
            }

            while let Err(x) = lock.state.compare_exchange_weak(
                state,
                (state & !QUEUE_MASK) | (thread_data.next.get() as usize),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                state = x;
            }
        }

        drop(queue_lock);
    }
}

#[test]
fn park() {
    static MTX: crate::mutex::Mutex<WordLock, ()> = unsafe {
        crate::mutex::Mutex::from_raw_parts(
            crate::mutex::raw::Mutex::from_raw(<WordLock as crate::RawLockInfo>::INIT),
            (),
        )
    };

    let a = MTX.lock();

    let all: Vec<_> = (0..1000)
        .map(|_| {
            std::thread::spawn(move || {
                // let mut mtx = &mut *MTX.lock();
                MTX.lock();
                // *mtx += 1;

                // if *mtx % 1000 == 0 {
                //     println!("mtx = {}", mtx);
                // }
            })
        })
        .collect();

    std::thread::sleep(std::time::Duration::from_millis(1));

    drop(a);

    for i in all {
        let _ = i.join();
    }
}
