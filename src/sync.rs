use std::cell::RefCell;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::RwLock;
use std::{mem, ptr};

use lazy_static::lazy::Lazy;

#[doc(hidden)]
pub struct AtomicInitCell<T>(AtomicPtr<T>);

impl<T> AtomicInitCell<T> {
    #[doc(hidden)]
    pub const fn new() -> AtomicInitCell<T> {
        AtomicInitCell(AtomicPtr::new(ptr::null_mut()))
    }

    #[doc(hidden)]
    pub fn initialize(&self, value: T) -> Result<(), ()> {
        let mut boxed = Box::new(value);
        if !self
            .0
            .compare_and_swap(ptr::null_mut(), &mut *boxed, Ordering::SeqCst)
            .is_null()
        {
            return Err(());
        }
        mem::forget(boxed);
        Ok(())
    }

    #[doc(hidden)]
    pub fn get(&self) -> Option<&'static T> {
        let data = self.0.load(Ordering::SeqCst);
        if data.is_null() {
            return None;
        }
        unsafe { Some(&*data) }
    }
}

pub struct StaticRwCell<T: Send + Sync> {
    init: RefCell<Option<T>>,
    lock: Lazy<RwLock<T>>,
}

impl<T: Send + Sync> StaticRwCell<T> {
    pub const fn new(value: T) -> StaticRwCell<T> {
        StaticRwCell {
            init: RefCell::new(Some(value)),
            lock: Lazy::INIT,
        }
    }

    fn lock(&'static self) -> &RwLock<T> {
        self.lock
            .get(|| RwLock::new(self.init.borrow_mut().take().unwrap()))
    }

    pub fn set(&'static self, value: T) {
        let mut data = self.lock().write().unwrap();
        *data = value;
    }

    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let data = self.lock().read().unwrap();
        f(&*data)
    }
}

impl<T: Send + Sync> StaticRwCell<Option<T>> {
    pub fn take(&'static self) -> Option<T> {
        let mut data = self.lock().write().unwrap();
        data.take()
    }
}

unsafe impl<T: Send + Sync> Sync for StaticRwCell<T> {}
unsafe impl<T: Send + Sync> Send for StaticRwCell<T> {}
