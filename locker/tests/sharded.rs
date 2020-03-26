#[test]
fn test() {
    use locker::{
        remutex::std_thread::StdThreadInfo,
        rwlock::{default::DefaultLock, raw::RwLock, sharded::Sharded},
    };

    let rwlock = unsafe { RwLock::from_raw(Sharded::<StdThreadInfo, [DefaultLock; 8]>::new()) };
    let rwlock: &RwLock<Sharded<_, [_]>> = &rwlock;

    let x = rwlock.write();

    assert!(rwlock.try_read().is_none());
    assert!(rwlock.try_write().is_none());
}
