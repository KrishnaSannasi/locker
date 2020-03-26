use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::rwlock::default::DefaultLock;
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

pub struct DynSharded<S: ?Sized> {
    pub shards: S,
}
