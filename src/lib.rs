//! # The minhook-rs library
//! This library provides function hooking support to Rust by providing a
//! Rust wrapper around the [MinHook][minhook] library.
//!
//! [minhook]: http://www.codeproject.com/KB/winsdk/LibMinHook.aspx
#![feature(on_unimplemented, static_mutex, const_fn, std_panic, recover, associated_consts, unboxed_closures)]
#![warn(missing_docs)]

use std::{io, mem, ops, result};
use std::any::Any;
use std::panic::{self, AssertRecoverSafe};
use std::sync::{StaticMutex};

use cell::{Error as CellError};
use function::{Function, FnPointer, HookableWith};

pub use cell::InitCell;
pub use error::Error;

mod cell;
mod error;
mod ffi;

pub mod function;



/// Result type for most functions and methods in this module.
pub type Result<T> = result::Result<T, Error>;

/// Function type for custom panic handling.
///
/// It takes the name of the static hook and a reference the panic argument.
/// Panicking or returning from this function results in an `abort()`.
pub type PanicHandler = fn(&'static str, &(Any + Send + 'static));

/// Initializes the minhook-rs library.
///
/// It is not required to call this function explicitly as the other library functions will do it
/// internally, unless you want to set a custom panic handler. This function will return an error
/// if the library has already been initialized either explicitly or implicitly or if initialization
/// failed (unlikely).
pub fn initialize(panic_handler: Option<PanicHandler>) -> Result<()> {
    ensure_init(panic_handler).and_then(|first| if first {
        Ok(())
    } else {
        Err(Error::AlreadyInitialized)
    })
}

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
        try!(ensure_init(None));

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
    /// This function is unsafe because there are a few guarantees to be upheld by the caller
    /// that can not be checked:
    ///
    /// * The target and detour function pointers must point to valid memory for the entire
    ///   lifetime of this hook.
    /// * The given target function type must uniquely match the actual target function. This
    ///   means two things: the given target function type has to be correct, but also there
    ///   can not be two function pointers with different signatures pointing to the same
    ///   code location. This last situation can for example happen when the Rust compiler
    ///   or LLVM decide to merge multiple functions with the same code into one.
    pub unsafe fn new<D>(target: T, detour: D) -> Result<Hook<T>>
    where T: HookableWith<D>, D: Function {
        try!(ensure_init(None));

        let target = target.to_ptr();
        let trampoline = try!(Hook::new_inner(target, detour.to_ptr()));

        Ok(Hook {
            target: target,
            trampoline: trampoline,
        })
    }

    unsafe fn new_inner(target: FnPointer, detour: FnPointer) -> Result<T> {
        let mut trampoline: ffi::LPVOID = mem::uninitialized();
        try!(s2r(ffi::MH_CreateHook(target.to_raw(), detour.to_raw(), &mut trampoline)));

        Ok(T::from_ptr(FnPointer::from_raw(trampoline)))
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
/// This hook can only be constructed using the `static_hooks!` macro. It has the form:
///
/// ```ignore
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE;
/// ```
///
/// Before accessing this hook it is **required** to call `initialize()` **once**. Accessing the
/// hook before initializing or trying to initialize the hook twice (even after the first attempt
/// failed) will result in a panic.
pub struct StaticHook<T: Function> {
    hook: &'static InitCell<__StaticHookInner<T>>,
    target: __StaticHookTarget<T>,
    detour: T
}

impl<T: Function> StaticHook<T> {
    #[doc(hidden)]
    pub const fn __new(hook: &'static InitCell<__StaticHookInner<T>>, target: __StaticHookTarget<T>, detour: T) -> StaticHook<T> {
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
        let result = self.hook.initialize(|| {
            let target = match self.target {
                __StaticHookTarget::Static(target) => target,
                __StaticHookTarget::Dynamic(..) => unimplemented!()
            };

            Hook::new(target, self.detour).map(|hook| __StaticHookInner(hook, closure))
        });

        match result {
            Ok(true) => Ok(()),
            Err(CellError::Initialization(error)) => Err(error),
            Ok(false) | Err(CellError::Dead) => panic!("attempt to initialize static hook more than once or after access")
        }
    }

    unsafe fn initialize_box(&self, closure: Box<Fn<T::Args, Output = T::Output> + Sync>) -> Result<()> {
        let guard = BoxGuard::new(closure);
        try!(self.initialize_ref(guard.as_static_ref()));
        guard.release();
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
    /// See documentation for [`Hook::new()`](struct.Hook.html#method.new).
    pub unsafe fn initialize<F>(&self, closure: F) -> Result<()>
    where F: Fn<T::Args, Output = T::Output> + Sync + 'static {
        self.initialize_box(Box::new(closure))
    }

    fn inner(&self) -> &'static Hook<T> {
        let &__StaticHookInner(ref hook, _) = self.hook.get().expect("attempt to access uninitializaed static hook");
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
/// This hook can only be constructed using the `static_hooks!` macro. It has the form:
///
/// ```ignore
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE = CLOSURE_EXPR;
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
    /// See documentation for [`Hook::new()`](struct.Hook.html#method.new).
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




static PANIC_HANDLER: InitCell<Option<PanicHandler>> = InitCell::new();

fn ensure_init(panic_handler: Option<PanicHandler>) -> Result<bool> {
    let result = PANIC_HANDLER.initialize(|| {
        unsafe { s2r(ffi::MH_Initialize()).map(|_| panic_handler) }
    });

    result.map_err(|error| match error {
        CellError::Initialization(error) => error,
        CellError::Dead => panic!("attempt to initialize library after initialization previously failed")
    })
}

fn s2r(status: ffi::MH_STATUS) -> Result<()> {
    Error::from_status(status).map_or(Ok(()), Err)
}

struct BoxGuard<T: ?Sized>(*mut T);

impl<T: ?Sized> BoxGuard<T> {
    fn new(value: Box<T>) -> BoxGuard<T> {
        BoxGuard(Box::into_raw(value))
    }

    fn release(self) {
        mem::forget(self);
    }

    unsafe fn as_static_ref(&self) -> &'static T {
        &*self.0
    }
}

impl<T: ?Sized> Drop for BoxGuard<T> {
    fn drop(&mut self) {
        unsafe { mem::drop(Box::from_raw(self.0)); }
    }
}



#[doc(hidden)]
pub struct __StaticHookInner<T: Function>(Hook<T>, pub &'static (Fn<T::Args, Output = T::Output> + Sync));

#[doc(hidden)]
pub enum __StaticHookTarget<T: Function> {
    Static(T),
    Dynamic(&'static str, &'static str)
}

#[doc(hidden)]
pub fn __handle_panic(name: &'static str, arg: Box<Any + Send + 'static>) -> ! {
    use std::io::Write;

    extern {
        fn abort() -> !;
    }

    let arg = AssertRecoverSafe::new(arg);

    let _ = panic::recover(move || {
        if let &Some(panic_handler) = PANIC_HANDLER.get().unwrap() {
            panic_handler(name, &**arg)
        } else {
            let message = if let Some(message) = arg.downcast_ref::<&str>() {
                Some(*message)
            } else if let Some(message) = arg.downcast_ref::<String>() {
                Some(message.as_ref())
            } else {
                None
            };

            let _ = write!(&mut io::stderr(), "The detour function for `{}` panicked", name);
            if let Some(message) = message {
                let _ = write!(&mut io::stderr(), " with the message: {}", message);
            }
            let _ = writeln!(&mut io::stderr(), ". Aborting.");
        }
    });

    unsafe { abort() }
}



/// Declares one or more thread-safe static hooks.
///
/// The syntax for these hooks is:
///
/// ```ignore
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE = CLOSURE_EXPR;
/// #[ATTR]* pub? impl HOOK_VAR_NAME for PATH::TO::TARGET: FN_TYPE;
/// ```
#[macro_export]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! static_hooks {
    // Step 1: parse attributes
    (parse_attr: ($($args:tt)*)
               | $(#[$var_attr:meta])* $next:tt $($rest:tt)*) => {
        static_hooks!(parse_pub: (($($var_attr)*)) | $next $($rest)*);
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
    // (parse_name_target: ($($args:tt)*)
    //                   | $var_name:ident for $target_fn_name:tt in $target_mod_name:tt : $($rest:tt)*) =>
    // {
    //     static_hooks!(parse_fn_unsafe: ($($args)* ($var_name) ($crate::__StaticHookTarget::Dynamic($target_mod_name, $target_fn_name))) | $($rest)*);
    // };
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
                static __DATA: $crate::InitCell<$crate::__StaticHookInner<$fn_type>> = $crate::InitCell::new();

                static_hooks!(make_detour: ($guard) ($var_name) ($($fn_mod)*) ($($arg_name)*) ($($arg_type)*) ($return_type));

                $crate::StaticHook::__new(&__DATA, $target, __detour as $fn_type)
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
                static __DATA: $crate::InitCell<$crate::__StaticHookInner<$fn_type>> = $crate::InitCell::new();

                static_hooks!(make_detour: ($guard) ($var_name) ($($fn_mod)*) ($($arg_name)*) ($($arg_type)*) ($return_type));

                $crate::StaticHookWithDefault::__new(
                    $crate::StaticHook::__new(&__DATA, $target, __detour as $fn_type),
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
                }).unwrap_or_else(|arg| $crate::__handle_panic(stringify!($var_name), arg))
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

    use super::*;

    #[test]
    fn local() {
        fn f(x: i32) -> i32 { x * 2 }
        fn d(x: i32) -> i32 { x * 3 }

        assert_eq!(f(5), 10);
        let h = unsafe { Hook::<fn(i32) -> i32>::new(f, d).unwrap() };
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
            Hook::<fn(&'static str) -> &'static str>::new(f1, d1).unwrap(),
            Hook::<fn(i32) -> i32>::new(f2, d2).unwrap(),
            Hook::<fn(i32) -> Option<u32>>::new(f3, d3).unwrap()
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