//! Raw bindings for the MinHook library.
//!
//! The functions exposed in this module provide absolutely no guarantees with
//! respect to type-safety of hooked functions. There should generally be no
//! reason to use this module directly.
#![allow(dead_code)]

extern crate winapi;

use std::ptr;

pub use self::winapi::{LPCSTR, LPCWSTR, LPVOID};

/// MinHook Error Codes.
#[must_use]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MH_STATUS {
    /// Unknown error. Should not be returned.
    MH_UNKNOWN = -1,
    /// Successful.
    MH_OK = 0,
    /// MinHook is already initialized.
    MH_ERROR_ALREADY_INITIALIZED,
    /// MinHook is not initialized yet, or already uninitialized.
    MH_ERROR_NOT_INITIALIZED,
    /// The hook for the specified target function is already created.
    MH_ERROR_ALREADY_CREATED,
    /// The hook for the specified target function is not created yet.
    MH_ERROR_NOT_CREATED,
    /// The hook for the specified target function is already enabled.
    MH_ERROR_ENABLED,
    /// The hook for the specified target function is not enabled yet, or
    /// already disabled.
    MH_ERROR_DISABLED,
    /// The specified pointer is invalid. It points the address of non-allocated
    /// and/or non-executable region.
    MH_ERROR_NOT_EXECUTABLE,
    /// The specified target function cannot be hooked.
    MH_ERROR_UNSUPPORTED_FUNCTION,
    /// Failed to allocate memory.
    MH_ERROR_MEMORY_ALLOC,
    /// Failed to change the memory protection.
    MH_ERROR_MEMORY_PROTECT,
    /// The specified module is not loaded.
    MH_ERROR_MODULE_NOT_FOUND,
    /// The specified function is not found.
    MH_ERROR_FUNCTION_NOT_FOUND
}

/// Can be passed as a parameter to `MH_EnableHook`, `MH_DisableHook`,
/// `MH_QueueEnableHook` or `MH_QueueDisableHook`.
pub const MH_ALL_HOOKS: LPVOID = ptr::null_mut();

/// Can be passed as a parameter to `MH_CreateHook` or `MH_CreateHookApi`.
pub const MH_NO_TRAMPOLINE: *mut LPVOID = ptr::null_mut();

extern "system" {
    /// Initialize the MinHook library.
    ///
    /// You must call this function **exactly once** at the beginning of your
    /// program.
    pub fn MH_Initialize() -> MH_STATUS;

    /// Uninitialize the MinHook library.
    ///
    /// You must call this function **exactly once** at the end of your program.
    pub fn MH_Uninitialize() -> MH_STATUS;

    /// Creates a Hook for the specified target function, in disabled state.
    ///
    /// # Arguments
    /// * `pTarget`    - A pointer to the target function, which will be
    ///                  overridden by the detour function.
    /// * `pDetour`    - A pointer to the detour function, which will override
    ///                  the target function.
    /// * `ppOriginal` - A pointer to the trampoline function, which will be
    ///                  used to call the original target function.
    ///                  This parameter can be `MH_NO_TRAMPOLINE`.
    pub fn MH_CreateHook(pTarget: LPVOID, pDetour: LPVOID, ppOriginal: *mut LPVOID) -> MH_STATUS;

    /// Creates a Hook for the specified API function, in disabled state.
    ///
    /// # Arguments
    /// * `pszModule`  - A pointer to the loaded module name which contains the
    ///                  target function.
    /// * `pszTarget`  - A pointer to the target function name, which will be
    ///                  overridden by the detour function.
    /// * `pDetour`    - A pointer to the detour function, which will override
    ///                  the target function.
    /// * `ppOriginal` - A pointer to the trampoline function, which will be
    ///                  used to call the original target function.
    ///                  This parameter can be `MH_NO_TRAMPOLINE`.
    pub fn MH_CreateHookApi(pszModule: LPCWSTR, pszProcName: LPCSTR, pDetour: LPVOID,
                            ppOriginal: *mut LPVOID)
                            -> MH_STATUS;

    /// Removes an already created hook.
    ///
    /// # Arguments
    /// * `pTarget` - A pointer to the target function.
    pub fn MH_RemoveHook(pTarget: LPVOID) -> MH_STATUS;

    /// Enables an already created hook.
    ///
    /// # Arguments
    /// * `pTarget` - A pointer to the target function.
    ///               If this parameter is `MH_ALL_HOOKS`, all created hooks are
    ///               enabled in one go.
    pub fn MH_EnableHook(pTarget: LPVOID) -> MH_STATUS;

    /// Disables an already created hook.
    ///
    /// # Arguments
    /// * `pTarget` - A pointer to the target function.
    ///               If this parameter is `MH_ALL_HOOKS`, all created hooks are
    ///               disabled in one go.
    pub fn MH_DisableHook(pTarget: LPVOID) -> MH_STATUS;

    /// Queues to enable an already created hook.
    ///
    /// # Arguments
    /// * `pTarget` - A pointer to the target function.
    ///               If this parameter is `MH_ALL_HOOKS`, all created hooks are
    ///               queued to be enabled.
    pub fn MH_QueueEnableHook(pTarget: LPVOID) -> MH_STATUS;

    /// Queues to disable an already created hook.
    ///
    /// # Arguments
    /// * `pTarget` - A pointer to the target function.
    ///               If this parameter is `MH_ALL_HOOKS`, all created hooks are
    ///               queued to be disabled.
    pub fn MH_QueueDisableHook(pTarget: LPVOID) -> MH_STATUS;

    /// Applies all queued changes in one go.
    pub fn MH_ApplyQueued() -> MH_STATUS;
}
