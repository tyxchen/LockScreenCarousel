#[macro_export]
macro_rules! wait {
    ($async:expr) => {
        let async_ref = &$async;
        while let Ok(windows_future::AsyncStatus::Started) = async_ref.Status() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    };
}

#[macro_export]
macro_rules! b {
    ($s:literal) => {
        BSTR::from_wide(w!($s).as_wide())
    };
}

pub struct DropGuard<F: Fn() + 'static> {
    dropper: F,
}

impl<F: Fn() + 'static> DropGuard<F> {
    pub fn new(dropper: F) -> Self {
        Self { dropper }
    }
}

impl<F: Fn() + 'static> Drop for DropGuard<F> {
    fn drop(&mut self) {
        (self.dropper)();
    }
}

