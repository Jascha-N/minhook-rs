//! # The minhook-rs library
//! This library provides function hooking support to Rust by providing a
//! wrapper around the [MinHook][minhook] library.
//!
//! [minhook]: http://www.codeproject.com/KB/winsdk/LibMinHook.aspx
#![feature(on_unimplemented,
           static_recursion,
           static_mutex,
           static_rwlock,
           const_fn,
           std_panic,
           recover,
           associated_consts,
           unboxed_closures,
           core_intrinsics)]
#![warn(missing_docs)]

extern crate winapi;
extern crate kernel32;

use std::{mem, ptr, ops, result};
use std::error::Error as StdError;
use std::sync::{Once, StaticMutex};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use cell::Error as CellError;
use function::{Function, FnPointer, HookableWith};

pub use cell::StaticInitCell;
pub use error::Error;

mod cell;
mod error;
mod ffi;

pub mod function;
pub mod panic;



/// Result type for most functions and methods in this module.
pub type Result<T> = result::Result<T, Error>;

/// Uninitializes the minhook-rs library.
///
/// # Safety
///
/// This function is unsafe because any live hooks might still depend on this library. After
/// calling this function existing trampoline functions might point to uninitialized memory.
/// Only use this function when you are absolutely sure no hook objects will be accessed after
/// its use.
pub unsafe fn uninitialize() -> Result<()> {
    s2r(ffi::MH_Uninitialize())
}



/// A queue of hook changes to be applied at once.
pub struct HookQueue(Vec<(FnPointer, bool)>);

impl HookQueue {
    /// Create a new empty queue.
    pub fn new() -> HookQueue {
        HookQueue(Vec::new())
    }

    /// Queue the given hook to be enabled.
    pub fn enable<T: Function>(&mut self, hook: &Hook<T>) -> &mut HookQueue {
        self.0.push((hook.target, true));
        self
    }

    /// Queue the given hook to be disabled.
    pub fn disable<T: Function>(&mut self, hook: &Hook<T>) -> &mut HookQueue {
        self.0.push((hook.target, false));
        self
    }

    /// Applies all the changes in this queue at once.
    pub fn apply(&mut self) -> Result<()> {
        initialize();

        static LOCK: StaticMutex = StaticMutex::new();
        let _lock = LOCK.lock().unwrap();

        for &(target, enabled) in &*self.0 {
            // Any failure at this point is a bug.
            if enabled {
                unsafe { s2r(ffi::MH_QueueEnableHook(target.to_raw())).unwrap() };
            } else {
                unsafe { s2r(ffi::MH_QueueDisableHook(target.to_raw())).unwrap() };
            }
        }
        unsafe { s2r(ffi::MH_ApplyQueued()) }
    }
}



/// A hook that is destroyed when it goes out of scope.
#[derive(Debug)]
pub struct Hook<T: Function> {
    target: FnPointer,
    trampoline: T
}

/// A function name used for dynamically hooking a function.
#[derive(Debug)]
pub enum FunctionName<S: AsRef<OsStr>> {
    /// The function's ordinal value.
    Ordinal(u16),
    /// The function's name.
    String(S)
}

impl<T: Function> Hook<T> {
    /// Create a new hook given a target function and a compatible detour function.
    ///
    /// The hook is disabled by default. Even when this function is succesful, there is no
    /// guaranteee that the detour function will actually get called when the target function gets
    /// called. An invocation of the target function might for example get inlined in which case
    /// it is impossible to hook at runtime.
    ///
    /// # Safety
    ///
    /// The given target function type must uniquely match the actual target function. This
    /// means two things: the given target function type has to be correct, but also there
    /// can not be two function pointers with different signatures pointing to the same
    /// code location. This last situation can for example happen when the Rust compiler
    /// or LLVM decide to merge multiple functions with the same code into one.
    pub unsafe fn create<D>(target: T, detour: D) -> Result<Hook<T>>
    where T: HookableWith<D>, D: Function {
        initialize();

        let target = target.to_ptr();
        let detour = detour.to_ptr();
        let mut trampoline = mem::uninitialized();
        try!(s2r(ffi::MH_CreateHook(target.to_raw(), detour.to_raw(), &mut trampoline)));

        Ok(Hook {
            target: target,
            trampoline: T::from_ptr(FnPointer::from_raw(trampoline)),
        })
    }

    /// Create a new hook given the name of the module, the name of the function symbol and a
    /// compatible detour function.
    ///
    /// The module has to be loaded before this function is called. This function does not
    /// attempt to load the module first. The hook is disabled by default.
    ///
    /// # Safety
    ///
    /// The target module must remain loaded in memory for the entire duration of the hook.
    ///
    /// See `create()` for more safety requirements.
    pub unsafe fn create_api<M, N, D>(target_module: M, target_function: FunctionName<N>, detour: D) -> Result<Hook<T>>
    where M: AsRef<OsStr>, N: AsRef<OsStr>, T: HookableWith<D>, D: Function {
        fn str_to_wstring(string: &OsStr) -> Option<Vec<winapi::WCHAR>> {
            let mut wide = string.encode_wide().collect::<Vec<_>>();
            if wide.contains(&0) {
                return None;
            }
            wide.push(0);
            Some(wide)
        }

        initialize();

        let module_name = try!(str_to_wstring(target_module.as_ref()).ok_or(Error::InvalidModuleName));

        let (function_name, _data) = match target_function {
            FunctionName::Ordinal(ord) => (ord as winapi::LPCSTR, Vec::new()),
            FunctionName::String(name) => {
                let symbol_name_wide = try!(str_to_wstring(name.as_ref()).ok_or(Error::InvalidFunctionName));

                let size = kernel32::WideCharToMultiByte(winapi::CP_ACP, 0, symbol_name_wide.as_ptr(), -1, ptr::null_mut(), 0, ptr::null(), ptr::null_mut());
                if size == 0 {
                    return Err(Error::InvalidFunctionName);
                }

                let mut buffer = Vec::with_capacity(size as usize);
                buffer.set_len(size as usize);

                let size = kernel32::WideCharToMultiByte(winapi::CP_ACP, 0, symbol_name_wide.as_ptr(), -1, buffer.as_mut_ptr(), size, ptr::null(), ptr::null_mut());
                if size == 0 {
                    return Err(Error::InvalidFunctionName);
                }

                (buffer.as_ptr(), buffer)
            }
        };

        let detour = detour.to_ptr();
        let mut trampoline = mem::uninitialized();
        let mut target = mem::uninitialized();

        try!(s2r(ffi::MH_CreateHookApiEx(module_name.as_ptr(), function_name, detour.to_raw(), &mut trampoline, &mut target)));

        Ok(Hook {
            target: FnPointer::from_raw(target),
            trampoline: T::from_ptr(FnPointer::from_raw(trampoline)),
        })
    }

    /// Returns an unsafe reference to the trampoline function.
    ///
    /// Calling the returned function is always unsafe as it will point to invalid memory after the
    /// hook is destroyed.
    pub fn trampoline(&self) -> T::Unsafe {
        self.trampoline.to_unsafe()
    }

    /// Enables this hook.
    ///
    /// Consider using a `HookQueue` if you want to enable/disable a large amount of hooks at once.
    pub fn enable(&self) -> Result<()> {
        unsafe { s2r(ffi::MH_EnableHook(self.target.to_raw())) }
    }

    /// Disables this hook.
    ///
    /// Consider using a `HookQueue` if you want to enable/disable a large amount of hooks at once.
    pub fn disable(&self) -> Result<()> {
        unsafe { s2r(ffi::MH_DisableHook(self.target.to_raw())) }
    }
}

impl<T: Function> Drop for Hook<T> {
    fn drop(&mut self) {
        let _ = unsafe { s2r(ffi::MH_RemoveHook(self.target.to_raw())) };
    }
}

// Synchronization is done in the MinHook library.
unsafe impl<T: Function> Sync for Hook<T> {}
unsafe impl<T: Function> Send for Hook<T> {}



/// A hook with a static lifetime.
///
/// This hook can only be constructed using the `static_hooks!` macro. It has one of the
/// following forms:
///
/// ```ignore
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE;
/// #[ATTR]* pub? impl HOOK_VAR_NAME for "FUNCTION" in "MODULE": FN_TYPE;
/// ```
///
/// Before accessing this hook it is **required** to call `initialize()` **once**. Accessing the
/// hook before initializing or trying to initialize the hook twice (even after the first attempt
/// failed) will result in a panic.
pub struct StaticHook<T: Function> {
    hook: &'static StaticInitCell<__StaticHookInner<T>>,
    target: __StaticHookTarget<T>,
    detour: T
}

impl<T: Function> StaticHook<T> {
    #[doc(hidden)]
    pub const fn __new(hook: &'static StaticInitCell<__StaticHookInner<T>>, target: __StaticHookTarget<T>, detour: T) -> StaticHook<T> {
        StaticHook {
            hook: hook,
            target: target,
            detour: detour
        }
    }

    /// Returns a reference to the trampoline function.
    pub fn trampoline(&self) -> T {
        self.inner().trampoline
    }

    unsafe fn initialize_ref(&self, closure: &'static (Fn<T::Args, Output = T::Output> + Sync)) -> Result<()> {
        let hook = match self.target {
            __StaticHookTarget::Static(target) => try!(Hook::create(target, self.detour)),
            __StaticHookTarget::Dynamic(module_name, function_name) =>
                try!(Hook::create_api(module_name, FunctionName::String(function_name), self.detour))
        };

        self.hook.initialize(__StaticHookInner(hook, closure)).map_err(|error| match error {
            CellError::AlreadyInitialized => Error::AlreadyCreated,
            CellError::AccessedBeforeInitialization => panic!("attempt to initialize static hook that was already accessed")
        })
    }

    unsafe fn initialize_box(&self, closure: Box<Fn<T::Args, Output = T::Output> + Sync>) -> Result<()> {
        try!(self.initialize_ref(&*(&*closure as *const _)));
        mem::forget(closure);
        Ok(())
    }

    /// Initialize and install the underlying hook using a detour closure.
    ///
    /// # Panics
    ///
    /// Panics if the hook was accessed or initialized before this call.
    ///
    /// # Safety
    ///
    /// See documentation for [`Hook::create()`](struct.Hook.html#method.create) and
    /// [`Hook::create_api()`](struct.Hook.html#method.create_api)
    pub unsafe fn initialize<F>(&self, closure: F) -> Result<()>
    where F: Fn<T::Args, Output = T::Output> + Sync + 'static {
        self.initialize_box(Box::new(closure))
    }

    fn inner(&self) -> &'static Hook<T> {
        let &__StaticHookInner(ref hook, _) = self.hook.get().expect("attempt to access uninitialized static hook");
        hook
    }
}

impl<T: Function> ops::Deref for StaticHook<T> {
    type Target = Hook<T>;

    fn deref(&self) -> &Hook<T> {
        self.inner()
    }
}



/// A hook with a static lifetime and a default detour closure.
///
/// This hook can only be constructed using the `static_hooks!` macro. It has one of the
/// following forms:
///
/// ```ignore
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE = CLOSURE_EXPR;
/// #[ATTR]* pub? impl HOOK_VAR_NAME for "FUNCTION" in "MODULE": FN_TYPE = CLOSURE_EXPR;
/// ```
///
/// Before accessing this hook it is **required** to call `initialize()` **once**. Accessing the
/// hook before initializing or trying to initialize the hook twice (even after the first attempt
/// failed) will result in a panic.
pub struct StaticHookWithDefault<T: Function> {
    inner: StaticHook<T>,
    default: &'static (Fn<T::Args, Output = T::Output> + Sync),
}

impl<T: Function> StaticHookWithDefault<T> {
    #[doc(hidden)]
    pub const fn __new(hook: StaticHook<T>, default: &'static (Fn<T::Args, Output = T::Output> + Sync)) -> StaticHookWithDefault<T> {
        StaticHookWithDefault {
            inner: hook,
            default: default
        }
    }

    /// Initialize and install the underlying hook.
    ///
    /// # Panics
    ///
    /// Panics if the hook was accessed or initialized before this call.
    ///
    /// # Safety
    ///
    /// See documentation for [`Hook::create()`](struct.Hook.html#method.create) and
    /// [`Hook::create_api()`](struct.Hook.html#method.create_api)
    pub unsafe fn initialize(&self) -> Result<()> {
        self.inner.initialize_ref(self.default)
    }
}

impl<T: Function> ops::Deref for StaticHookWithDefault<T> {
    type Target = StaticHook<T>;

    fn deref(&self) -> &StaticHook<T> {
        &self.inner
    }
}



static INIT: Once = Once::new();

fn initialize() {
    INIT.call_once(|| unsafe {
        let _ = s2r(ffi::MH_Initialize()).map_err(|error| {
            panic!("initialization failed with error: {}", error.description());
        });
    });
}

fn s2r(status: ffi::MH_STATUS) -> Result<()> {
    Error::from_status(status).map_or(Ok(()), Err)
}



#[doc(hidden)]
pub struct __StaticHookInner<T: Function>(Hook<T>, pub &'static (Fn<T::Args, Output = T::Output> + Sync));

#[doc(hidden)]
pub enum __StaticHookTarget<T: Function> {
    Static(T),
    Dynamic(&'static str, &'static str)
}



/// Defines one or more static hooks.
///
/// A `static_hooks!` block can contain one or more hook definitions of the following forms:
///
/// ```ignore
/// // Creates a `StaticHookWithDefault`
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE = FN_EXPR;
/// #[ATTR]* pub? impl HOOK_VAR_NAME for "FUNCTION" in "MODULE": FN_TYPE = FN_EXPR;
///
/// // Creates a `StaticHook`
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE;
/// #[ATTR]* pub? impl HOOK_VAR_NAME for "FUNCTION" in "MODULE": FN_TYPE;
/// ```
///
/// All of the above definitions create a static variable with the specified name of
/// type `StaticHook` or `StaticHookWithDefault` for a target function of the given
/// type. If the function signature contains `extern`, any panics that happen inside of the
/// detour `Fn` are automatically caught before they can propagate across foreign code boundaries.
/// See the `panic` submodule for more information.
///
/// The first two forms create a static hook with a default detour `Fn`. This is useful if
/// the detour `Fn` is a closure that does not need to capture any local variables
/// or if the detour `Fn` is just a normal function. See `StaticHookWithDefault`.
///
/// The last two forms require a `Fn` to be supplied at the time of initialization of the
/// static hook. In this case a closure that captures local variables can be supplied.
/// See `StaticHook`.
///
/// The first and third forms are used for hooking functions by their compile-time identifier.
///
/// The second and fourth form will try to find the target function by name at initialization
/// instead of at compile time. These forms require the exported function symbol name and
/// its containing module's name to be supplied.
///
/// The optional `pub` keyword can be used to give the resulting hook variable public
/// visibility. Any attributes used on a hook definition will be applied to the resulting
/// hook variable.
#[macro_export]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! static_hooks {
    // Step 1: parse attributes
    (parse_attr: ($($args:tt)*)
               | $(#[$var_attr:meta])* $next:tt $($rest:tt)*) => {
        static_hooks!(parse_pub: ($($args)* ($($var_attr)*)) | $next $($rest)*);
    };

    // Step 2: parse optional pub modifier
    (parse_pub: ($($args:tt)*)
              | pub impl $($rest:tt)*) =>
    {
        static_hooks!(parse_mod: ($($args)* (pub)) | $($rest)*);
    };
    (parse_pub: ($($args:tt)*)
              | impl $($rest:tt)*) =>
    {
        static_hooks!(parse_mod: ($($args)* ()) | $($rest)*);
    };

    // Step 3: parse optional mut or const modifier
    // (parse_mod: ($($args:tt)*)
    //           | mut $($rest:tt)*) =>
    // {
    //     static_hooks!(parse_name_target: ($($args)* (mut)) | $($rest)*);
    // };
    // (parse_mod: ($($args:tt)*)
    //           | const $($rest:tt)*) =>
    // {
    //     static_hooks!(parse_name_target: ($($args)* (const)) | $($rest)*);
    // };
    (parse_mod: ($($args:tt)*)
              | $($rest:tt)*) =>
    {
        static_hooks!(parse_name_target: ($($args)* ()) | $($rest)*);
    };

    // Step 4: parse name and target
    (parse_name_target: ($($args:tt)*)
                      | $var_name:ident for $target_fn_name:tt in $target_mod_name:tt : $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_unsafe: ($($args)* ($var_name) ($crate::__StaticHookTarget::Dynamic($target_mod_name, $target_fn_name))) | $($rest)*);
    };
    (parse_name_target: ($($args:tt)*)
                      | $var_name:ident for $target_path:path : $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_unsafe: ($($args)* ($var_name) ($crate::__StaticHookTarget::Static($target_path))) | $($rest)*);
    };

    // Step 5a: parse optional unsafe modifier
    (parse_fn_unsafe: ($($args:tt)*)
                    | unsafe $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_linkage: ($($args)*) (unsafe) | $($rest)*);
    };
    (parse_fn_unsafe: ($($args:tt)*)
                    | $($rest:tt)*) => {
        static_hooks!(parse_fn_linkage: ($($args)*) () | $($rest)*);
    };

    // Step 5b: parse linkage
    (parse_fn_linkage: ($($args:tt)*) ($($fn_mod:tt)*)
                     | extern $linkage:tt fn $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_args: ($($args)* ($($fn_mod)* extern $linkage) (GUARD)) | $($rest)*);
    };
    (parse_fn_linkage: ($($args:tt)*) ($($fn_mod:tt)*)
                     | extern fn $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_args: ($($args)* ($($fn_mod)* extern) (GUARD)) | $($rest)*);
    };
    (parse_fn_linkage: ($($args:tt)*) ($($fn_mod:tt)*)
                     | fn $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_args: ($($args)* ($($fn_mod)*) (NO_GUARD)) | $($rest)*);
    };

    // Step 5c: parse argument types and return type
    // Requires explicit look-ahead to satisfy rule for tokens following ty fragment specifier
    (parse_fn_args: ($($args:tt)*)
                  | ($($arg_type:ty),*) -> $return_type:ty = $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_value: ($($args)* ($($arg_type)*) ($return_type)) | = $($rest)*);
    };
    (parse_fn_args: ($($args:tt)*)
                  | ($($arg_type:ty),*) -> $return_type:ty ; $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_value: ($($args)* ($($arg_type)*) ($return_type)) | ; $($rest)*);
    };

    (parse_fn_args: ($($args:tt)*)
                  | ($($arg_type:ty),*) $($rest:tt)*) =>
    {
        static_hooks!(parse_fn_value: ($($args)* ($($arg_type)*) (())) | $($rest)*);
    };

    // Step 6: parse argument types and return type
    // Requires explicit look-ahead to satisfy rule for tokens following ty fragment specifier
    (parse_fn_value: ($($args:tt)*)
                   | = $value:expr ; $($rest:tt)*) =>
    {
        static_hooks!(parse_rest: ($($args)* ($value)) | $($rest)*);
    };
    (parse_fn_value: ($($args:tt)*)
                   | ; $($rest:tt)*) =>
    {
        static_hooks!(parse_rest: ($($args)* (!)) | $($rest)*);
    };

    // Step 6: parse rest and recurse
    (parse_rest: ($($args:tt)*)
               | $($rest:tt)+) =>
    {
        static_hooks!(make: $($args)*);
        static_hooks!($($rest)*);
    };
    (parse_rest: ($($args:tt)*)
               | ) =>
    {
        static_hooks!(make: $($args)*);
    };

    // Step 7: parse rest and recurse
    (make: ($($var_attr:meta)*) ($($var_mod:tt)*) ($($hook_mod:tt)*) ($var_name:ident) ($target:expr)
           ($($fn_mod:tt)*) ($guard:tt) ($($arg_type:ty)*) ($return_type:ty) ($value:tt)) =>
    {
        static_hooks!(gen_arg_names: (make_hook_var)
                                     (
                                         ($($var_attr)*) ($($var_mod)*) ($($hook_mod)*) ($var_name) ($target)
                                         ($($fn_mod)*) ($guard) ($($arg_type)*) ($return_type) ($value)
                                         ($($fn_mod)* fn ($($arg_type),*) -> $return_type)
                                     )
                                     ($($arg_type)*));
    };

    (make_hook_var: ($($arg_name:ident)*) ($($var_attr:meta)*) ($($var_mod:tt)*) ($($hook_mod:tt)*)
                    ($var_name:ident) ($target:expr) ($($fn_mod:tt)*) ($guard:tt)
                    ($($arg_type:ty)*) ($return_type:ty) (!) ($fn_type:ty)) =>
    {
        static_hooks! { make_item:
            #[allow(non_upper_case_globals)]
            $(#[$var_attr])*
            $($var_mod)* static $var_name: $crate::StaticHook<$fn_type> = {
                static __DATA: $crate::StaticInitCell<$crate::__StaticHookInner<$fn_type>> = $crate::StaticInitCell::new();

                static_hooks!(make_detour: ($guard) ($var_name) ($($fn_mod)*) ($($arg_name)*) ($($arg_type)*) ($return_type));

                $crate::StaticHook::<$fn_type>::__new(&__DATA, $target, __detour)
            };
        }
    };

    (make_hook_var: ($($arg_name:ident)*) ($($var_attr:meta)*) ($($var_mod:tt)*) ($($hook_mod:tt)*)
                    ($var_name:ident) ($target:expr) ($($fn_mod:tt)*) ($guard:tt)
                    ($($arg_type:ty)*) ($return_type:ty) ($value:tt) ($fn_type:ty)) =>
    {
        static_hooks! { make_item:
            #[allow(non_upper_case_globals)]
            $(#[$var_attr])*
            $($var_mod)* static $var_name: $crate::StaticHookWithDefault<$fn_type> = {
                static __DATA: $crate::StaticInitCell<$crate::__StaticHookInner<$fn_type>> = $crate::StaticInitCell::new();

                static_hooks!(make_detour: ($guard) ($var_name) ($($fn_mod)*) ($($arg_name)*) ($($arg_type)*) ($return_type));

                $crate::StaticHookWithDefault::<$fn_type>::__new(
                    $crate::StaticHook::__new(&__DATA, $target, __detour),
                    &$value)
            };
        }
    };

    (make_detour: (GUARD) ($var_name:ident) ($($fn_mod:tt)*) ($($arg_name:ident)*) ($($arg_type:ty)*) ($return_type:ty)) => {
        static_hooks! { make_item:
            #[inline(never)]
            $($fn_mod)* fn __detour($($arg_name: $arg_type),*) -> $return_type {
                ::std::panic::recover(|| {
                    let &$crate::__StaticHookInner(_, ref closure) = __DATA.get().unwrap();
                    closure($($arg_name),*)
                }).unwrap_or_else(|payload| $crate::panic::__handle(module_path!(), stringify!($var_name), payload))
            }
        }
    };

    (make_detour: (NO_GUARD) ($var_name:ident) ($($fn_mod:tt)*) ($($arg_name:ident)*) ($($arg_type:ty)*) ($return_type:ty)) => {
        static_hooks! { make_item:
            #[inline(never)]
            $($fn_mod)* fn __detour($($arg_name: $arg_type),*) -> $return_type {
                let &$crate::__StaticHookInner(_, ref closure) = __DATA.get().unwrap();
                closure($($arg_name),*)
            }
        }
    };



    // Makes sure items are interpreted correctly
    (make_item: $item:item) => {
        $item
    };

    // Generates a list of idents for each given token and invokes the macro by the given label passing through arguments
    (gen_arg_names: ($label:ident) ($($args:tt)*) ($($token:tt)*)) => {
        static_hooks!(gen_arg_names: ($label)
                                     ($($args)*)
                                     (
                                         __arg_0  __arg_1  __arg_2  __arg_3  __arg_4  __arg_5  __arg_6  __arg_7
                                         __arg_8  __arg_9  __arg_10 __arg_11 __arg_12 __arg_13 __arg_14 __arg_15
                                         __arg_16 __arg_17 __arg_18 __arg_19 __arg_20 __arg_21 __arg_22 __arg_23
                                         __arg_24 __arg_25
                                     )
                                     ($($token)*)
                                     ());
    };
    (gen_arg_names: ($label:ident) ($($args:tt)*) ($hd_name:tt $($tl_name:tt)*) ($hd:tt $($tl:tt)*) ($($acc:tt)*) ) => {
        static_hooks!(gen_arg_names: ($label) ($($args)*) ($($tl_name)*) ($($tl)*) ($($acc)* $hd_name));
    };
    (gen_arg_names: ($label:ident) ($($args:tt)*) ($($name:tt)*) () ($($acc:tt)*)) => {
        static_hooks!($label: ($($acc)*) $($args)*);
    };

    // Step 0
    ($($t:tt)+) => {
        static_hooks!(parse_attr: () | $($t)+);
    };
}



#[cfg(test)]
mod tests {
    use std::mem;
    use std::sync::Mutex;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::raw::c_int;

    use {winapi, kernel32};

    use super::*;

    #[test]
    fn local() {
        fn f(x: i32) -> i32 { x * 2 }
        fn d(x: i32) -> i32 { x * 3 }

        assert_eq!(f(5), 10);
        let h = unsafe { Hook::<fn(i32) -> i32>::create(f, d).unwrap() };
        assert_eq!(f(5), 10);
        h.enable().unwrap();
        assert_eq!(f(5), 15);
        h.disable().unwrap();
        assert_eq!(f(5), 10);
        h.enable().unwrap();
        assert_eq!(f(5), 15);
        mem::drop(h);
        assert_eq!(f(5), 10);
    }

    #[test]
    fn local_dynamic() {
        extern "system" fn lstrlen_w_detour(_string: winapi::LPCWSTR) -> c_int {
            -42
        }

        let foo = OsStr::new("foo").encode_wide().chain(Some(0)).collect::<Vec<_>>();
        unsafe {
            assert_eq!(kernel32::lstrlenW(foo.as_ptr()), 3);
            let h =  Hook::<extern "system" fn(winapi::LPCWSTR) -> c_int>::create_api(
                "kernel32.dll",
                FunctionName::String("lstrlenW"),
                lstrlen_w_detour).unwrap();
            assert_eq!(kernel32::lstrlenW(foo.as_ptr()), 3);
            h.enable().unwrap();
            assert_eq!(kernel32::lstrlenW(foo.as_ptr()), -42);
            h.disable().unwrap();
            assert_eq!(kernel32::lstrlenW(foo.as_ptr()), 3);
        }
    }

    #[test]
    fn static_with_default() {
        fn f(x: i32, y: i32) -> i32 { x + y }

        static_hooks! {
            impl h for f: fn(i32, i32) -> i32 = |x, y| x * y;
        }

        assert_eq!(f(3, 6), 9);
        unsafe { h.initialize().unwrap(); }
        assert_eq!(f(3, 6), 9);
        h.enable().unwrap();
        assert_eq!(f(3, 6), 18);
        h.disable().unwrap();
        assert_eq!(f(3, 6), 9);
    }

    #[test]
    fn static_no_default() {
        fn f(x: i32, y: i32) -> i32 { x + y }

        static_hooks! {
            impl h for f: fn(i32, i32) -> i32;
        }

        let y = Mutex::new(2);
        let d = move |x, _| {
            let mut y = y.lock().unwrap();
            let r = h.call_real(x, *y);
            *y += 1;
            r
        };

        assert_eq!(f(3, 6), 9);
        unsafe { h.initialize(d).unwrap(); }
        assert_eq!(f(3, 6), 9);
        h.enable().unwrap();
        assert_eq!(f(3, 6), 5);
        assert_eq!(f(3, 6), 6);
        assert_eq!(f(3, 66), 7);
        h.disable().unwrap();
        assert_eq!(f(3, 6), 9);
    }

    #[test]
    fn static_dynamic() {
        static_hooks! {
            impl h for "lstrlenA" in "kernel32.dll": extern "system" fn(winapi::LPCSTR) -> c_int = |s| -h.call_real(s);
        }

        let foobar = b"foobar\0".as_ptr() as winapi::LPCSTR;
        unsafe {
            assert_eq!(kernel32::lstrlenA(foobar), 6);
            h.initialize().unwrap();
            assert_eq!(kernel32::lstrlenA(foobar), 6);
            h.enable().unwrap();
            assert_eq!(kernel32::lstrlenA(foobar), -6);
            h.disable().unwrap();
            assert_eq!(kernel32::lstrlenA(foobar), 6);
        }
    }

    #[test]
    #[should_panic]
    fn static_use_before_init() {
        fn f() {}

        static_hooks! {
            impl h for f: fn() = || ();
        }

        h.enable().unwrap();
    }

    #[test]
    fn queue() {
        fn f1(x: &str) -> &str { x }
        fn d1(_x: &str) -> &str { "bar" }

        fn f2(x: i32) -> i32 { x * 2 }
        fn d2(x: i32) -> i32 { x + 2 }

        fn f3(x: i32) -> Option<u32> { if x < 0 { None } else { Some(x as u32) } }
        fn d3(x: i32) -> Option<u32> { Some(x.abs() as u32) }

        let (h1, h2, h3) = unsafe { (
            Hook::<fn(&'static str) -> &'static str>::create(f1, d1).unwrap(),
            Hook::<fn(i32) -> i32>::create(f2, d2).unwrap(),
            Hook::<fn(i32) -> Option<u32>>::create(f3, d3).unwrap()
        ) };

        HookQueue::new()
                  .enable(&h1)
                  .disable(&h2)
                  .enable(&h3)
                  .disable(&h3)
                  .apply()
                  .unwrap();

        assert_eq!(f1("foo"), "bar");
        assert_eq!(f2(42), 84);
        assert_eq!(f3(-10), None);
    }
}