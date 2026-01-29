//! Cache operation implementation for ARM64 architecture
//!
//! This module provides data cache operation functions for the ARM64 architecture.
//! Implemented using the `dcache_range` function from the `aarch64_cpu_ext` crate.

use core::ptr::NonNull;

use aarch64_cpu_ext::cache::{dcache_range, CacheOp};

/// Flush data cache
///
/// Writes back the data cache for the specified address range to memory, without invalidating the cache.
///
/// # Parameters
///
/// * `addr` - Starting address for cache flush
/// * `size` - Size in bytes to flush
pub fn flush(addr: NonNull<u8>, size: usize) {
    dcache_range(CacheOp::Clean, addr.as_ptr() as _, size);
}

/// Invalidate data cache
///
/// Invalidates the data cache for the specified address range, ensuring data is reloaded from memory on the next access.
///
/// # Parameters
///
/// * `addr` - Starting address for cache invalidation
/// * `size` - Size in bytes to invalidate
pub fn invalidate(addr: NonNull<u8>, size: usize) {
    dcache_range(CacheOp::Invalidate, addr.as_ptr() as _, size);
}
