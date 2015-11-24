use super::*;
use std::mem;

fn func_detour(x: i32, y: i32) -> i32 {
    x * y
}

#[test]
fn test_local_hook() {
    fn func(x: i32, y: i32) -> i32 {
        x + y
    }

    assert_eq!(func(2, 5), 7);
    let hook = unsafe {
        LocalHook::new(func as fn(i32, i32) -> i32,
                       func_detour as fn(i32, i32) -> i32)
            .unwrap()
    };

    assert_eq!(func(2, 5), 7);
    assert_eq!(hook.with_trampoline_safe(|f| f(2, 5)), 7);

    hook.set_enabled(true).unwrap();
    assert_eq!(func(2, 5), 10);
    assert_eq!(hook.with_trampoline_safe(|f| f(2, 5)), 7);

    hook.set_enabled(false).unwrap();
    assert_eq!(func(2, 5), 7);

    hook.set_enabled(true).unwrap();
    assert_eq!(func(2, 5), 10);

    hook.destroy().unwrap();

    assert_eq!(func(2, 5), 7);
}

#[test]
fn test_static_hook_locally() {
    fn func(x: i32, y: i32) -> i32 {
        x + y
    }

    assert_eq!(func(2, 5), 7);
    let hook = unsafe {
        LocalHook::new(func as fn(i32, i32) -> i32,
                       func_detour as fn(i32, i32) -> i32)
            .unwrap()
    };

    assert_eq!(func(2, 5), 7);
    let static_hook = hook.into_static();

    assert_eq!(func(2, 5), 7);
    assert_eq!(static_hook(2, 5), 7);

    static_hook.set_enabled(true).unwrap();
    assert_eq!(func(2, 5), 10);
    assert_eq!(static_hook(2, 5), 7);
    assert_eq!(static_hook.trampoline()(2, 5), 7);
    assert_eq!(static_hook.with_trampoline_safe(|f| f(2, 5)), 7);

    mem::drop(static_hook);

    assert_eq!(func(2, 5), 10);
}

#[test]
fn test_static_hook_statically() {
    fn func(x: i32, y: i32) -> i32 {
        x + y
    }

    let hook = unsafe {
        LocalHook::new(func as fn(i32, i32) -> i32,
                       func_detour as fn(i32, i32) -> i32)
            .unwrap()
    };

    static mut HOOK: Option<StaticHook<fn(i32, i32) -> i32>> = None;
    unsafe {
        HOOK = Some(hook.into_static());
        assert_eq!(func(2, 5), 7);
        HOOK.as_ref().unwrap().set_enabled(true).unwrap();
        assert_eq!(func(2, 5), 10);
    }
}

#[test]
fn test_static_hook_macro() {
    fn func(x: i32, y: i32) -> i32 {
        x + y
    }

    static_hooks! {
		unsafe hook<fn(i32, i32) -> i32> static_hook(x, y) for func {
			static_hook(x * 2, y)
		}
	}

    assert_eq!(func(2, 5), 7);
    static_hook.initialize().unwrap();

    assert_eq!(func(2, 5), 7);
    static_hook.set_enabled(true).unwrap();

    assert_eq!(func(2, 5), 9);
}

#[test]
#[should_panic]
// Workaround for broken unwinding on 32-bit MSVC
#[cfg(not(all(target_env = "msvc", target_arch = "x86")))]
fn test_static_hook_macro_panic() {
    fn func(x: i32, y: i32) -> i32 {
        x + y
    }

    let _hook = unsafe {
        LocalHook::new(func as fn(i32, i32) -> i32,
                       func_detour as fn(i32, i32) -> i32)
            .unwrap()
    };

    static_hooks! {
		unsafe hook<fn(i32, i32) -> i32> static_hook(x, y) for func {
			static_hook(x * 2, y)
		}
	}

    static_hook(10, 10);
}
