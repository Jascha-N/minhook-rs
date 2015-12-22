use std::{fmt, error};
use std::cell::UnsafeCell;
use std::sync::Once;

#[derive(Debug)]
pub enum Error<E> {
    Dead,
    Initialization(E)
}

impl<E: error::Error> error::Error for Error<E> {
    fn description(&self) -> &str {
        match *self {
            Error::Dead => "cell is dead",
            Error::Initialization(_) => "error during initialization"
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Initialization(ref error) => Some(error),
            _ => None
        }
    }
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Dead => write!(fmt, "The cell is dead (forever uninitialized)."),
            Error::Initialization(ref error) => write!(fmt, "An error occurred during initialization: {}.", error)
        }
    }
}

#[doc(hidden)]
pub struct InitCell<T: Sync> {
    data: UnsafeCell<Option<T>>,
    once: Once,
}

impl<T: Sync> InitCell<T> {
    #[doc(hidden)]
    pub const fn new() -> InitCell<T> {
        InitCell {
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
    pub fn initialize<F, E>(&'static self, f: F) -> Result<bool, Error<E>>
    where F: FnOnce() -> Result<T, E> {
        let mut result = None;

        self.once.call_once(|| {
            result = Some(f().map(|value| unsafe {
                self.set_unsync(value);
            }));
        });

        unsafe {
            result.map_or_else(|| self.get_ref_unsync().ok_or(Error::Dead).map(|_| false),
                               |result| result.map(|_| true).map_err(Error::Initialization))
        }
    }

    #[doc(hidden)]
    pub fn get(&'static self) -> Option<&'static T> {
        self.once.call_once(|| ());

        unsafe { self.get_ref_unsync() }
    }
}

unsafe impl<T: Sync> Sync for InitCell<T> {}