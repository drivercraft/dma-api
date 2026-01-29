//! DMA-related types and implementations
//!
//! This module provides core types and implementations for DMA (Direct Memory Access) operations, including:
//! - DMA buffer slices (`DSlice` and `DSliceMut`)
//! - DMA memory allocation and containers (requires `alloc` feature)
//! - Extended methods for DMA transfer direction
//!
//! # Examples
//!
//! ```
//! use dma_api::*;
//!
//! init(&NopOsal);
//!
//! // Create read-only DMA buffer
//! let data = [1u32, 2, 3, 4];
//! let slice = DSlice::from(&data, Direction::ToDevice);
//!
//! // Create mutable DMA buffer
//! let mut buffer = [0u32; 4];
//! let mut slice_mut = DSliceMut::from(&mut buffer, Direction::FromDevice);
//! ```

use crate::{flush, invalidate, Direction};
use core::ptr::NonNull;

#[cfg(feature = "alloc")]
pub mod alloc;
pub mod slice;

impl Direction {
    /// Prepare for read operation
    ///
    /// For `FromDevice` or `Bidirectional` directions, this method invalidates cache
    /// to ensure CPU can read latest data written by device.
    ///
    /// # Parameters
    ///
    /// * `ptr` - Starting address of buffer
    /// * `size` - Size of buffer
    pub fn prepare_read(self, ptr: NonNull<u8>, size: usize) {
        if matches!(self, Direction::FromDevice | Direction::Bidirectional) {
            invalidate(ptr, size);
        }
    }
    /// Confirm write operation
    ///
    /// For `ToDevice` or `Bidirectional` directions, this method flushes cache
    /// to ensure data is written back to memory, allowing device to read correct data.
    ///
    /// # Parameters
    ///
    /// * `ptr` - Starting address of buffer
    /// * `size` - Size of buffer
    pub fn confirm_write(self, ptr: NonNull<u8>, size: usize) {
        if matches!(self, Direction::ToDevice | Direction::Bidirectional) {
            flush(ptr, size)
        }
    }
}
