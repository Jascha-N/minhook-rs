//! # The minhook-rs library
//! This library provides function hooking support to Rust by providing a
//! Rust wrapper around the [MinHook][minhook] library.
//!
//! [minhook]: http://www.codeproject.com/KB/winsdk/LibMinHook.aspx
#![cfg_attr(feature = "nightly", feature(on_unimplemented))]
#![warn(missing_docs)]

pub mod ffi;

use std::{error, fmt, mem, ops, result};
use std::sync::{ONCE_INIT, Once};
use std::os::raw::c_void;

use ffi::MH_STATUS;



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
    pub unsafe fn create_hook(target: FnPointer, detour: FnPointer) -> Result<FnPointer> {
        let mut trampoline: *mut c_void = mem::uninitialized();

        status_to_result(MH_CreateHook(target.as_raw_mut(), detour.as_raw_mut(), &mut trampoline as *mut _))
            .map(|_| FnPointer::from_raw(trampoline))
    }

    #[inline]
    pub unsafe fn remove_hook(target: FnPointer) -> Result<()> {
        status_to_result(MH_RemoveHook(target.as_raw_mut()))
    }

    #[inline]
    pub unsafe fn enable_hook(target: FnPointer) -> Result<()> {
        status_to_result(MH_EnableHook(target.as_raw_mut()))
    }

    #[inline]
    pub unsafe fn disable_hook(target: FnPointer) -> Result<()> {
        status_to_result(MH_DisableHook(target.as_raw_mut()))
    }

    #[inline]
    pub unsafe fn queue_enable_hook(target: FnPointer) -> Result<()> {
        status_to_result(MH_QueueEnableHook(target.as_raw_mut()))
    }

    #[inline]
    pub unsafe fn queue_disable_hook(target: FnPointer) -> Result<()> {
        status_to_result(MH_QueueDisableHook(target.as_raw_mut()))
    }

    #[inline]
    pub unsafe fn queue_enable_hook_all() -> Result<()> {
        status_to_result(MH_QueueEnableHook(MH_ALL_HOOKS))
    }

    #[inline]
    pub unsafe fn queue_disable_hook_all() -> Result<()> {
        status_to_result(MH_QueueDisableHook(MH_ALL_HOOKS))
    }

    #[inline]
    pub unsafe fn apply_queued() -> Result<()> {
        status_to_result(MH_ApplyQueued())
    }
}



/// The error type for all hooking operations.
///
/// MinHook error status codes map directly to this type.
#[derive(Debug)]
pub enum Error {
    /// MinHook is already initialized.
    AlreadyInitialized,
    /// MinHook is not initialized yet, or already uninitialized.
    NotInitialized,
    /// The hook for the specified target function is already created.
    AlreadyCreated,
    /// The hook for the specified target function is not created yet.
    NotCreated,
    /// The hook for the specified target function is already enabled.
    AlreadyEnabled,
    /// The hook for the specified target function is not enabled yet, or
    /// already disabled.
    Disabled,
    /// The specified pointer is invalid. It points the address of non-allocated
    /// and/or non-executable region.
    NotExecutable,
    /// The specified target function cannot be hooked.
    UnsupportedFunction,
    /// Failed to allocate memory.
    MemoryAlloc,
    /// Failed to change the memory protection.
    MemoryProtect,
    /// The specified module is not loaded.
    ModuleNotFound,
    /// The specified function is not found.
    FunctionNotFound,

    /// The specified module name is invalid.
    InvalidModuleName,
    /// The specified function name is invalid.
    InvalidFunctionName
}

impl Error {
    fn from(status: MH_STATUS) -> Option<Error> {
        match status {
            MH_STATUS::MH_OK => None,
            MH_STATUS::MH_ERROR_ALREADY_INITIALIZED => Some(Error::AlreadyInitialized),
            MH_STATUS::MH_ERROR_NOT_INITIALIZED => Some(Error::NotInitialized),
            MH_STATUS::MH_ERROR_ALREADY_CREATED => Some(Error::AlreadyCreated),
            MH_STATUS::MH_ERROR_NOT_CREATED => Some(Error::NotCreated),
            MH_STATUS::MH_ERROR_ENABLED => Some(Error::AlreadyEnabled),
            MH_STATUS::MH_ERROR_DISABLED => Some(Error::Disabled),
            MH_STATUS::MH_ERROR_NOT_EXECUTABLE => Some(Error::NotExecutable),
            MH_STATUS::MH_ERROR_UNSUPPORTED_FUNCTION => Some(Error::UnsupportedFunction),
            MH_STATUS::MH_ERROR_MEMORY_ALLOC => Some(Error::MemoryAlloc),
            MH_STATUS::MH_ERROR_MEMORY_PROTECT => Some(Error::MemoryProtect),
            MH_STATUS::MH_ERROR_MODULE_NOT_FOUND => Some(Error::ModuleNotFound),
            MH_STATUS::MH_ERROR_FUNCTION_NOT_FOUND => Some(Error::FunctionNotFound),
            MH_STATUS::MH_UNKNOWN => unreachable!(),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::AlreadyInitialized => "library already initialized",
            Error::NotInitialized => "library not initialized",
            Error::AlreadyCreated => "hook already created",
            Error::NotCreated => "hook not created",
            Error::AlreadyEnabled => "hook already enabled",
            Error::Disabled => "hook not enabled",
            Error::NotExecutable => "invalid pointer",
            Error::UnsupportedFunction => "function cannot be hooked",
            Error::MemoryAlloc => "failed to allocate memory",
            Error::MemoryProtect => "failed to change the memory protection",
            Error::ModuleNotFound => "module not loaded",
            Error::FunctionNotFound => "function not found",

            Error::InvalidModuleName => "invalid module name",
            Error::InvalidFunctionName => "invalid function name",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let message = match *self {
            Error::AlreadyInitialized => "MinHook is already initialized.",
            Error::NotInitialized => "MinHook is not initialized yet, or already uninitialized.",
            Error::AlreadyCreated => "The hook for the specified target function is already \
                                      created.",
            Error::NotCreated => "The hook for the specified target function is not created yet.",
            Error::AlreadyEnabled => "The hook for the specified target function is already \
                                      enabled.",
            Error::Disabled => "The hook for the specified target function is not enabled yet, or \
                                already disabled.",
            Error::NotExecutable => "The specified pointer is invalid. It points the address of \
                                     non-allocated and/or non-executable region.",
            Error::UnsupportedFunction => "The specified target function cannot be hooked.",
            Error::MemoryAlloc => "Failed to allocate memory.",
            Error::MemoryProtect => "Failed to change the memory protection.",
            Error::ModuleNotFound => "The specified module is not loaded.",
            Error::FunctionNotFound => "The specified function is not found.",

            Error::InvalidModuleName => "The specified module name is invalid.",
            Error::InvalidFunctionName => "The specified function name is invalid.",
        };

        write!(fmt, "{:?} error: {}", self, message)
    }
}

/// Result type for all functions and methods in this module.
pub type Result<T> = result::Result<T, Error>;



/// Initializes the minhook-rs library.
///
/// It is not required to call this function directly as the other library functions will do it internally.
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
/// its usage.
pub unsafe fn uninitialize() -> Result<()> {
    imp::uninitialize()
}

/// Applies all queued hook changes at once.
pub fn apply_queued_hooks() -> Result<()> {
    try!(initialize());

    unsafe { imp::apply_queued() }
}

/// Enables all hooks at once.
pub fn enable_all_hooks() -> Result<()> {
    try!(initialize());

    unsafe {
        try!(imp::queue_enable_hook_all());
        imp::apply_queued()
    }
}

/// Disables all hooks at once.
pub fn disable_all_hooks() -> Result<()> {
    try!(initialize());

    unsafe {
        try!(imp::queue_disable_hook_all());
        imp::apply_queued()
    }
}



/// Base trait for hooks.
pub trait Hook<T: Function> {
    /// Returns a reference to the hook target function.
    fn target(&self) -> &T;

    /// Enables or disables this hook.
    ///
    /// Consider using `queue_enabled()` with `apply_queued_hooks()` or `apply_hooks!` if you
    /// want to enable/disable a large amount of hooks at once.
    fn set_enabled(&self, enabled: bool) -> Result<()> {
        unsafe {
            if enabled {
                imp::enable_hook(self.target().as_ptr())
            } else {
                imp::disable_hook(self.target().as_ptr())
            }
        }
    }

    /// Queues this hook for enabling or disabling.
    ///
    /// Use `apply_queued_hooks()` to apply the queued hooks.
    fn queue_enabled(&self, enabled: bool) -> Result<()> {
        unsafe {
            if enabled {
                imp::queue_enable_hook(self.target().as_ptr())
            } else {
                imp::queue_disable_hook(self.target().as_ptr())
            }
        }
    }
}



/// A temporary hook that is destroyed when it goes out of scope.
#[derive(Debug)]
pub struct LocalHook<T: Function> {
    target: Option<T>,
    trampoline: Option<T>
}

impl<T: Function> LocalHook<T> {
    /// Creates a new temporary hook given a target function and a detour function.
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
    pub unsafe fn new<D>(target: T, detour: D) -> Result<LocalHook<T>>
    where T: HookableWith<D>, D: Function {
        try!(initialize());

        let trampoline = T::from_ptr(try!(imp::create_hook(target.as_ptr(), detour.as_ptr())));

        Ok(LocalHook {
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

    /// Safely destroys this hook.
    ///
    /// This method returns whether it was succesful as opposed to `drop()`.
    pub fn destroy(mut self) -> Result<()> {
        let target = self.target.take().unwrap();

        unsafe { imp::remove_hook(target.as_ptr()) }
    }
}

impl<T: Function> Hook<T> for LocalHook<T> {
    fn target(&self) -> &T {
        self.target.as_ref().unwrap()
    }
}

impl<T: Function> Drop for LocalHook<T> {
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
    /// This method is unsafe since any trampoline function pointers will become dangling after destroying the hook.
    pub unsafe fn destroy(self) -> Result<()> {
        imp::remove_hook(self.target.as_ptr())
    }
}

impl<T: Function> Hook<T> for StaticHook<T> {
    fn target(&self) -> &T {
        &self.target
    }
}

impl<T: Function> From<LocalHook<T>> for StaticHook<T> {
    fn from(local_hook: LocalHook<T>) -> Self {
        local_hook.into_static()
    }
}



/// A thread-safe initializer type for a lazily initialized `StaticHook`.
///
/// This type is implemented by the static variables created with the `static_hooks!` macro.
/// It is strongly recommended to call `initialize()` before accessing
/// the underlying `StaticHook` through derefencing. The reason
/// for this is that `initialize()` will return a wrapped `Error` when
/// initialization fails, while dereferencing will panic.
pub trait LazyStaticHook<T: Function>: Sync {
    /// Initialize and install the underlying hook and return a reference to it.
    ///
    /// Calling this method again after a previous successful initialization is a no-op.
    fn initialize(&self) -> Result<&StaticHook<T>>;
}

impl<T: Function> ops::Deref for LazyStaticHook<T> {
    type Target = StaticHook<T>;

    fn deref(&self) -> &Self::Target {
        self.initialize().expect("Static hook creation panicked")
    }
}



/// Initializes a list of static hooks.
#[macro_export]
macro_rules! init_static_hooks {
    ($head:path) => {
        $head.initialize()
    };

    ($head:path, $($tail:path),*) => {
        $head.initialize().and_then(|_| init_static_hooks!($($tail),*))
    };
}

/// Enables or disables a list of hooks all at once.
///
/// Hooks prefixed with `-` are disabled instead of enabled.
#[macro_export]
macro_rules! apply_hooks {
    () => {
        $crate::apply_queued_hooks()
    };

    ($head:path) => {
        $head.queue_enabled(true).and_then(|_| apply_hooks!())
    };
    (-$head:path) => {
        $head.queue_enabled(false).and_then(|_| apply_hooks!())
    };

    ($head:path, $($tail:tt)*) => {
        $head.queue_enabled(true).and_then(|_| apply_hooks!($($tail)*))
    };
    (-$head:path, $($tail:tt)*) => {
        $head.queue_enabled(false).and_then(|_| apply_hooks!($($tail)*))
    };
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

                    use $crate::{Result, LazyStaticHook, StaticHook, LocalHook};

                    #[allow(dead_code)]
                    struct LazyStaticHookImpl;

                    impl LazyStaticHook<$fun_type> for LazyStaticHookImpl {
                        fn initialize(&self) -> Result<&StaticHook<$fun_type>> {
                            static     INIT: Once                          = ONCE_INIT;
                            static mut HOOK: Option<StaticHook<$fun_type>> = None;

                            let mut result = Ok(());

                            INIT.call_once(|| unsafe {
                                result = LocalHook::<$fun_type>::new($target as $fun_type, __detour as $fun_type)
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
pub struct FnPointer(*const c_void);

impl FnPointer {
    #[inline(always)]
    fn from_raw(ptr: *const c_void) -> FnPointer {
        FnPointer(ptr)
    }

    #[inline(always)]
    fn as_raw(&self) -> *const c_void {
        self.0
    }

    #[inline(always)]
    fn as_raw_mut(&self) -> *mut c_void {
        self.0 as *mut _
    }
}

/// Trait representing a function that can be used as a target function or detour function for hooking.
#[cfg_attr(feature = "nightly", rustc_on_unimplemented = "The type `{Self}` is not an eligible target function or detour function.")]
pub trait Function {
    /// Converts this function into an untyped function pointer.
    fn as_ptr(&self) -> FnPointer;

    /// Constructs a `Function` from a raw pointer.
    ///
    /// # Unsafety
    ///
    /// This method is unsafe because the argument should point to proper executable memory.
    unsafe fn from_ptr(ptr: FnPointer) -> Self;
}

/// Trait representing a "hookable with" relation between a target function and a detour function.
///
/// If a target `Function` implements `HookableWith<Detour>` then `Detour` is a valid detour function for this target.
///
/// Implementing this trait requires proper understanding of the compatibility of the target and detour functions.
/// Incompatible functions can cause all kinds of undefined behaviour.
#[cfg_attr(feature = "nightly", rustc_on_unimplemented = "The type `{D}` is not a suitable detour function type for a target function of type `{Self}`.")]
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
        impl<Ret: 'static, $($arg: 'static),*> LocalHook<$fun_type> {
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
        impl<Ret: 'static, $($arg: 'static),*> LocalHook<$fun_type> {
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
            #[inline(always)]
            fn as_ptr(&self) -> FnPointer {
                FnPointer(*self as *mut _)
            }

            #[inline(always)]
            unsafe fn from_ptr(ptr: FnPointer) -> Self {
                mem::transmute(ptr.as_raw())
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
