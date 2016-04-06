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
    (@parse_attr ($($args:tt)*)
               | $(#[$var_attr:meta])* $next:tt $($rest:tt)*) => {
        static_hooks!(@parse_pub ($($args)* ($($var_attr)*)) | $next $($rest)*);
    };

    // Step 2: parse optional pub modifier
    (@parse_pub ($($args:tt)*)
              | pub impl $($rest:tt)*) =>
    {
        static_hooks!(@parse_mod ($($args)* (pub)) | $($rest)*);
    };
    (@parse_pub ($($args:tt)*)
              | impl $($rest:tt)*) =>
    {
        static_hooks!(@parse_mod ($($args)* ()) | $($rest)*);
    };

    // Step 3: parse optional mut or const modifier
    // (@parse_mod ($($args:tt)*)
    //           | mut $($rest:tt)*) =>
    // {
    //     static_hooks!(@parse_name_target ($($args)* (mut)) | $($rest)*);
    // };
    // (@parse_mod ($($args:tt)*)
    //           | const $($rest:tt)*) =>
    // {
    //     static_hooks!(@parse_name_target ($($args)* (const)) | $($rest)*);
    // };
    (@parse_mod ($($args:tt)*)
              | $($rest:tt)*) =>
    {
        static_hooks!(@parse_name_target ($($args)* ()) | $($rest)*);
    };

    // Step 4: parse name and target
    (@parse_name_target ($($args:tt)*)
                      | $var_name:ident for $target_fn_name:tt in $target_mod_name:tt : $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_unsafe ($($args)* ($var_name) ($crate::__StaticHookTarget::Dynamic($target_mod_name, $target_fn_name))) | $($rest)*);
    };
    (@parse_name_target ($($args:tt)*)
                      | $var_name:ident for $target_path:path : $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_unsafe ($($args)* ($var_name) ($crate::__StaticHookTarget::Static($target_path))) | $($rest)*);
    };

    // Step 5a: parse optional unsafe modifier
    (@parse_fn_unsafe ($($args:tt)*)
                    | unsafe $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_linkage ($($args)*) (unsafe) | $($rest)*);
    };
    (@parse_fn_unsafe ($($args:tt)*)
                    | $($rest:tt)*) => {
        static_hooks!(@parse_fn_linkage ($($args)*) () | $($rest)*);
    };

    // Step 5b: parse linkage
    (@parse_fn_linkage ($($args:tt)*) ($($fn_mod:tt)*)
                     | extern $linkage:tt fn $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_args ($($args)* ($($fn_mod)* extern $linkage) (GUARD)) | $($rest)*);
    };
    (@parse_fn_linkage ($($args:tt)*) ($($fn_mod:tt)*)
                     | extern fn $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_args ($($args)* ($($fn_mod)* extern) (GUARD)) | $($rest)*);
    };
    (@parse_fn_linkage ($($args:tt)*) ($($fn_mod:tt)*)
                     | fn $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_args ($($args)* ($($fn_mod)*) (NO_GUARD)) | $($rest)*);
    };

    // Step 5c: parse argument types and return type
    // Requires explicit look-ahead to satisfy rule for tokens following ty fragment specifier
    (@parse_fn_args ($($args:tt)*)
                  | ($($arg_type:ty),*) -> $return_type:ty = $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_value ($($args)* ($($arg_type)*) ($return_type)) | = $($rest)*);
    };
    (@parse_fn_args ($($args:tt)*)
                  | ($($arg_type:ty),*) -> $return_type:ty ; $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_value ($($args)* ($($arg_type)*) ($return_type)) | ; $($rest)*);
    };

    (@parse_fn_args ($($args:tt)*)
                  | ($($arg_type:ty),*) $($rest:tt)*) =>
    {
        static_hooks!(@parse_fn_value ($($args)* ($($arg_type)*) (())) | $($rest)*);
    };

    // Step 6: parse argument types and return type
    // Requires explicit look-ahead to satisfy rule for tokens following ty fragment specifier
    (@parse_fn_value ($($args:tt)*)
                   | = $value:expr ; $($rest:tt)*) =>
    {
        static_hooks!(@parse_rest ($($args)* ($value)) | $($rest)*);
    };
    (@parse_fn_value ($($args:tt)*)
                   | ; $($rest:tt)*) =>
    {
        static_hooks!(@parse_rest ($($args)* (!)) | $($rest)*);
    };

    // Step 6: parse rest and recurse
    (@parse_rest ($($args:tt)*)
               | $($rest:tt)+) =>
    {
        static_hooks!(@make $($args)*);
        static_hooks!($($rest)*);
    };
    (@parse_rest ($($args:tt)*)
               | ) =>
    {
        static_hooks!(@make $($args)*);
    };

    // Step 7: parse rest and recurse
    (@make ($($var_attr:meta)*) ($($var_mod:tt)*) ($($hook_mod:tt)*) ($var_name:ident) ($target:expr)
           ($($fn_mod:tt)*) ($guard:tt) ($($arg_type:ty)*) ($return_type:ty) ($value:tt)) =>
    {
        static_hooks!(@gen_arg_names (make_hook_var)
                                     (
                                         ($($var_attr)*) ($($var_mod)*) ($($hook_mod)*) ($var_name) ($target)
                                         ($($fn_mod)*) ($guard) ($($arg_type)*) ($return_type) ($value)
                                         ($($fn_mod)* fn ($($arg_type),*) -> $return_type)
                                     )
                                     ($($arg_type)*));
    };

    (@make_hook_var ($($arg_name:ident)*) ($($var_attr:meta)*) ($($var_mod:tt)*) ($($hook_mod:tt)*)
                    ($var_name:ident) ($target:expr) ($($fn_mod:tt)*) ($guard:tt)
                    ($($arg_type:ty)*) ($return_type:ty) (!) ($fn_type:ty)) =>
    {
        static_hooks!(@make_item
            #[allow(non_upper_case_globals)]
            $(#[$var_attr])*
            $($var_mod)* static $var_name: $crate::StaticHook<$fn_type> = {
                static __DATA: $crate::AtomicInitCell<$crate::__StaticHookInner<$fn_type>> = $crate::AtomicInitCell::new();

                static_hooks!(@make_detour ($guard) ($var_name) ($($fn_mod)*) ($($arg_name)*) ($($arg_type)*) ($return_type));

                $crate::StaticHook::<$fn_type>::__new(&__DATA, $target, __detour)
            };
        );
    };

    (@make_hook_var ($($arg_name:ident)*) ($($var_attr:meta)*) ($($var_mod:tt)*) ($($hook_mod:tt)*)
                    ($var_name:ident) ($target:expr) ($($fn_mod:tt)*) ($guard:tt)
                    ($($arg_type:ty)*) ($return_type:ty) ($value:tt) ($fn_type:ty)) =>
    {
        static_hooks!(@make_item
            #[allow(non_upper_case_globals)]
            $(#[$var_attr])*
            $($var_mod)* static $var_name: $crate::StaticHookWithDefault<$fn_type> = {
                static __DATA: $crate::AtomicInitCell<$crate::__StaticHookInner<$fn_type>> = $crate::AtomicInitCell::new();

                static_hooks!(@make_detour ($guard) ($var_name) ($($fn_mod)*) ($($arg_name)*) ($($arg_type)*) ($return_type));

                $crate::StaticHookWithDefault::<$fn_type>::__new(
                    $crate::StaticHook::__new(&__DATA, $target, __detour),
                    &$value)
            };
        );
    };

    (@make_detour (GUARD) ($var_name:ident) ($($fn_mod:tt)*) ($($arg_name:ident)*) ($($arg_type:ty)*) ($return_type:ty)) => {
        static_hooks!(@make_item
            #[inline(never)]
            $($fn_mod)* fn __detour($($arg_name: $arg_type),*) -> $return_type {
                ::std::panic::recover(|| {
                    let &$crate::__StaticHookInner(_, ref closure) = __DATA.get().unwrap();
                    closure($($arg_name),*)
                }).unwrap_or_else(|payload| $crate::panic::__handle(module_path!(), stringify!($var_name), payload))
            }
        );
    };

    (@make_detour (NO_GUARD) ($var_name:ident) ($($fn_mod:tt)*) ($($arg_name:ident)*) ($($arg_type:ty)*) ($return_type:ty)) => {
        static_hooks!(@make_item
            #[inline(never)]
            $($fn_mod)* fn __detour($($arg_name: $arg_type),*) -> $return_type {
                let &$crate::__StaticHookInner(_, ref closure) = __DATA.get().unwrap();
                closure($($arg_name),*)
            }
        );
    };



    // Makes sure items are interpreted correctly
    (@make_item $item:item) => {
        $item
    };

    // Generates a list of idents for each given token and invokes the macro by the given label passing through arguments
    (@gen_arg_names ($label:ident) ($($args:tt)*) ($($token:tt)*)) => {
        static_hooks!(@gen_arg_names ($label)
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
    (@gen_arg_names ($label:ident) ($($args:tt)*) ($hd_name:tt $($tl_name:tt)*) ($hd:tt $($tl:tt)*) ($($acc:tt)*) ) => {
        static_hooks!(@gen_arg_names ($label) ($($args)*) ($($tl_name)*) ($($tl)*) ($($acc)* $hd_name));
    };
    (@gen_arg_names ($label:ident) ($($args:tt)*) ($($name:tt)*) () ($($acc:tt)*)) => {
        static_hooks!(@$label ($($acc)*) $($args)*);
    };

    // Step 0
    ($($t:tt)+) => {
        static_hooks!(@parse_attr () | $($t)+);
    };
}

macro_rules! impl_hookable {
    (@recurse () ($($nm:ident : $ty:ident),*)) => {
        impl_hookable!(@impl_all ($($nm : $ty),*));
    };
    (@recurse ($hd_nm:ident : $hd_ty:ident $(, $tl_nm:ident : $tl_ty:ident)*) ($($nm:ident : $ty:ident),*)) => {
        impl_hookable!(@impl_all ($($nm : $ty),*));
        impl_hookable!(@recurse ($($tl_nm : $tl_ty),*) ($($nm : $ty,)* $hd_nm : $hd_ty));
    };

    (@impl_all ($($nm:ident : $ty:ident),*)) => {
        impl_hookable!(@impl_pair ($($nm : $ty),*) (                  fn($($ty),*) -> Ret));
        impl_hookable!(@impl_pair ($($nm : $ty),*) (extern "cdecl"    fn($($ty),*) -> Ret));
        impl_hookable!(@impl_pair ($($nm : $ty),*) (extern "stdcall"  fn($($ty),*) -> Ret));
        impl_hookable!(@impl_pair ($($nm : $ty),*) (extern "fastcall" fn($($ty),*) -> Ret));
        impl_hookable!(@impl_pair ($($nm : $ty),*) (extern "win64"    fn($($ty),*) -> Ret));
        impl_hookable!(@impl_pair ($($nm : $ty),*) (extern "C"        fn($($ty),*) -> Ret));
        impl_hookable!(@impl_pair ($($nm : $ty),*) (extern "system"   fn($($ty),*) -> Ret));
    };

    (@impl_pair ($($nm:ident : $ty:ident),*) ($($fn_t:tt)*)) => {
        impl_hookable!(@impl_fun ($($nm : $ty),*) ($($fn_t)*) (unsafe $($fn_t)*));
    };

    (@impl_fun ($($nm:ident : $ty:ident),*) ($safe_type:ty) ($unsafe_type:ty)) => {
        impl_hookable!(@impl_core ($($nm : $ty),*) ($safe_type) ($unsafe_type));
        impl_hookable!(@impl_core ($($nm : $ty),*) ($unsafe_type) ($unsafe_type));

        impl_hookable!(@impl_hookable_with ($($nm : $ty),*) ($unsafe_type) ($safe_type));

        impl_hookable!(@impl_safe ($($nm : $ty),*) ($safe_type));
        impl_hookable!(@impl_unsafe ($($nm : $ty),*) ($unsafe_type));
    };

    (@impl_hookable_with ($($nm:ident : $ty:ident),*) ($target:ty) ($detour:ty)) => {
        unsafe impl<Ret: 'static, $($ty: 'static),*> HookableWith<$detour> for $target {}
    };

    (@impl_safe ($($nm:ident : $ty:ident),*) ($fn_type:ty)) => {
        impl<Ret: 'static, $($ty: 'static),*> Hook<$fn_type> {
            #[doc(hidden)]
            #[cfg_attr(feature = "clippy", allow(too_many_arguments))]
            pub fn call_real(&self, $($nm : $ty),*) -> Ret {
                (self.trampoline)($($nm),*)
            }
        }
    };

    (@impl_unsafe ($($nm:ident : $ty:ident),*) ($fn_type:ty)) => {
        unsafe impl<Ret: 'static, $($ty: 'static),*> UnsafeFunction for $fn_type {}

        impl<Ret: 'static, $($ty: 'static),*> Hook<$fn_type> {
            #[doc(hidden)]
            #[cfg_attr(feature = "clippy", allow(too_many_arguments))]
            pub unsafe fn call_real(&self, $($nm : $ty),*) -> Ret {
                (self.trampoline)($($nm),*)
            }
        }
    };

    (@impl_core ($($nm:ident : $ty:ident),*) ($fn_type:ty) ($unsafe_type:ty)) => {
        unsafe impl<Ret: 'static, $($ty: 'static),*> Function for $fn_type {
            type Args = ($($ty,)*);
            type Output = Ret;
            type Unsafe = $unsafe_type;

            const ARITY: usize = impl_hookable!(@count ($($ty)*));

            unsafe fn from_ptr(ptr: FnPointer) -> Self {
                mem::transmute(ptr.to_raw())
            }

            fn to_ptr(&self) -> FnPointer {
                unsafe { FnPointer::from_raw(*self as *mut c_void) }
            }

            #[cfg_attr(feature = "clippy", allow(useless_transmute))]
            fn to_unsafe(&self) -> Self::Unsafe {
                unsafe { mem::transmute(*self) }
            }
        }
    };

    (@count ()) => {
        0
    };
    (@count ($hd:tt $($tl:tt)*)) => {
        1 + impl_hookable!(@count ($($tl)*))
    };

    ($($nm:ident : $ty:ident),*) => {
        impl_hookable!(@recurse ($($nm : $ty),*) ());
    };
}
