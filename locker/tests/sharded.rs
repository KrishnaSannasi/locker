#[test]
#[cfg(all(feature = "extra", feature = "nightly", feature = "std"))]
fn test() {
    use locker::{
        remutex::std_thread::StdThreadInfo,
        rwlock::{default::DefaultLock, raw::RwLock, sharded::Sharded},
    };

    let rwlock: RwLock<Box<Sharded<_, [_]>>> = unsafe {
        RwLock::from_raw(Box::new(Sharded::<StdThreadInfo, [DefaultLock; 8]>::new()) as _)
    };

    let x = rwlock.write();

    assert!(rwlock.try_read().is_none());
    assert!(rwlock.try_write().is_none());
}
