pub struct Defer<F: FnOnce()> {
    func: Option<F>,
}

impl<F: FnOnce()> Defer<F> {
    pub fn new(func: F) -> Defer<F> {
        Self { func: Some(func) }
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        self.func.take().unwrap()()
    }
}