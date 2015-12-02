//! # The minhook-rs library
//! This library provides function hooking support to Rust by providing a
//! Rust wrapper around the [MinHook][minhook] library.
//!
//! [minhook]: http://www.codeproject.com/KB/winsdk/LibMinHook.aspx
#![cfg_attr(feature = "unstable", feature(on_unimplemented, static_mutex))]
#![warn(missing_docs)]

use std::{mem, ops, result, fmt};
use std::sync::{MutexGuard, Once, ONCE_INIT};
use std::os::raw::c_void;

pub use error::Error;



mod error;

mod imp {
    use std::mem;
    use std::os::raw::c_void;

    use super::ffi::*;
    use super::{FnPointer, Error, Result};

    #[inline]
    fn status_to_result(status: MH_STATUS) -> Result<()> {
        Error::from(status).map_or(Ok(()), Err)
    }

    #[inline]
    pub unsafe fn initialize() -> Result<()> {
        status_to_result(MH_Initialize())
    }

    #[inline]
    pub unsafe fn uninitialize() -> Result<()> {
        status_to_result(MH_Uninitialize())
    }

    #[inline]
    pub unsafe fn create_hook(FnPointer(target): FnPointer, FnPointer(detour): FnPointer) -> Result<FnPointer> {
        let mut trampoline: *mut c_void = mem::uninitialized();

        status_to_result(MH_CreateHook(target, detour, &mut trampoline))
            .map(|_| FnPointer(trampoline))
    }

    #[inline]
    pub unsafe fn remove_hook(FnPointer(target): FnPointer) -> Result<()> {
        status_to_result(MH_RemoveHook(target))
    }

    #[inline]
    pub unsafe fn enable_hook(FnPointer(target): FnPointer) -> Result<()> {
        status_to_result(MH_EnableHook(target))
    }

    #[inline]
    pub unsafe fn disable_hook(FnPointer(target): FnPointer) -> Result<()> {
        status_to_result(MH_DisableHook(target))
    }

    #[inline]
    pub unsafe fn queue_enable_hook(FnPointer(target): FnPointer) -> Result<()> {
        status_to_result(MH_QueueEnableHook(target))
    }

    #[inline]
    pub unsafe fn queue_disable_hook(FnPointer(target): FnPointer) -> Result<()> {
        status_to_result(MH_QueueDisableHook(target))
    }

    #[inline]
    pub unsafe fn apply_queued() -> Result<()> {
        status_to_result(MH_ApplyQueued())
    }
}

pub mod ffi;

/// The minhook-rs prelude.
///
/// Glob import this prelude to bring commonly used traits into scope.
pub mod prelude {
    pub use super::{Hook, LazyStaticHook, LazyStaticHookInit};
}



/// Result type for all functions and methods in this module.
pub type Result<T> = result::Result<T, Error>;



/// Initializes the minhook-rs library.
///
/// It is not required to call this function excplicitly as the other library functions will do it internally.
/// Calling this function again after a previous successful initialization is a no-op.
pub fn initialize() -> Result<()> {
    static INIT: Once = ONCE_INIT;

    let mut result: Result<()> = Ok(());
    INIT.call_once(|| unsafe {
        result = imp::initialize();
    });
    result
}

/// Uninitializes the minhook-rs library.
///
/// # Unsafety
///
/// This function is very unsafe because any live hooks might still depend on this library. After
/// calling this function existing trampoline functions might point to uninitialized memory.
/// Only use this function when you are absolutely sure no hook objects will be accessed after
/// its use.
pub unsafe fn uninitialize() -> Result<()> {
    imp::uninitialize()
}



/// A queue of hook changes to be applied at once.
pub struct HookQueue(Vec<(FnPointer, bool)>);

impl HookQueue {
    /// Create a new empty queue.
    pub fn new() -> HookQueue {
        HookQueue(Vec::new())
    }

    /// Queue the given hook to be enabled.
    pub fn enable(mut self, hook: &Hook) -> HookQueue {
        self.0.push((hook.target_ptr(), true));
        self
    }

    /// Queue the given hook to be disabled.
    pub fn disable(mut self, hook: &Hook) -> HookQueue {
        self.0.push((hook.target_ptr(), false));
        self
    }

    /// Applies all the changes at once and consumes this queue.
    pub fn apply(self) -> Result<()> {
        // Requires a lock to prevent hooks queued from other threads to be applied as well.
        #[cfg(not(feature = "unstable"))]
        fn obtain_lock() -> MutexGuard<'static, ()> {
            use std::sync::Mutex;

            static mut LOCK: *const Mutex<()> = 0 as *const _;
            static     INIT: Once             = ONCE_INIT;

            unsafe {
                INIT.call_once(|| LOCK = Box::into_raw(Box::new(Mutex::new(()))));
                (*LOCK).lock().unwrap()
            }
        }

        #[cfg(feature = "unstable")]
        fn obtain_lock() -> MutexGuard<'static, ()> {
            use std::sync::{StaticMutex, MUTEX_INIT};

            static LOCK: StaticMutex = MUTEX_INIT;

            LOCK.lock().unwrap()
        }

        try!(initialize());

        unsafe {
            let _g = obtain_lock();

            for (target, enabled) in self.0 {
                // Any failure at this point is a bug.
                if enabled {
                    imp::queue_enable_hook(target).unwrap();
                } else {
                    imp::queue_disable_hook(target).unwrap();
                }
            }
            imp::apply_queued()
        }
    }
}



/// Base trait for hooks.
pub trait Hook {
    /// Returns the target function pointer.
    fn target_ptr(&self) -> FnPointer;

    /// Enables this hook.
    ///
    /// Consider using a `HookQueue` if you want to enable/disable a large amount of hooks at once.
    fn enable(&self) -> Result<()> {
        unsafe { imp::enable_hook(self.target_ptr()) }
    }

    /// Disables this hook.
    ///
    /// Consider using a `HookQueue` if you want to enable/disable a large amount of hooks at once.
    fn disable(&self) -> Result<()> {
        unsafe { imp::disable_hook(self.target_ptr()) }
    }
}



/// A hook that is destroyed when it goes out of scope.
#[derive(Debug)]
pub struct ScopedHook<T: Function> {
    target: Option<T>,
    trampoline: Option<T>
}

impl<T: Function> ScopedHook<T> {
    /// Install a new `ScopedHook` given a target function and a detour function.
    ///
    /// The hook is disabled by default.
    ///
    /// # Unsafety
    ///
    /// The target and detour function pointers should point to valid memory during the lifetime
    /// of this hook.
    ///
    /// While hooking functions with type parameters is possible it is absolutely discouraged.
    /// Due to optimizations not every concrete implementation of a parameterized function has it's own
    /// code in the resulting binary. This can lead to situations where a hook is created for more than
    /// just the the target function and the function signature of the detour function does not match up.
    pub unsafe fn install<D>(target: T, detour: D) -> Result<ScopedHook<T>>
    where T: HookableWith<D>, D: Function {
        try!(initialize());

        let trampoline = T::from_ptr(try!(imp::create_hook(target.as_ptr(), detour.as_ptr())));

        Ok(ScopedHook {
            target: Some(target),
            trampoline: Some(trampoline),
        })
    }

    /// Transforms this hook into a static hook, consuming this object.
    pub fn into_static(mut self) -> StaticHook<T> {
        StaticHook {
            target: self.target.take().unwrap(),
            trampoline: self.trampoline.take().unwrap(),
        }
    }

    /// Uninstalls and destroys this hook.
    ///
    /// This method returns whether it was succesful as opposed to `drop()`.
    pub fn destroy(mut self) -> Result<()> {
        let target = self.target.take().unwrap();

        unsafe { imp::remove_hook(target.as_ptr()) }
    }
}

impl<T: Function> Hook for ScopedHook<T> {
    fn target_ptr(&self) -> FnPointer {
        self.target.as_ref().unwrap().as_ptr()
    }
}

impl<T: Function> Drop for ScopedHook<T> {
    fn drop(&mut self) {
        self.target.as_ref().map(|target| unsafe {
            let _ = imp::remove_hook(target.as_ptr());
        });
    }
}



/// A static hook that lives during the entire life-time of the program.
///
/// This type can be used for static variables.
#[derive(Debug)]
pub struct StaticHook<T: Function> {
    target: T,
    trampoline: T
}

impl<T: Function> StaticHook<T> {
    /// Returns a reference to the trampoline function.
    pub fn trampoline(&self) -> &T {
        &self.trampoline
    }

    /// Destroys this static hook.
    ///
    /// # Unsafety
    ///
    /// This method is unsafe since any trampoline function pointers will become dangling.
    pub unsafe fn destroy(self) -> Result<()> {
        imp::remove_hook(self.target.as_ptr())
    }
}

impl<T: Function> Hook for StaticHook<T> {
    fn target_ptr(&self) -> FnPointer {
        self.target.as_ptr()
    }
}

impl<T: Function> From<ScopedHook<T>> for StaticHook<T> {
    fn from(scoped: ScopedHook<T>) -> Self {
        scoped.into_static()
    }
}



/// See [LazyStaticHook](trait.LazyStaticHook.html).
pub trait LazyStaticHookInit {
    /// Initialize and install the underlying hook.
    ///
    /// Calling this method again after a previous successful initialization is a no-op.
    fn install(&self) -> Result<()>;
}

/// A thread-safe initializer type for a lazily initialized `StaticHook`.
///
/// This type is implemented by the static variables created with the `static_hooks!` macro.
/// It is strongly recommended to call `install()` before accessing
/// the underlying `StaticHook` through derefencing. The reason
/// for this is that `install()` will return an error when
/// hook installation fails, while dereferencing will panic.
pub trait LazyStaticHook<T: Function>: Sync {
    #[doc(hidden)]
    fn __get(&self) -> Result<&StaticHook<T>>;
}

impl<T: Function> LazyStaticHookInit for LazyStaticHook<T> {
    fn install(&self) -> Result<()> {
        self.__get().map(|_|())
    }
}

impl<T: Function> ops::Deref for LazyStaticHook<T> {
    type Target = StaticHook<T>;

    fn deref(&self) -> &Self::Target {
        self.__get().expect("Lazy hook installation panicked")
    }
}



/// Declares one or more lazily initialized thread-safe static hooks.
///
/// The syntax for these hooks is:
///
/// ```text
/// pub? unsafe hook<$function_type> $hook_name($argument_name*) for $target_function {
///     statement+
/// }
/// ```
#[macro_export]
macro_rules! static_hooks {
    // Empty match to terminate recursion
    () => {};

    // Step 1: parse attributes
    // Requires explicit look-ahead to prevent local ambiguity
    ($(#[$fun_attr:meta])* pub $($rest:tt)*) => {
        static_hooks!(parse_mods: ($($fun_attr)*) | pub $($rest)*);
    };
    ($(#[$fun_attr:meta])* unsafe $($rest:tt)*) => {
        static_hooks!(parse_mods: ($($fun_attr)*) | unsafe $($rest)*);
    };

    // Step 2: parse optional pub modifier
    (parse_mods: ($($fun_attr:meta)*)
               | pub unsafe hook < $($rest:tt)*) =>
    {
        static_hooks!(parse_safety: ($($fun_attr)*), (pub) | $($rest)*);
    };
    (parse_mods: ($($fun_attr:meta)*)
               | unsafe hook < $($rest:tt)*) =>
    {
        static_hooks!(parse_safety: ($($fun_attr)*), () | $($rest)*);
    };

    // Step 3: parse optional unsafe modifier
    (parse_safety: ($($fun_attr:meta)*), ($($var_mod:tt)*)
                 | unsafe $($rest:tt)*) =>
    {
        static_hooks!(parse_linkage: ($($fun_attr)*), ($($var_mod)*), (unsafe) | $($rest)*);
    };
    (parse_safety: ($($fun_attr:meta)*), ($($var_mod:tt)*)
                 | $($rest:tt)*) =>
    {
        static_hooks!(parse_linkage: ($($fun_attr)*), ($($var_mod)*), () | $($rest)*);
    };

    // Step 4: parse optional extern modifier and linkage type
    (parse_linkage: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*)
                  | extern $linkage:tt fn $($rest:tt)*) =>
    {
        static_hooks!(parse_types: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)* extern $linkage) | $($rest)*);
    };
    (parse_linkage: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*)
                  | extern fn $($rest:tt)*) =>
    {
        static_hooks!(parse_types: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)* extern) | $($rest)*);
    };
    (parse_linkage: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*)
                  | fn $($rest:tt)*) =>
    {
        static_hooks!(parse_types: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)*) | $($rest)*);
    };

    // Step 5: parse argument types and optional return type
    (parse_types: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*)
                | ($($arg_type:ty),*) -> $return_type:ty > $($rest:tt)*) =>
    {
        static_hooks!(parse_name_and_args: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)*), ($($arg_type),*), ($return_type) | $($rest)*);
    };
    (parse_types: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*)
                | ($($arg_type:ty),*) > $($rest:tt)*) =>
    {
        static_hooks!(parse_name_and_args: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)*), ($($arg_type),*), (()) | $($rest)*);
    };

    // Step 6: parse name and argument names
    (parse_name_and_args: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*), ($($arg_type:ty),*), ($return_type:ty)
                        | $var_name:ident ($($arg_name:ident),*) for $($rest:tt)*) =>
    {
        static_hooks!(parse_target: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)*), ($($arg_type),*),
                                    $return_type, $var_name, ($($arg_name),*)
                                  | $($rest)*);
    };

    // Step 7: parse target
    (parse_target: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*), ($($arg_type:ty),*),
                   $return_type:ty, $var_name:ident, ($($arg_name:ident),*)
                 | $target:path { $($body:tt)* } $($rest:tt)*) =>
    {
        static_hooks!(parse_body: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)*), ($($arg_type),*),
                                  $return_type, $var_name, ($($arg_name),*), $target, { $($body)* }
                                | $($rest)*);
    };

    // Step 8: parse body and recurse
    (parse_body: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*), ($($arg_type:ty),*),
                 $return_type:ty, $var_name:ident, ($($arg_name:ident),*), $target:path, $body:block
               | $($rest:tt)*) =>
    {
        static_hooks!(make: ($($fun_attr)*), ($($var_mod)*), ($($fun_mod)*), ($($arg_type),*),
                            $return_type, $var_name, ($($arg_name),*), $target, $body);
        static_hooks!($($rest)*);
    };


    // Step 9: generate output
    (make: ($($fun_attr:meta)*), ($($var_mod:tt)*), ($($fun_mod:tt)*), ($($arg_type:ty),*), $return_type:ty, $var_name:ident, ($($arg_name:ident),*), $target:path, $body:block) => {
        static_hooks!(make_var:
            $var_name,
            ($($var_mod)*),
            $target,
            $($fun_mod)* fn ($($arg_type),*) -> $return_type,
            static_hooks!(make_detour: ($($fun_attr)*), ($($fun_mod)*), ($($arg_name),*), ($($arg_type),*), $return_type, $body);
        );
    };

    // Makes sure items are interpreted correctly
    (make_item: $item:item) => {
        $item
    };

    // Creates a detour function
    (make_detour: ($($fun_attr:meta)*), ($($fun_mod:tt)*), ($($arg_name:ident),*), ($($arg_type:ty),*), $return_type:ty, $body:block) => {
        static_hooks!(make_item: $(#[$fun_attr])* $($fun_mod)* fn __detour($($arg_name: $arg_type),*) -> $return_type $body);
    };

    // Creates a static hook variable and implementation
    (make_var: $var_name:ident, ($($var_mod:tt)*), $target:path, $fun_type:ty, $detour:item) => {
        static_hooks!(make_item:
            #[allow(non_upper_case_globals)] $($var_mod)* static $var_name: &'static $crate::LazyStaticHook<$fun_type> = {
                $detour

                {
                    use ::std::sync::{Once, ONCE_INIT};
                    use ::std::option::Option::{self, Some, None};
                    use ::std::result::Result::Ok;

                    use $crate::{Result, LazyStaticHook, StaticHook, ScopedHook};

                    #[allow(dead_code)]
                    struct LazyStaticHookImpl;

                    impl LazyStaticHook<$fun_type> for LazyStaticHookImpl {
                        fn __get(&self) -> Result<&StaticHook<$fun_type>> {
                            static     INIT: Once                          = ONCE_INIT;
                            static mut HOOK: Option<StaticHook<$fun_type>> = None;

                            let mut result = Ok(());

                            INIT.call_once(|| unsafe {
                                result = ScopedHook::<$fun_type>::install($target as $fun_type, __detour as $fun_type)
                                                                 .map(|hook| HOOK = Some(hook.into_static()));
                            });

                            result.map(|_| unsafe {
                                HOOK.as_ref().unwrap()
                            })
                        }
                    }

                    &LazyStaticHookImpl
                }
            };
        );
    };
}


/// An untyped function pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FnPointer(*mut c_void);

impl fmt::Pointer for FnPointer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:p}", self.0)
    }
}

/// Trait representing a function that can be used as a target function or detour function for hooking.
#[cfg_attr(feature = "unstable", rustc_on_unimplemented = "The type `{Self}` is not an eligible target function or detour function.")]
pub trait Function {
    /// Converts this function into an untyped function pointer.
    fn as_ptr(&self) -> FnPointer;

    /// Constructs a `Function` from an untyped function pointer.
    ///
    /// # Unsafety
    ///
    /// This method is unsafe because the argument should point to proper executable memory of the correct type.
    unsafe fn from_ptr(ptr: FnPointer) -> Self;
}

/// Trait representing a "hookable with" relation between a target function and a detour function.
///
/// If a target `Function` implements `HookableWith<Detour>` then `Detour` is a valid detour function for this target.
///
/// Implementing this trait requires proper understanding of the compatibility of the target and detour functions.
/// Incompatible functions can cause all kinds of undefined behaviour.
#[cfg_attr(feature = "unstable", rustc_on_unimplemented = "The type `{D}` is not a suitable detour function type for a target function of type `{Self}`.")]
pub unsafe trait HookableWith<D: Function>: Function {}



macro_rules! impl_hookable {
    ($($param:ident),*) => {
        impl_hookable!(recurse: ($($param),*) ());
    };

    (recurse: () ($($param:ident),*)) => {
        impl_hookable!(impl_all: $($param),*);
    };
    (recurse: ($head:ident $(, $tail:ident)*) ($($param:ident),*)) => {
        impl_hookable!(impl_all: $($param),*);
        impl_hookable!(recurse: ($($tail),*) ($($param,)* $head));
    };

    (impl_all: $($arg:ident),*) => {
        impl_hookable!(impl_pair: ($($arg),*) (extern "Rust"     fn($($arg),*) -> Ret));
        impl_hookable!(impl_pair: ($($arg),*) (extern "cdecl"    fn($($arg),*) -> Ret));
        impl_hookable!(impl_pair: ($($arg),*) (extern "stdcall"  fn($($arg),*) -> Ret));
        impl_hookable!(impl_pair: ($($arg),*) (extern "fastcall" fn($($arg),*) -> Ret));
        impl_hookable!(impl_pair: ($($arg),*) (extern "win64"    fn($($arg),*) -> Ret));
        impl_hookable!(impl_pair: ($($arg),*) (extern "C"        fn($($arg),*) -> Ret));
        impl_hookable!(impl_pair: ($($arg),*) (extern "system"   fn($($arg),*) -> Ret));
    };

    (impl_pair: ($($arg:ident),*) ($($fun_tokens:tt)+)) => {
        impl_hookable!(impl_safe:   ($($arg),*) (       $($fun_tokens)*));
        impl_hookable!(impl_unsafe: ($($arg),*) (unsafe $($fun_tokens)*));
    };

    (impl_safe: ($($arg:ident),*) ($fun_type:ty)) => {
        impl<Ret: 'static, $($arg: 'static),*> ScopedHook<$fun_type> {
            /// Call the original function.
            #[inline]
            #[allow(non_snake_case)]
            pub fn call_real(&self, $($arg : $arg),*) -> Ret {
                self.trampoline.unwrap()($($arg),*)
            }
        }

        impl<Ret: 'static, $($arg: 'static),*> StaticHook<$fun_type> {
            /// Call the original function.
            #[inline]
            #[allow(non_snake_case)]
            pub fn call_real(&self, $($arg : $arg),*) -> Ret {
                (self.trampoline)($($arg),*)
            }
        }

        impl_hookable!(impl_fun: ($($arg),*) ($fun_type));
    };

    (impl_unsafe: ($($arg:ident),*) ($fun_type:ty)) => {
        impl<Ret: 'static, $($arg: 'static),*> ScopedHook<$fun_type> {
            /// Call the original function.
            #[inline]
            #[allow(non_snake_case)]
            pub unsafe fn call_real(&self, $($arg : $arg),*) -> Ret {
                self.trampoline.unwrap()($($arg),*)
            }
        }

        impl<Ret: 'static, $($arg: 'static),*> StaticHook<$fun_type> {
            /// Call the original function.
            #[inline]
            #[allow(non_snake_case)]
            pub unsafe fn call_real(&self, $($arg : $arg),*) -> Ret {
                (self.trampoline)($($arg),*)
            }
        }

        impl_hookable!(impl_fun: ($($arg),*) ($fun_type));
    };

    (impl_fun: ($($arg:ident),*) ($fun_type:ty)) => {
        impl<Ret: 'static, $($arg: 'static),*> Function for $fun_type {
            #[inline]
            fn as_ptr(&self) -> FnPointer {
                FnPointer(*self as *mut _)
            }

            #[inline]
            unsafe fn from_ptr(FnPointer(ptr): FnPointer) -> Self {
                mem::transmute(ptr)
            }
        }

        unsafe impl<Ret: 'static, $($arg: 'static),*> HookableWith<$fun_type> for $fun_type {}
    };

    (make_item: $item:item) => {
        $item
    };
}

#[cfg(not(feature = "increased_arity"))]
impl_hookable!(A, B, C, D, E, F, G, H, I, J, K, L);

#[cfg(feature = "increased_arity")]
impl_hookable!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);



#[cfg(test)]
mod tests;
