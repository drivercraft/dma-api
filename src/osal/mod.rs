//! Operating System Abstraction Layer (OSAL)
//!
//! This module provides the default OSAL implementation and architecture-specific cache operations.
//!
//! # Architecture Support
//!
//! - `aarch64`: Cache operation implementation for ARM64 architecture
//! - Other architectures: Provides empty placeholder implementation (nop)
//!
//! # NopOsal
//!
//! `NopOsal` is an empty OSAL implementation, only used for compilation checks and testing.
//! All methods call `unimplemented!()` and cannot be used in actual DMA operations.
//! Users need to implement their own `Osal` trait and initialize it via `dma_api::init()`.

use crate::Osal;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        #[path = "aarch64.rs"]
        pub mod arch;
    } else{
        #[path = "nop.rs"]
        pub mod arch;
    }
}

/// Simple OSAL implementation for testing
///
/// This type provides a basic `Osal` trait implementation for testing and doctests.
/// It uses a simple identity mapping (virtual address = physical address).
///
/// # Notes
///
/// This implementation is only suitable for testing and doctests. For actual use,
/// you must implement your own `Osal` trait with proper memory management and initialize it via `dma_api::init()`.
pub struct NopOsal;

#[allow(unused_variables)]
impl Osal for NopOsal {
    fn map(&self, addr: core::ptr::NonNull<u8>, size: usize, direction: crate::Direction) -> u64 {
        addr.as_ptr() as _
    }

    fn unmap(&self, addr: core::ptr::NonNull<u8>, size: usize) {
        // No-op for simple testing
    }

    fn flush(&self, addr: core::ptr::NonNull<u8>, size: usize) {
        // No-op for simple testing
    }

    fn invalidate(&self, addr: core::ptr::NonNull<u8>, size: usize) {
        // No-op for simple testing
    }
}
