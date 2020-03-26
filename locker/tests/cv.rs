use locker::condvar::Condvar;
use locker::mutex::default::DefaultLock;
use locker::Init;

type Mutex<T> = locker::mutex::Mutex<DefaultLock, T>;

#[test]
pub fn condvar() {
    static CV: Condvar = Condvar::new();
    static MX: Mutex<usize> = Mutex::from_raw_parts(Init::INIT, 0);
    const COUNT: usize = 10;

    let threads = (0..COUNT)
        .map(|i| {
            std::thread::spawn(move || {
                let mut guard = MX.lock();

                println!("register {}", i);
                *guard += 1;
                while *guard < COUNT {
                    CV.wait(&mut guard);
                }
                println!("complete {}", i);

                CV.notify_all();
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        let _ = thread.join();
    }
}
