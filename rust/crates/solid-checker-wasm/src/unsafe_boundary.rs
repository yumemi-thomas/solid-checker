//! The only module of this crate allowed to use `unsafe`.
//!
//! The WASI reactor ABI is foreign by construction: the host calls the
//! exported symbols below, and the owned check calls the host's
//! `solid_typefacts` imports. Exports keep their safe signatures and only
//! carry the `no_mangle` attribute; imports are wrapped in safe functions
//! that derive every pointer and length from a Rust slice or reference.
#![allow(unsafe_code)]

/// Reactor exports. Each forwards to the safe implementation in the crate root.
#[cfg(feature = "reactor")]
mod exports {
    /// Allocate the request buffer used by the process-free WASI reactor.
    #[unsafe(no_mangle)]
    pub extern "C" fn allocate_input(size: u32) -> u32 {
        crate::reactor_allocate_input(size)
    }

    /// Plan the exact Type Facts demands for the encoded source request.
    #[unsafe(no_mangle)]
    pub extern "C" fn run_plan() -> u32 {
        crate::reactor_run_plan()
    }

    /// Analyze the encoded source and Type Facts request.
    #[unsafe(no_mangle)]
    pub extern "C" fn run_check() -> u32 {
        crate::reactor_run_check()
    }

    /// Check a project from its files alone, reaching the canonical producer
    /// through the host's owned reactor.
    #[cfg(target_family = "wasm")]
    #[unsafe(no_mangle)]
    pub extern "C" fn run_owned_check() -> u32 {
        crate::reactor_run_owned_check()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn output_pointer() -> u32 {
        crate::reactor_output_pointer()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn output_length() -> u32 {
        crate::reactor_output_length()
    }
}

/// Safe wrappers over the host's `solid_typefacts` imports (S1 of the WASM
/// host protocol). Every call is synchronous and the host touches this
/// module's memory only at the pointers given, during the call.
#[cfg(all(feature = "reactor", target_family = "wasm"))]
pub(crate) mod host {
    #[link(wasm_import_module = "solid_typefacts")]
    unsafe extern "C" {
        fn open(envelope: *const u8, length: u32, handle: *mut u32) -> u32;
        fn handshake(handle: u32) -> u32;
        fn exchange(handle: u32, frame: *const u8, length: u32) -> u32;
        fn result_length(handle: u32) -> u32;
        fn copy_result(handle: u32, destination: *mut u8, capacity: u32) -> u32;
        fn close(handle: u32) -> u32;
    }

    /// Opens one program from the CBOR envelope; returns the status and the
    /// handle the host wrote.
    pub(crate) fn open_program(envelope: &[u8]) -> Result<(u32, u32), &'static str> {
        let length = u32::try_from(envelope.len()).map_err(|_| "open envelope too large")?;
        let mut handle = 0_u32;
        // SAFETY: the host reads `length` bytes at `envelope` and writes four
        // bytes at `handle`, during this call only.
        let status = unsafe { open(envelope.as_ptr(), length, &raw mut handle) };
        Ok((status, handle))
    }

    pub(crate) fn start(handle: u32) -> u32 {
        // SAFETY: a plain call into the host; no memory is shared.
        unsafe { handshake(handle) }
    }

    pub(crate) fn send_frame(handle: u32, frame: &[u8]) -> Result<u32, &'static str> {
        let length = u32::try_from(frame.len()).map_err(|_| "request frame too large")?;
        // SAFETY: the host reads `length` bytes at `frame` during this call
        // only.
        Ok(unsafe { exchange(handle, frame.as_ptr(), length) })
    }

    pub(crate) fn pending_result_length(handle: u32) -> u32 {
        // SAFETY: a plain call into the host; no memory is shared.
        unsafe { result_length(handle) }
    }

    /// Fills `destination` with the pending result and returns the status.
    pub(crate) fn copy_pending_result(
        handle: u32,
        destination: &mut [u8],
    ) -> Result<u32, &'static str> {
        let capacity = u32::try_from(destination.len()).map_err(|_| "result too large")?;
        // SAFETY: the host writes at most `capacity` bytes, the length of
        // `destination`, during this call only.
        Ok(unsafe { copy_result(handle, destination.as_mut_ptr(), capacity) })
    }

    pub(crate) fn close_program(handle: u32) {
        // SAFETY: a plain call into the host; no memory is shared.
        unsafe { close(handle) };
    }
}
