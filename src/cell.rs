use std::cell::UnsafeCell;
use std::sync::{Once, StaticRwLock};

#[derive(Copy, PartialEq, Eq, Clone, Debug)]
pub enum Error {
    AlreadyInitialized,
    AccessedBeforeInitialization
}

#[doc(hidden)]
pub struct StaticInitCell<T> {
    data: UnsafeCell<Option<T>>,
    once: Once,
}

impl<T> StaticInitCell<T> {
    #[doc(hidden)]
    pub const fn new() -> StaticInitCell<T> {
        StaticInitCell {
            data: UnsafeCell::new(None),
            once: Once::new()
        }
    }

    unsafe fn set_unsync(&self, value: T) {
        *self.data.get() = Some(value)
    }

    unsafe fn get_ref_unsync(&self) -> Option<&T> {
        (*self.data.get()).as_ref()
    }

    #[doc(hidden)]
    pub fn initialize(&'static self, value: T) -> Result<(), Error> {
        let mut first = false;

        self.once.call_once(|| unsafe {
            self.set_unsync(value);
            first = true;
        });

        if first {
            Ok(())
        } else {
            unsafe {
                self.get_ref_unsync().ok_or(Error::AccessedBeforeInitialization)
                                     .and_then(|_| Err(Error::AlreadyInitialized))
            }
        }
    }

    #[doc(hidden)]
    pub fn get(&'static self) -> Result<&'static T, Error> {
        self.once.call_once(|| ());

        unsafe { self.get_ref_unsync().ok_or(Error::AccessedBeforeInitialization) }
    }
}

unsafe impl<T: Sync> Sync for StaticInitCell<T> {}



pub struct StaticRwCell<T> {
    data: UnsafeCell<T>,
    lock: StaticRwLock,
}

impl<T> StaticRwCell<T> {
    pub const fn new(value: T) -> StaticRwCell<T> {
        StaticRwCell {
            data: UnsafeCell::new(value),
            lock: StaticRwLock::new()
        }
    }

    unsafe fn set_unsync(&self, value: T) {
        *self.data.get() = value
    }

    unsafe fn get_ref_unsync(&self) -> &T {
        &*self.data.get()
    }

    pub fn set(&'static self, value: T) {
        let _lock = self.lock.write();

        unsafe { self.set_unsync(value); }
    }

    pub fn with<F, R>(&'static self, f: F) -> R
    where F: FnOnce(&T) -> R {
        let _lock = self.lock.read();

        unsafe { f(self.get_ref_unsync()) }
    }
}

impl<T> StaticRwCell<Option<T>> {
    pub fn take(&'static self) -> Option<T> {
        let _lock = self.lock.write();

        let option = unsafe { &mut *self.data.get() };
        option.take()
    }
}

unsafe impl<T: Send + Sync> Sync for StaticRwCell<T> {}
unsafe impl<T: Send + Sync> Send for StaticRwCell<T> {}