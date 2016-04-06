//! Panic handling for panics caught at foreign code boundaries in detour functions.

use libc;
use std::any::Any;
use std::io::{self, Write};
use std::panic::{self, AssertRecoverSafe};

use sync::StaticRwCell;



/// A struct providing information about a panic that happened inside of a guarded detour function.
#[derive(Clone, Copy, Debug)]
pub struct DetourPanicInfo<'a> {
    payload: &'a (Any + Send),
    detour: &'a str
}

impl<'a> DetourPanicInfo<'a> {
    /// Returns the payload associated with the panic.
    ///
    /// This will commonly, but not always, be a `&'static str` or `String`.
    pub fn payload(&self) -> &(Any + Send) {
        self.payload
    }

    /// Returns the name of the static hook for which the detour function
    /// panicked.
    pub fn detour(&self) -> &str {
        &self.detour
    }
}



static HANDLER: StaticRwCell<Option<Box<Fn(&DetourPanicInfo) + Sync + Send>>> = StaticRwCell::new(None);

/// Registers a custom detour panic handler, replacing any that was previously
/// registered.
///
/// The panic handler is invoked when an extern detour function panics just before
/// the code would unwind into foreign code. The default handler prints a message
/// to standard error and aborts the process to prevent further unwinding, but this behavior
/// can be customized with the `set_handler` and `take_handler` functions.
///
/// The handler is provided with a `DetourPanicInfo` struct which contains information
/// about the origin of the panic, including the payload passed to `panic!` and
/// the name of the name of the associated hook.
///
/// If the handler panics or returns normally, the process will be aborted.
///
/// The panic handler is a global resource.
pub fn set_handler<F>(handler: F)
where F: Fn(&DetourPanicInfo) + Sync + Send + 'static {
    HANDLER.set(Some(Box::new(handler)));
}

/// Unregisters the current panic handler, returning it.
///
/// If no custom handler is registered, the default handler will be returned.
pub fn take_handler() -> Box<Fn(&DetourPanicInfo) + Sync + Send> {
    HANDLER.take().unwrap_or_else(|| Box::new(default_handler))
}

#[doc(hidden)]
pub fn __handle(path: &'static str, name: &'static str, payload: Box<Any + Send>) -> ! {
    let payload = AssertRecoverSafe(payload);

    let _ = panic::recover(move || {
        let full_path = format!("{}::{}", path, name);
        let info = DetourPanicInfo {
            payload: &**payload,
            detour: &full_path
        };

        HANDLER.with(|handler| {
            if let Some(ref handler) = *handler {
                handler(&info);
            } else {
                default_handler(&info);
            }
        });
    });

    unsafe { libc::abort() }
}

fn default_handler(info: &DetourPanicInfo) {
    let mut stderr = io::stderr();
    let _ = writeln!(stderr, "The detour function for '{}' panicked. Aborting.", info.detour);
    let _ = stderr.flush();
}