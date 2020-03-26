#![allow(missing_docs)]

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::mutex::RawMutex;
use crate::remutex::ThreadInfo;
use crate::rwlock::RawRwLock;
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

pub struct Sharded<I, S: ?Sized> {
    thread_info: I,
    shards: S,
}

impl<I, S> Sharded<I, S> {
    /// # Safety
    ///
    /// There must be at least 1 shard in the shards
    pub const unsafe fn from_raw_parts(thread_info: I, shards: S) -> Self {
        Self {
            thread_info,
            shards,
        }
    }
}

#[cfg(feature = "nightly")]
impl<I: ThreadInfo + crate::Init, S: RawRwLock + crate::Init, const N: usize> Sharded<I, [S; N]> {
    pub fn new() -> Self {
        unsafe {
            use core::mem::MaybeUninit;
            let mut shards = MaybeUninit::<[S; N]>::uninit();
            let mut ptr = shards.as_mut_ptr().cast::<S>();

            for _ in 0..N {
                ptr.write(crate::Init::INIT);
                ptr = ptr.add(1);
            }

            Self {
                thread_info: crate::Init::INIT,
                shards: shards.assume_init(),
            }
        }
    }
}

unsafe impl<I: ThreadInfo, S: RawMutex> RawMutex for Sharded<I, [S]> {}
unsafe impl<I: ThreadInfo, S: RawRwLock> RawRwLock for Sharded<I, [S]> {}

impl<I: ThreadInfo, S> Sharded<I, [S]> {
    pub fn get(&self) -> &S {
        let id = self.thread_info.id().get() % self.shards.len();

        &self.shards[id]
    }
}

unsafe impl<I, S: RawLockInfo> RawLockInfo for Sharded<I, [S]> {
    type ExclusiveGuardTraits = S::ExclusiveGuardTraits;
    type ShareGuardTraits = S::ShareGuardTraits;
}

unsafe impl<I: ThreadInfo, S: RawShareLock> RawShareLock for Sharded<I, [S]> {
    fn shr_lock(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.get().shr_lock();
    }

    fn shr_try_lock(&self) -> bool {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.get().shr_try_lock()
    }

    unsafe fn shr_split(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.get().shr_split()
    }

    unsafe fn shr_unlock(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.get().shr_unlock();
    }

    unsafe fn shr_bump(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.get().shr_bump();
    }
}

unsafe impl<I: ThreadInfo, S: RawShareLockFair> RawShareLockFair for Sharded<I, [S]> {
    unsafe fn shr_unlock_fair(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.get().shr_unlock_fair();
    }

    unsafe fn shr_bump_fair(&self) {
        self.get().shr_bump_fair();
    }
}

unsafe impl<I: ThreadInfo, S: RawExclusiveLock> RawExclusiveLock for Sharded<I, [S]> {
    fn exc_lock(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        for shard in self.shards.iter() {
            shard.exc_lock();
        }
    }

    fn exc_try_lock(&self) -> bool {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        let locked = self
            .shards
            .iter()
            .take_while(|shard| shard.exc_try_lock())
            .count();

        if locked == self.shards.len() {
            return true;
        }

        unsafe {
            self.shards
                .iter()
                .take(locked)
                .for_each(|shard| shard.exc_unlock());
        }

        false
    }

    unsafe fn exc_unlock(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.shards.iter().for_each(|shard| shard.exc_unlock())
    }
}

unsafe impl<I: ThreadInfo, S: RawExclusiveLockFair> RawExclusiveLockFair for Sharded<I, [S]> {
    unsafe fn exc_unlock_fair(&self) {
        debug_assert!(
            !self.shards.is_empty(),
            "You cannot use an empty shard list in a `Sharded`"
        );
        self.shards.iter().for_each(|shard| shard.exc_unlock_fair())
    }
}
