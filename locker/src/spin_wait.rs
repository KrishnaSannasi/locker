#[cfg(feature = "parking_lot_core")]
pub use parking_lot_core::SpinWait;

// Wastes some CPU time for the given number of iterations,
// using a hint to indicate to the CPU that we are spinning.
#[inline]
#[cfg(not(feature = "parking_lot_core"))]
fn cpu_relax(iterations: u32) {
    for _ in 0..iterations {
        core::sync::atomic::spin_loop_hint()
    }
}

#[cfg(not(feature = "parking_lot_core"))]
pub struct SpinWait {
    counter: u32,
}

#[cfg(not(feature = "parking_lot_core"))]
impl SpinWait {
    /// Creates a new `SpinWait`.
    #[inline]
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Spins until the sleep threshold has been reached.
    ///
    /// This function returns whether the sleep threshold has been reached, at
    /// which point further spinning has diminishing returns and the thread
    /// should be parked instead.
    ///
    /// The spin strategy will initially use a CPU-bound loop but will fall back
    /// to yielding the CPU to the OS after a few iterations.
    #[inline]
    pub fn spin(&mut self) -> bool {
        self.counter = self.counter.min(9) + 1;

        #[cfg(feature = "std")]
        {
            if self.counter > 3 {
                std::thread::yield_now();
                return self.counter < 10;
            }
        }

        cpu_relax(1 << self.counter);
        self.counter < 10
    }

    #[inline]
    pub fn reset(&mut self) {
        self.counter = 0;
    }
}
