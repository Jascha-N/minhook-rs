use super::*;
use std::mem;

fn func_detour(x: i32, y: i32) -> i32 {
    x * y
}

#[test]
fn test_scoped_hook() {
    fn func(x: i32, y: i32) -> i32 {
        x + y
    }

    assert_eq!(func(2, 5), 7);
    let hook = unsafe {
        ScopedHook::install(func as fn(i32, i32) -> i32,
                            func_detour as fn(i32, i32) -> i32)
                   .unwrap()
    };

    assert_eq!(func(2, 5), 7);
    assert_eq!(hook.call_real(2, 5), 7);

    hook.enable().unwrap();
    assert_eq!(func(2, 5), 10);
    assert_eq!(hook.call_real(2, 5), 7);

    hook.disable().unwrap();
    assert_eq!(func(2, 5), 7);

    hook.enable().unwrap();
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
        ScopedHook::install(func as fn(i32, i32) -> i32,
                            func_detour as fn(i32, i32) -> i32)
                   .unwrap()
    };

    assert_eq!(func(2, 5), 7);
    let static_hook = hook.into_static();

    assert_eq!(func(2, 5), 7);
    assert_eq!(static_hook.call_real(2, 5), 7);

    static_hook.enable().unwrap();
    assert_eq!(func(2, 5), 10);
    assert_eq!(static_hook.call_real(2, 5), 7);
    assert_eq!(static_hook.trampoline()(2, 5), 7);

    mem::drop(static_hook);

    assert_eq!(func(2, 5), 10);
}

#[test]
fn test_static_hook_statically() {
    fn func(x: i32, y: i32) -> i32 {
        x + y
    }

    let hook = unsafe {
        ScopedHook::install(func as fn(i32, i32) -> i32,
                            func_detour as fn(i32, i32) -> i32)
                   .unwrap()
    };

    static mut HOOK: Option<StaticHook<fn(i32, i32) -> i32>> = None;
    unsafe {
        HOOK = Some(hook.into_static());
        assert_eq!(func(2, 5), 7);
        HOOK.as_ref().unwrap().enable().unwrap();
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
            static_hook.call_real(x * 2, y)
        }
    }

    assert_eq!(func(2, 5), 7);
    static_hook.install().unwrap();

    assert_eq!(func(2, 5), 7);
    static_hook.enable().unwrap();

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
        ScopedHook::install(func as fn(i32, i32) -> i32,
                            func_detour as fn(i32, i32) -> i32)
                   .unwrap()
    };

    static_hooks! {
        unsafe hook<fn(i32, i32) -> i32> static_hook(x, y) for func {
            static_hook.call_real(x * 2, y)
        }
    }

    static_hook.call_real(10, 10);
}

#[test]
fn test_hook_queue() {
    fn func1() -> &'static str {
        "foo"
    }
    fn detour1() -> &'static str {
        "bar"
    }

    fn func2() -> i32 {
        7
    }
    fn detour2() -> i32 {
        42
    }

    fn func3(x: i32) -> Option<i32> {
        Some(x)
    }
    fn detour3(_x: i32) -> Option<i32> {
        None
    }

    let (hook1, hook2, hook3) = unsafe {
        (ScopedHook::install(func1 as fn() -> &'static str,
                             detour1 as fn() -> &'static str)
                    .unwrap(),
         ScopedHook::install(func2 as fn() -> i32,
                             detour2 as fn() -> i32)
                    .unwrap(),
         ScopedHook::install(func3 as fn(i32) -> Option<i32>,
                             detour3 as fn(i32) -> Option<i32>)
                    .unwrap())
    };

    hook2.enable().unwrap();

    let mut queue = HookQueue::new();
    queue.enable(&hook1).disable(&hook2).enable(&hook3).disable(&hook3);
    queue.apply().unwrap();

    assert_eq!(func1(), "bar");
    assert_eq!(func2(), 7);
    assert_eq!(func3(11), Some(11));
}