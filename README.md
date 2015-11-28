# minhook-rs

[![Build status](https://ci.appveyor.com/api/projects/status/e7yg48n0835hy9b6?svg=true)](https://ci.appveyor.com/project/Jascha-N/minhook-rs)

A function hooking library for the Rust programming language. This library provides efficient and safe bindings to the
[MinHook](https://github.com/TsudaKageyu/minhook) library.

It currently supports the x86 and x86_64 architectures and the GCC (MinGW) and MSVC toolchains on Windows.
The supported target triples are:
- `i686-pc-windows-msvc`
- `x86_64-pc-windows-msvc`
- `i686-pc-windows-gnu`
- `x86_64-pc-windows-gnu`

## Usage
First, add the following lines to your `Cargo.toml`:

```toml
[dependencies]
minhook-rs = { git = "https://github.com/Jascha-N/minhook-rs" }
```

Next, add this to your crate root:

```rust
#[macro_use]
extern crate minhook;
```

### Features
The minhook-rs library has the following features:
- `nightly`         - Enables some gated features only available with the Nightly compiler.
- `increased_arity` - If there is a need to hook functions with an arity greater than 12, this will allow functions of up to 26 arguments to be hooked.
- `winapi`          - Because [`winapi`](https://github.com/retep998/winapi-rs) is such a huge dependency, it is optional.

## Example

Example using a static hook.

```rust
#[macro_use]
extern crate minhook;
extern crate winapi;
extern crate user32;

use std::ptr;
use minhook::prelude::*;

mod hooks {
    use winapi::{HWND, LPCSTR, UINT, c_int};

    static_hooks! {
        // Create a hook for user32::MessageBoxA
        pub unsafe hook<unsafe extern "system" fn(HWND, LPCSTR, LPCSTR, UINT) -> c_int>
        MessageBoxA(wnd, text, caption, flags) for ::user32::MessageBoxA {
            // Switch caption and text and call the original function
            MessageBoxA.call_real(wnd, caption, text, flags)
        }
    }
}

fn main() {
	// Install the hook
    hooks::MessageBoxA.install().unwrap();

    // Call the function
    unsafe {
        user32::MessageBoxA(ptr::null_mut(),
                            b"Hello\0".as_ptr() as *const _,
                            b"World\0".as_ptr() as *const _,
                            winapi::MB_OK);
    }

    // Enable the hook
    hooks::MessageBoxA.set_enabled(true).unwrap();

    // Call the - now hooked - function
    unsafe {
        user32::MessageBoxA(ptr::null_mut(),
                            b"Hello\0".as_ptr() as *const _,
                            b"World\0".as_ptr() as *const _,
                            winapi::MB_OK);
    }
}
```
