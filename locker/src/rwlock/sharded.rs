use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::rwlock::default::DefaultLock;
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

// pub struct Sharded<L, const N: usize> {
//     pub shards: [L; N],
// }

// unsafe impl<T, const N: usize> RawLockInfo for Sharded<L, N> {}
