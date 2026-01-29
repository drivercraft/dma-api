//! DMA memory allocation module
//!
//! This module provides DMA-compatible memory allocation and container types:
//! - `DBox<T>`: DMA-compatible heap-allocated single value
//! - `DVec<T>`: DMA-compatible heap-allocated vector
//! - `DVecPool`: Memory pool for DMA vectors for efficient reuse of allocated memory
//! - `DError`: Error types related to DMA operations
//!
//! # Examples
//!
//! ```
//! use dma_api::*;
//!
//! // Create zero-initialized DMA vector
//! let mut dvec = DVec::zeros(u64::MAX, 10, 0x1000, Direction::ToDevice).unwrap();
//! dvec.set(0, 42);
//!
//! // Create DMA box
//! let mut dbox = DBox::zero(u64::MAX, Direction::FromDevice).unwrap();
//! dbox.write(123);
//! ```

use alloc::vec::Vec;
use core::{
    alloc::Layout,
    ptr::{slice_from_raw_parts_mut, NonNull},
};

use crate::{flush, map, unmap, Direction};

pub mod r#box;
pub mod pool;
pub mod vec;

/// DMA operation error type
///
/// This enum defines various errors that may occur during DMA operations.
#[derive(thiserror::Error, Debug, Clone)]
pub enum DError {
    /// DMA mask mismatch
    ///
    /// The allocated physical address exceeds the DMA address range accessible by the device.
    #[error("DMA mask not match, required {mask:#x}, got {got:#x}")]
    DmaMaskNotMatch { mask: u64, got: u64 },
    /// Out of memory
    ///
    /// Unable to allocate the required DMA memory.
    #[error("No memory")]
    NoMemory,
    /// Memory layout error
    ///
    /// The provided memory layout parameters are invalid (e.g., size or alignment are invalid).
    #[error("Layout error")]
    LayoutError,
}

impl From<core::alloc::LayoutError> for DError {
    fn from(_: core::alloc::LayoutError) -> Self {
        DError::LayoutError
    }
}

/// Common implementation for DMA memory blocks
///
/// This is an internal structure that provides core functionality for all DMA memory types.
///
/// # Fields
///
/// * `addr` - Virtual address pointer
/// * `bus_addr` - Mapped physical address (bus address)
/// * `layout` - Memory layout information
/// * `direction` - DMA transfer direction
struct DCommon<T> {
    addr: NonNull<T>,
    bus_addr: u64,
    layout: Layout,
    direction: Direction,
}

unsafe impl<T: Send> Send for DCommon<T> {}

impl<T> DCommon<T> {
    /// Create a zero-initialized DMA memory block
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask, specifying the address range device can access
    /// * `layout` - Memory layout
    /// * `direction` - DMA transfer direction
    ///
    /// # Returns
    ///
    /// Returns the initialized `DCommon` instance, or an error on failure
    ///
    /// # Errors
    ///
    /// * `DError::NoMemory` - Memory allocation failed
    /// * `DError::DmaMaskNotMatch` - Allocated address exceeds DMA mask range
    pub fn zeros(dma_mask: u64, layout: Layout, direction: Direction) -> Result<Self, DError> {
        unsafe {
            let mut addr = NonNull::new(crate::alloc(dma_mask, layout)).ok_or(DError::NoMemory)?;
            (*slice_from_raw_parts_mut(addr.as_mut(), layout.size())).fill(0);

            let bus_addr = map(addr, layout.size(), direction);
            if let Err(e) = Self::check_dma_mask(dma_mask, bus_addr) {
                crate::dealloc(addr.as_ptr() as _, layout);
                return Err(e);
            }
            flush(addr, layout.size());
            Ok(Self {
                bus_addr,
                addr: addr.cast(),
                layout,
                direction,
            })
        }
    }

    /// Check if bus address meets DMA mask requirements
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask
    /// * `bus_addr` - Bus address to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if address meets requirements, otherwise returns an error
    fn check_dma_mask(dma_mask: u64, bus_addr: u64) -> Result<(), DError> {
        if (bus_addr) & (dma_mask) != (bus_addr) {
            return Err(DError::DmaMaskNotMatch {
                mask: dma_mask,
                got: bus_addr,
            });
        }
        Ok(())
    }

    /// Create a DMA memory block from an existing Vec
    ///
    /// This method takes ownership of the Vec and converts it to a DMA-compatible memory block.
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask, specifying the address range device can access
    /// * `value` - The Vec to convert
    /// * `direction` - DMA transfer direction
    ///
    /// # Returns
    ///
    /// Returns the converted `DCommon` instance, or an error on failure
    pub fn from_vec(
        dma_mask: u64,
        mut value: Vec<T>,
        direction: Direction,
    ) -> Result<Self, DError> {
        unsafe {
            let layout = Layout::from_size_align_unchecked(
                value.capacity() * size_of::<T>(),
                align_of::<T>(),
            );

            let addr = NonNull::new(value.as_mut_ptr()).unwrap();

            let bus_addr = map(addr.cast(), layout.size(), direction);
            Self::check_dma_mask(dma_mask, bus_addr)?;

            core::mem::forget(value);

            flush(addr.cast(), layout.size());
            Ok(Self {
                bus_addr,
                addr: addr.cast(),
                layout,
                direction,
            })
        }
    }

    /// Prepare to read data in the specified range
    ///
    /// Performs necessary cache invalidation operations according to the DMA direction.
    ///
    /// # Parameters
    ///
    /// * `ptr` - Starting address of the data to read
    /// * `size` - Size of the data to read
    pub fn prepare_read(&self, ptr: NonNull<u8>, size: usize) {
        self.direction.prepare_read(ptr, size);
    }

    /// Confirm write of data in the specified range
    ///
    /// Performs necessary cache flush operations according to the DMA direction.
    ///
    /// # Parameters
    ///
    /// * `ptr` - Starting address of the written data
    /// * `size` - Size of the written data
    pub fn confirm_write(&self, ptr: NonNull<u8>, size: usize) {
        self.direction.confirm_write(ptr, size);
    }

    /// Confirm write of the entire memory block
    ///
    /// Performs necessary cache flush operations on the entire memory block according to the DMA direction.
    pub fn confirm_write_all(&self) {
        self.direction
            .confirm_write(self.addr.cast(), self.layout.size());
    }
}

/// Release DMA memory resources
///
/// Automatically unmaps the address and frees the memory when `DCommon` is dropped.
impl<T> Drop for DCommon<T> {
    fn drop(&mut self) {
        if self.layout.size() > 0 {
            unmap(self.addr.cast(), self.layout.size());

            crate::dealloc(self.addr.as_ptr() as _, self.layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init, Direction, NopOsal};

    #[test]
    fn test_derror_variants() {
        let dma_mask_error = DError::DmaMaskNotMatch;
        let no_memory_error = DError::NoMemory;
        let layout_error = DError::LayoutError;

        assert!(matches!(dma_mask_error, DError::DmaMaskNotMatch));
        assert!(matches!(no_memory_error, DError::NoMemory));
        assert!(matches!(layout_error, DError::LayoutError));
    }

    #[test]
    fn test_dcommon_check_dma_mask() {
        // Test with bus_addr that fits within mask
        assert!(DCommon::<u32>::check_dma_mask(0xFFFFFFFFFFFF, 0x1000).is_ok());

        // Test with bus_addr that exceeds mask
        assert!(DCommon::<u32>::check_dma_mask(0xFFFF, 0x10000).is_err());
    }

    #[test]
    fn test_dcommon_zeros() {
        init(&NopOsal);

        let layout = Layout::new::<[u32; 4]>();
        let result = DCommon::<u32>::zeros(u64::MAX, layout, Direction::ToDevice);

        assert!(result.is_ok());
        let common = result.unwrap();

        // Check that the structure has valid values
        assert_eq!(common.layout.size(), 16); // 4 * 4 bytes
        assert!(common.addr.as_ptr() as usize != 0);
        assert!(common.bus_addr != 0);
    }

    #[test]
    fn test_dcommon_from_vec() {
        init(&NopOsal);

        let vec = vec![1u32, 2, 3, 4];
        let result = DCommon::from_vec(u64::MAX, vec, Direction::Bidirectional);

        assert!(result.is_ok());
        let common = result.unwrap();

        assert_eq!(common.layout.size(), 16); // 4 * 4 bytes
        assert!(common.addr.as_ptr() as usize != 0);
    }

    #[test]
    fn test_dcommon_prepare_read() {
        init(&NopOsal);

        let layout = Layout::new::<[u32; 4]>();
        let common = DCommon::<u32>::zeros(u64::MAX, layout, Direction::FromDevice).unwrap();

        // Should not panic with NopOsal
        common.prepare_read(common.addr.cast(), 4);
    }

    #[test]
    fn test_dcommon_confirm_write() {
        init(&NopOsal);

        let layout = Layout::new::<[u32; 4]>();
        let common = DCommon::<u32>::zeros(u64::MAX, layout, Direction::ToDevice).unwrap();

        // Should not panic with NopOsal
        common.confirm_write(common.addr.cast(), 4);
    }

    #[test]
    fn test_dcommon_confirm_write_all() {
        init(&NopOsal);

        let layout = Layout::new::<[u32; 4]>();
        let common = DCommon::<u32>::zeros(u64::MAX, layout, Direction::ToDevice).unwrap();

        // Should not panic with NopOsal
        common.confirm_write_all();
    }

    #[test]
    fn test_dcommon_direction_variants() {
        init(&NopOsal);

        let layout = Layout::new::<[u32; 4]>();

        let _to_device = DCommon::<u32>::zeros(u64::MAX, layout, Direction::ToDevice).unwrap();
        let _from_device = DCommon::<u32>::zeros(u64::MAX, layout, Direction::FromDevice).unwrap();
        let _bidirectional =
            DCommon::<u32>::zeros(u64::MAX, layout, Direction::Bidirectional).unwrap();

        // All should work without panicking
    }

    #[test]
    fn test_dcommon_with_different_alignments() {
        init(&NopOsal);

        // Test with 4-byte alignment
        let layout = Layout::from_size_align(16, 4).unwrap();
        let result = DCommon::<u32>::zeros(u64::MAX, layout, Direction::ToDevice);
        assert!(result.is_ok());

        // Test with 8-byte alignment
        let layout = Layout::from_size_align(16, 8).unwrap();
        let result = DCommon::<u32>::zeros(u64::MAX, layout, Direction::ToDevice);
        assert!(result.is_ok());
    }
}
