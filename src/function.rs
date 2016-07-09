//! Module containing information about hookable functions.
//!
//! The traits in this module are automatically implemented and should generally not be implemented
//! by users of this library.

use std::{fmt, mem};
use std::os::raw::c_void;

use super::Hook;



/// An untyped function pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FnPointer(*mut c_void);

impl FnPointer {
    /// Creates a function pointer from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it can not check if the argument points to valid
    /// executable memory.
    pub unsafe fn from_raw(ptr: *mut c_void) -> FnPointer { FnPointer(ptr) }

    /// Returns function pointer as a raw pointer.
    pub fn to_raw(&self) -> *mut c_void { self.0 }
}

impl fmt::Pointer for FnPointer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:p}", self.0)
    }
}



/// Trait representing a function that can be used as a target function or detour function for
/// hooking.
#[rustc_on_unimplemented = "The type `{Self}` is not an eligible target function or \
                            detour function."]
pub unsafe trait Function: Sized + Copy + Sync + 'static {
    /// Unsafe version of this function.
    type Unsafe: UnsafeFunction<Args = Self::Args, Output = Self::Output>;

    /// The argument types as a tuple.
    type Args;

    /// The return type.
    type Output;

    /// The function's arity (number of arguments).
    const ARITY: usize;

    /// Constructs a `Function` from an untyped function pointer.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it can not check if the argument points to a function
    /// of the correct type.
    unsafe fn from_ptr(ptr: FnPointer) -> Self;

    /// Returns a untyped function pointer for this function.
    fn to_ptr(&self) -> FnPointer;

    /// Returns this function as its unsafe variant.
    fn to_unsafe(&self) -> Self::Unsafe;
}



/// Trait representing an unsafe function.
pub unsafe trait UnsafeFunction: Function {}



/// Marker trait indicating that the function `Self` can be hooked by the given function `D`.
#[rustc_on_unimplemented = "The type `{D}` is not a suitable detour function type for a \
                            target function of type `{Self}`."]
pub unsafe trait HookableWith<D: Function>: Function {}

unsafe impl<T: Function> HookableWith<T> for T {}



#[cfg(not(feature = "increased_arity"))]
impl_hookable! {
    __arg_0:  A, __arg_1:  B, __arg_2:  C, __arg_3:  D, __arg_4:  E, __arg_5:  F, __arg_6:  G,
    __arg_7:  H, __arg_8:  I, __arg_9:  J, __arg_10: K, __arg_11: L
}

#[cfg(feature = "increased_arity")]
impl_hookable! {
    __arg_0:  A, __arg_1:  B, __arg_2:  C, __arg_3:  D, __arg_4:  E, __arg_5:  F, __arg_6:  G,
    __arg_7:  H, __arg_8:  I, __arg_9:  J, __arg_10: K, __arg_11: L, __arg_12: M, __arg_13: N,
    __arg_14: O, __arg_15: P, __arg_16: Q, __arg_17: R, __arg_18: S, __arg_19: T, __arg_20: U,
    __arg_21: V, __arg_22: W, __arg_23: X, __arg_24: Y, __arg_25: Z
}
