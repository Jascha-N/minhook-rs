# minhook-rs

[![Build status](https://ci.appveyor.com/api/projects/status/e7yg48n0835hy9b6?svg=true)](https://ci.appveyor.com/project/Jascha-N/minhook-rs)

A function hooking library for the Rust programming language. This library provides efficient and safe bindings to the
[MinHook](https://github.com/TsudaKageyu/minhook) library.

It currently supports the x86 and x86_64 architectures and the GCC (MinGW) and MSVC toolchains on Windows. Requires the Rust Nightly compiler.
The supported target triples are:
- `i686-pc-windows-msvc`
- `x86_64-pc-windows-msvc`
- `i686-pc-windows-gnu`
- `x86_64-pc-windows-gnu`

## Usage
First, add the following lines to your `Cargo.toml`:

```toml
[dependencies]
minhook = { git = "https://github.com/Jascha-N/minhook-rs" }
```

Next, add this to your crate root:

```rust
#[macro_use]
extern crate minhook;
```

### Features
The minhook-rs library has the following features:
- `increased_arity` - If there is a need to hook functions with an arity greater than 12, this will allow functions of up to 26 arguments to be hooked.
- `winapi`          - Because [`winapi`](https://github.com/retep998/winapi-rs) is such a huge dependency, it is optional.

## Example

Example using a static hook.

```rust
#![feature(const_fn, recover)]

#[macro_use]
extern crate minhook;
extern crate winapi;
extern crate user32;

use std::ptr;

use winapi::{HWND, LPCSTR, UINT, c_int};

static_hooks! {
    // Create a hook for user32::MessageBoxA.
    impl MessageBoxA for user32::MessageBoxA: unsafe extern "system" fn(HWND, LPCSTR, LPCSTR, UINT) -> c_int;
}

fn main() {
    // Create a detour closure. This closure can capture any Sync variables.
    let detour = |wnd, text, caption, flags| unsafe { MessageBoxA.call_real(wnd, caption, text, flags) };

	// Install the hook.
    unsafe { MessageBoxA.initialize(detour).unwrap(); }

    let hello = b"Hello\0".as_ptr() as LPCSTR;
    let world = b"World\0".as_ptr() as LPCSTR;

    // Call the function.
    unsafe { user32::MessageBoxA(ptr::null_mut(), hello, world, winapi::MB_OK); }

    // Enable the hook.
    MessageBoxA.enable().unwrap();

    // Call the - now hooked - function.
    unsafe { user32::MessageBoxA(ptr::null_mut(), hello, world, winapi::MB_OK); }
}
```
