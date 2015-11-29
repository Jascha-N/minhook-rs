use std::{error, fmt};

use ffi::MH_STATUS;

/// The error type for all hooking operations.
///
/// MinHook error status codes map directly to this type.
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
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
    /// Constructs an `Error` from a MinHook status.
    pub fn from(status: MH_STATUS) -> Option<Error> {
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