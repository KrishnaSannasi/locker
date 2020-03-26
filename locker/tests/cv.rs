use locker::condvar::Condvar;
use locker::mutex::default::DefaultLock;
use locker::Init;

type Mutex<T> = locker::mutex::Mutex<DefaultLock, T>;

#[test]
pub fn condvar() {
    struct Count {
        gen: u8,
        count: u8,
    }

    static CV: Condvar = Init::INIT;
    static MX: Mutex<Count> = Mutex::from_raw_parts(Init::INIT, Count { gen: 0, count: 0 });
    const COUNT: u8 = 10;

    let threads = (0..10 * COUNT)
        .map(|i| {
            std::thread::spawn(move || {
                let mut guard = MX.lock();
                let gen = i / COUNT;

                println!("sleep {}", i);
                while guard.gen != gen {
                    CV.notify_one();
                    CV.wait(&mut guard);
                }

                println!("reg {}", i);
                guard.count += 1;

                while guard.count < COUNT {
                    CV.notify_one();
                    CV.wait(&mut guard);
                }

                guard.count += 1;

                println!("complete {}", i);

                if guard.count == 2 * COUNT {
                    guard.count = 0;
                    guard.gen += 1;
                }

                CV.notify_one();
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        let _ = thread.join();
    }
    println!("done");
}
