//! Empty cache operation implementation
//!
//! This module provides empty cache operation functions for other unsupported architectures.
//! These functions perform no operations and only provide placeholder implementations.
//!
//! # Notes
//!
//! This implementation performs no actual operations and is only for compilation to pass.
//! For actual use, you need to provide the correct cache operation implementation based on the target architecture.

use core::ptr::NonNull;

/// Empty cache flush function
///
/// This function performs no operations.
///
/// # Notes
///
/// Only used as a placeholder implementation for other architectures. For actual use, you need to provide the correct cache flush functionality.
pub fn flush(_addr: NonNull<u8>, _size: usize) {}

/// Empty cache invalidation function
///
/// This function performs no operations.
///
/// # Notes
///
/// Only used as a placeholder implementation for other architectures. For actual use, you need to provide the correct cache invalidation functionality.
pub fn invalidate(_addr: NonNull<u8>, _size: usize) {}
