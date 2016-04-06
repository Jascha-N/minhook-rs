use std::{mem, ptr};
use std::cell::UnsafeCell;
use std::sync::StaticRwLock;
use std::sync::atomic::{AtomicPtr, Ordering};



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
        if !self.0.compare_and_swap(ptr::null_mut(), &mut *boxed, Ordering::SeqCst).is_null() {
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