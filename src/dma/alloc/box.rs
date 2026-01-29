//! DMA-compatible heap-allocated single value
//!
//! This module provides the `DBox<T>` type for allocating a single DMA-compatible value on the heap.
//! It automatically handles memory mapping, DMA mask checking, and cache coherence operations.
//!
//! # Example
//!
//! ```
//! use dma_api::*;
//!
//! init(&NopOsal);
//!
//! // Create a zero-initialized DMA box
//! let mut dbox = DBox::zero(u64::MAX, Direction::Bidirectional).unwrap();
//!
//! // Write data
//! dbox.write(42);
//!
//! // Read data
//! let value = dbox.read();
//! assert_eq!(value, 42);
//! ```

use core::alloc::Layout;

use crate::{dma::alloc::DError, Direction};

use super::DCommon;

/// DMA-compatible heap-allocated single value
///
/// This type allocates a value of type `T` on the heap and provides memory management
/// functionality required for DMA transfers. It automatically handles virtual to physical
/// address mapping, DMA mask checking, and cache coherence operations.
///
/// # Type Parameters
///
/// * `T` - Type of the value to store
///
/// # Example
///
/// ```
/// use dma_api::*;
///
/// init(&NopOsal);
///
/// // Use default alignment
/// let mut dbox = DBox::zero(u64::MAX, Direction::ToDevice).unwrap();
/// dbox.write(123);
///
/// // Use custom alignment
/// let mut dbox = DBox::zero_with_align(u64::MAX, Direction::FromDevice, 0x1000).unwrap();
/// dbox.write(456);
/// ```
pub struct DBox<T> {
    inner: DCommon<T>,
}

impl<T> DBox<T> {
    const SIZE: usize = core::mem::size_of::<T>();

    /// Create a zero-initialized DMA box with specified alignment
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask, specifying the address range the device can access
    /// * `direction` - DMA transfer direction
    /// * `align` - Alignment in bytes, must be a power of 2
    ///
    /// # Returns
    ///
    /// Returns the initialized `DBox`, or an error on failure
    ///
    /// # Errors
    ///
    /// * `DError::LayoutError` - Invalid alignment parameter
    /// * `DError::NoMemory` - Memory allocation failed
    /// * `DError::DmaMaskNotMatch` - Allocated address exceeds DMA mask range
    pub fn zero_with_align(
        dma_mask: u64,
        direction: Direction,
        align: usize,
    ) -> Result<Self, super::DError> {
        let layout = Layout::from_size_align(Self::SIZE, align)?;

        Ok(Self {
            inner: DCommon::zeros(dma_mask, layout, direction)?,
        })
    }

    /// Create a zero-initialized DMA box with default alignment
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask, specifying the address range the device can access
    /// * `direction` - DMA transfer direction
    ///
    /// # Returns
    ///
    /// Returns the initialized `DBox`, or an error on failure
    pub fn zero(dma_mask: u64, direction: Direction) -> Result<Self, DError> {
        let layout = Layout::new::<T>();
        Ok(Self {
            inner: DCommon::zeros(dma_mask, layout, direction)?,
        })
    }
    /// Returns the mapped bus address (physical address)
    ///
    /// This address can be directly passed to the DMA controller
    pub fn bus_addr(&self) -> u64 {
        self.inner.bus_addr
    }

    /// Read the stored value
    ///
    /// This method uses volatile read to ensure the latest data is fetched from memory,
    /// and performs necessary cache invalidation operations according to the DMA direction.
    ///
    /// # Returns
    ///
    /// Returns the stored value
    pub fn read(&self) -> T {
        unsafe {
            let ptr = self.inner.addr;

            self.inner.prepare_read(ptr.cast(), Self::SIZE);

            ptr.read_volatile()
        }
    }

    /// Write a new value
    ///
    /// This method uses volatile write to ensure data is immediately written to memory,
    /// and performs necessary cache flush operations according to the DMA direction.
    ///
    /// # Parameters
    ///
    /// * `value` - The value to write
    pub fn write(&mut self, value: T) {
        unsafe {
            let ptr = self.inner.addr;

            ptr.write_volatile(value);

            self.inner.confirm_write(ptr.cast(), Self::SIZE);
        }
    }

    /// Modify the stored value
    ///
    /// This method allows in-place modification of the stored value and automatically
    /// handles cache coherence.
    ///
    /// # Parameters
    ///
    /// * `f` - Closure function to modify the value
    ///
    /// # Example
    ///
    /// ```
    /// use dma_api::*;
    ///
    /// init(&NopOsal);
    ///
    /// let mut dbox = DBox::zero(u64::MAX, Direction::Bidirectional).unwrap();
    /// dbox.write(10);
    /// dbox.modify(|val| *val += 5);
    /// assert_eq!(dbox.read(), 15);
    /// ```
    pub fn modify(&mut self, f: impl FnOnce(&mut T)) {
        unsafe {
            let mut ptr = self.inner.addr;

            self.inner.prepare_read(ptr.cast(), Self::SIZE);

            f(ptr.as_mut());

            self.inner.confirm_write(ptr.cast(), Self::SIZE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init, Direction, NopOsal};

    #[test]
    fn test_dbox_creation() {
        init(&NopOsal);

        let dbox = DBox::<u32>::zero(u64::MAX, Direction::ToDevice).unwrap();

        // Should have valid bus address
        assert!(dbox.bus_addr() != 0);
    }

    #[test]
    fn test_dbox_write_and_read() {
        init(&NopOsal);

        let mut dbox = DBox::<u32>::zero(u64::MAX, Direction::Bidirectional).unwrap();

        dbox.write(42);
        let value = dbox.read();

        assert_eq!(value, 42);
    }

    #[test]
    fn test_dbox_modify() {
        init(&NopOsal);

        let mut dbox = DBox::<u32>::zero(u64::MAX, Direction::Bidirectional).unwrap();

        dbox.write(10);
        dbox.modify(|val| *val += 5);

        assert_eq!(dbox.read(), 15);
    }

    #[test]
    fn test_dbox_zero_with_align() {
        init(&NopOsal);

        let dbox = DBox::<u32>::zero_with_align(u64::MAX, Direction::ToDevice, 8).unwrap();

        assert!(dbox.bus_addr() != 0);
    }

    #[test]
    fn test_dbox_directions() {
        init(&NopOsal);

        let _to_device = DBox::<u32>::zero(u64::MAX, Direction::ToDevice).unwrap();
        let _from_device = DBox::<u32>::zero(u64::MAX, Direction::FromDevice).unwrap();
        let _bidirectional = DBox::<u32>::zero(u64::MAX, Direction::Bidirectional).unwrap();

        // All should work without panicking
    }

    #[test]
    fn test_dbox_different_types() {
        init(&NopOsal);

        // Test with u8
        let dbox_u8 = DBox::<u8>::zero(u64::MAX, Direction::Bidirectional).unwrap();
        dbox_u8.write(255);
        assert_eq!(dbox_u8.read(), 255);

        // Test with u32
        let dbox_u32 = DBox::<u32>::zero(u64::MAX, Direction::Bidirectional).unwrap();
        dbox_u32.write(0xDEADBEEF);
        assert_eq!(dbox_u32.read(), 0xDEADBEEF);

        // Test with u64
        let dbox_u64 = DBox::<u64>::zero(u64::MAX, Direction::Bidirectional).unwrap();
        dbox_u64.write(0x123456789ABCDEF0);
        assert_eq!(dbox_u64.read(), 0x123456789ABCDEF0);
    }

    #[test]
    fn test_dbox_zero_initialization() {
        init(&NopOsal);

        let dbox = DBox::<u32>::zero(u64::MAX, Direction::Bidirectional).unwrap();

        // Should be zero-initialized
        assert_eq!(dbox.read(), 0);
    }

    #[test]
    fn test_dbox_multiple_writes() {
        init(&NopOsal);

        let mut dbox = DBox::<u32>::zero(u64::MAX, Direction::Bidirectional).unwrap();

        for i in 0..10 {
            dbox.write(i);
            assert_eq!(dbox.read(), i);
        }
    }
}
