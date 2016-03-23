//! Module containing information about hookable functions.
//!
//! The traits in this module are automatically implemented and should generally not be implemented
//! by users of this library.

use std::{fmt, mem};

use super::Hook;

/// An untyped function pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FnPointer(*mut ());

impl FnPointer {
    /// Creates a function pointer from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it can not check if the argument points to valid
    /// executable memory.
    pub unsafe fn from_raw<T>(ptr: *mut T) -> FnPointer { FnPointer(ptr as *mut _) }

    /// Returns function pointer as a raw pointer.
    pub fn to_raw<T>(&self) -> *mut T { self.0 as *mut _ }
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
    type Unsafe: UnsafeFunction;

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



macro_rules! impl_hookable {
    (recurse: () ($($nm:ident : $ty:ident),*)) => {
        impl_hookable!(impl_all: ($($nm : $ty),*));
    };
    (recurse: ($hd_nm:ident : $hd_ty:ident $(, $tl_nm:ident : $tl_ty:ident)*) ($($nm:ident : $ty:ident),*)) => {
        impl_hookable!(impl_all: ($($nm : $ty),*));
        impl_hookable!(recurse: ($($tl_nm : $tl_ty),*) ($($nm : $ty,)* $hd_nm : $hd_ty));
    };

    (impl_all: ($($nm:ident : $ty:ident),*)) => {
        impl_hookable!(impl_pair: ($($nm : $ty),*) (                  fn($($ty),*) -> Ret));
        impl_hookable!(impl_pair: ($($nm : $ty),*) (extern "cdecl"    fn($($ty),*) -> Ret));
        impl_hookable!(impl_pair: ($($nm : $ty),*) (extern "stdcall"  fn($($ty),*) -> Ret));
        impl_hookable!(impl_pair: ($($nm : $ty),*) (extern "fastcall" fn($($ty),*) -> Ret));
        impl_hookable!(impl_pair: ($($nm : $ty),*) (extern "win64"    fn($($ty),*) -> Ret));
        impl_hookable!(impl_pair: ($($nm : $ty),*) (extern "C"        fn($($ty),*) -> Ret));
        impl_hookable!(impl_pair: ($($nm : $ty),*) (extern "system"   fn($($ty),*) -> Ret));
    };

    (impl_pair: ($($nm:ident : $ty:ident),*) ($($fn_t:tt)*)) => {
        impl_hookable!(impl_fun: ($($nm : $ty),*) ($($fn_t)*) (unsafe $($fn_t)*));
    };

    (impl_fun: ($($nm:ident : $ty:ident),*) ($safe_type:ty) ($unsafe_type:ty)) => {
        impl_hookable!(impl_core: ($($nm : $ty),*) ($safe_type) ($unsafe_type));
        impl_hookable!(impl_core: ($($nm : $ty),*) ($unsafe_type) ($unsafe_type));

        impl_hookable!(impl_hookable_with: ($($nm : $ty),*) ($unsafe_type) ($safe_type));

        impl_hookable!(impl_safe: ($($nm : $ty),*) ($safe_type));
        impl_hookable!(impl_unsafe: ($($nm : $ty),*) ($unsafe_type));
    };

    (impl_hookable_with: ($($nm:ident : $ty:ident),*) ($target:ty) ($detour:ty)) => {
        unsafe impl<Ret: 'static, $($ty: 'static),*> HookableWith<$detour> for $target {}
    };

    // (impl_safe: ($nm:ident : $ty:ident) ($fn_type:ty)) => {
    //     impl<Ret: 'static, $ty: 'static> Hook<$fn_type> {
    //         /// Call the original function.
    //         #[inline]
    //         pub fn call_real(&self, $nm : $ty) -> Ret {
    //             (self.trampoline)($nm)
    //         }
    //     }
    // };

    (impl_safe: ($($nm:ident : $ty:ident),*) ($fn_type:ty)) => {
        impl<Ret: 'static, $($ty: 'static),*> Hook<$fn_type> {
            #[doc(hidden)]
            pub fn call_real(&self, $($nm : $ty),*) -> Ret {
                (self.trampoline)($($nm),*)
            }
        }
    };

    (impl_unsafe: ($($nm:ident : $ty:ident),*) ($fn_type:ty)) => {
        unsafe impl<Ret: 'static, $($ty: 'static),*> UnsafeFunction for $fn_type {}

        impl<Ret: 'static, $($ty: 'static),*> Hook<$fn_type> {
            #[doc(hidden)]
            pub unsafe fn call_real(&self, $($nm : $ty),*) -> Ret {
                (self.trampoline)($($nm),*)
            }
        }
    };

    (impl_core: ($($nm:ident : $ty:ident),*) ($fn_type:ty) ($unsafe_type:ty)) => {
        unsafe impl<Ret: 'static, $($ty: 'static),*> Function for $fn_type {
            type Args = ($($ty,)*);
            type Output = Ret;
            type Unsafe = $unsafe_type;

            const ARITY: usize = impl_hookable!(count: ($($ty)*));

            unsafe fn from_ptr(ptr: FnPointer) -> Self {
                mem::transmute(ptr.to_raw() as *mut ())
            }

            fn to_ptr(&self) -> FnPointer {
                unsafe { FnPointer::from_raw(*self as *mut ()) }
            }

            fn to_unsafe(&self) -> Self::Unsafe {
                unsafe { mem::transmute(*self) }
            }
        }
    };

    (count: ()) => {
        0
    };
    (count: ($hd:tt $($tl:tt)*)) => {
        1 + impl_hookable!(count: ($($tl)*))
    };

    ($($nm:ident : $ty:ident),*) => {
        impl_hookable!(recurse: ($($nm : $ty),*) ());
    };
}

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