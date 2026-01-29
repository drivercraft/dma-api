//! DMA-compatible heap-allocated vector
//!
//! This module provides the `DVec<T>` type for allocating a DMA-compatible vector (contiguous memory array) on the heap.
//! It automatically handles memory mapping, DMA mask checking, and cache coherence operations.
//!
//! # Example
//!
//! ```
//! use dma_api::*;
//!
//! init(&NopOsal);
//!
//! // Create a zero-initialized DMA vector
//! let mut dvec = DVec::zeros(u64::MAX, 10, 0x1000, Direction::ToDevice).unwrap();
//! dvec.set(0, 42);
//!
//! // Read data
//! let value = dvec.get(0).unwrap();
//! assert_eq!(value, 42);
//!
//! // Convert to normal Vec
//! let normal_vec = dvec.to_vec();
//! ```

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::{alloc::Layout, mem::size_of, ops::Index};

use super::DCommon;
use crate::{dma::alloc::DError, Direction};

/// DMA-compatible heap-allocated vector
///
/// This type allocates a contiguous element array on the heap and provides memory management
/// functionality required for DMA transfers. It automatically handles virtual to physical
/// address mapping, DMA mask checking, and cache coherence operations.
///
/// # Type Parameters
///
/// * `T` - Type of the vector elements
///
/// # Example
///
/// ```
/// // Create a zero-initialized vector
/// let mut dvec = DVec::zeros(u64::MAX, 10, 0x1000, Direction::ToDevice).unwrap();
///
/// // Set elements
/// dvec.set(0, 1);
/// dvec.set(1, 2);
///
/// // Read elements
/// assert_eq!(dvec[0], 1);
/// assert_eq!(dvec.get(0), Some(1));
///
/// // Get bus address
/// let addr = dvec.bus_addr();
/// ```
pub struct DVec<T> {
    inner: DCommon<T>,
}

impl<T> DVec<T> {
    const T_SIZE: usize = size_of::<T>();

    /// Create a zero-initialized DMA vector
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask, specifying the address range the device can access
    /// * `len` - Number of vector elements
    /// * `align` - Alignment in bytes, must be a power of 2
    /// * `direction` - DMA transfer direction
    ///
    /// # Returns
    ///
    /// Returns the initialized `DVec`, or an error on failure
    ///
    /// # Errors
    ///
    /// * `DError::LayoutError` - Invalid layout parameter
    /// * `DError::NoMemory` - Memory allocation failed
    /// * `DError::DmaMaskNotMatch` - Allocated address exceeds DMA mask range
    pub fn zeros(
        dma_mask: u64,
        len: usize,
        align: usize,
        direction: Direction,
    ) -> Result<Self, DError> {
        let size = len * size_of::<T>();
        let layout = Layout::from_size_align(size, align)?;

        Ok(Self {
            inner: DCommon::zeros(dma_mask, layout, direction)?,
        })
    }

    /// Create a DMA vector from an existing Vec
    ///
    /// This method takes ownership of the Vec and converts it to a DMA-compatible vector.
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask, specifying the address range the device can access
    /// * `value` - The Vec to convert
    /// * `direction` - DMA transfer direction
    ///
    /// # Returns
    ///
    /// Returns the converted `DVec`, or an error on failure
    pub fn from_vec(dma_mask: u64, value: Vec<T>, direction: Direction) -> Result<Self, DError> {
        Ok(Self {
            inner: DCommon::from_vec(dma_mask, value, direction)?,
        })
    }

    /// Convert the DMA vector to a normal Vec
    ///
    /// This method takes ownership of the DMA vector and converts it to a normal Rust Vec.
    /// The read operation is automatically prepared and DMA mapping is released before conversion.
    ///
    /// # Returns
    ///
    /// Returns the converted normal Vec
    pub fn to_vec(mut self) -> Vec<T> {
        unsafe {
            self.inner
                .prepare_read(self.inner.addr.cast(), self.inner.layout.size());
            crate::unmap(self.inner.addr.cast(), self.inner.layout.size());
            let len = self.len();

            self.inner.layout = Layout::from_size_align_unchecked(0, 0x1000);
            Vec::from_raw_parts(self.inner.addr.as_ptr(), len, len)
        }
    }

    /// Returns the number of elements in the vector
    pub fn len(&self) -> usize {
        self.inner.layout.size() / size_of::<T>()
    }

    /// Checks if the vector is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the mapped bus address (physical address)
    ///
    /// This address can be directly passed to the DMA controller
    pub fn bus_addr(&self) -> u64 {
        self.inner.bus_addr
    }

    /// Get the element at the specified index
    ///
    /// This method uses volatile read to ensure the latest data is fetched from memory,
    /// and performs necessary cache invalidation operations according to the DMA direction.
    ///
    /// # Parameters
    ///
    /// * `index` - Element index
    ///
    /// # Returns
    ///
    /// Returns `Some(T)` if the index is valid, otherwise returns `None`
    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }

        unsafe {
            let ptr = self.inner.addr.add(index);

            self.inner.prepare_read(ptr.cast(), Self::T_SIZE);

            Some(ptr.read_volatile())
        }
    }

    /// Set the element at the specified index
    ///
    /// This method uses volatile write to ensure data is immediately written to memory,
    /// and performs necessary cache flush operations according to the DMA direction.
    ///
    /// # Parameters
    ///
    /// * `index` - Element index
    /// * `value` - The value to write
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds
    pub fn set(&mut self, index: usize, value: T) {
        assert!(
            index < self.len(),
            "index out of range, index: {},len: {}",
            index,
            self.len()
        );

        unsafe {
            let ptr = self.inner.addr.add(index);

            ptr.write_volatile(value);

            self.inner.confirm_write(ptr.cast(), Self::T_SIZE);
        }
    }

    fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.inner.addr.as_ptr(), self.len()) }
    }

    /// Confirm write of the entire vector
    ///
    /// This method performs necessary cache flush operations on the entire vector.
    pub fn confirm_write_all(&self) {
        self.inner.confirm_write_all();
    }

    /// Returns a raw pointer to the data
    ///
    /// Note: This pointer can be directly used for DMA operations, but direct use is not recommended.
    pub fn as_ptr(&self) -> *mut T {
        self.inner.addr.as_ptr()
    }

    /// Prepare to read the entire vector
    ///
    /// This method performs necessary cache invalidation operations on the entire vector.
    pub fn prepare_read_all(&self) {
        self.inner
            .prepare_read(self.inner.addr.cast(), self.inner.layout.size());
    }
}

/// Supports accessing elements using the index operator `[]`
///
/// Note: This method returns a temporary reference that cannot be stored long-term.
/// For long-term usage, use the `get()` method instead.
impl<T> Index<usize> for DVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len());

        let ptr = unsafe { self.inner.addr.add(index) };

        self.inner.prepare_read(ptr.cast(), Self::T_SIZE);

        unsafe { &*ptr.as_ptr() }
    }
}

/// Copy data from a regular slice to the DMA vector
///
/// This method copies data from the source slice to the DMA vector and automatically flushes the cache.
///
/// # Type Parameters
///
/// * `T` - Element type, must implement the `Copy` trait
///
/// # Parameters
///
/// * `src` - Source slice
///
/// # Panics
///
/// Panics if the source slice length exceeds the DMA vector length
impl<T: Copy> DVec<T> {
    pub fn copy_from_slice(&mut self, src: &[T]) {
        assert!(src.len() <= self.len());

        self.as_slice_mut().copy_from_slice(src);

        self.inner.confirm_write_all();
    }
}

/// Convert DVec to a Rust slice reference
///
/// Automatically prepares the read operation (cache invalidation) before conversion.
impl<T> AsRef<[T]> for DVec<T> {
    fn as_ref(&self) -> &[T] {
        self.inner
            .prepare_read(self.inner.addr.cast(), self.inner.layout.size());
        unsafe { core::slice::from_raw_parts(self.inner.addr.as_ptr(), self.len()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init, Direction, NopOsal};

    #[test]
    fn test_dvec_zeros() {
        init(&NopOsal);

        let dvec = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::ToDevice).unwrap();

        assert_eq!(dvec.len(), 10);
        assert!(!dvec.is_empty());
        assert!(dvec.bus_addr() != 0);
    }

    #[test]
    fn test_dvec_from_vec() {
        init(&NopOsal);

        let vec = vec![1u32, 2, 3, 4, 5];
        let dvec = DVec::from_vec(u64::MAX, vec, Direction::Bidirectional).unwrap();

        assert_eq!(dvec.len(), 5);
        assert!(!dvec.is_empty());
    }

    #[test]
    fn test_dvec_to_vec() {
        init(&NopOsal);

        let dvec = DVec::<u32>::zeros(u64::MAX, 5, 4, Direction::Bidirectional).unwrap();
        dvec.set(0, 10);
        dvec.set(1, 20);
        dvec.set(2, 30);

        let vec = dvec.to_vec();

        assert_eq!(vec.len(), 5);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);
    }

    #[test]
    fn test_dvec_get_and_set() {
        init(&NopOsal);

        let mut dvec = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::Bidirectional).unwrap();

        dvec.set(0, 42);
        dvec.set(5, 100);

        assert_eq!(dvec.get(0), Some(42));
        assert_eq!(dvec.get(5), Some(100));
        assert_eq!(dvec.get(10), None);
    }

    #[test]
    fn test_dvec_index_access() {
        init(&NopOsal);

        let mut dvec = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::Bidirectional).unwrap();

        dvec.set(0, 1);
        dvec.set(1, 2);
        dvec.set(2, 3);

        assert_eq!(dvec[0], 1);
        assert_eq!(dvec[1], 2);
        assert_eq!(dvec[2], 3);
    }

    #[test]
    fn test_dvec_empty() {
        init(&NopOsal);

        let dvec = DVec::<u32>::zeros(u64::MAX, 0, 4, Direction::Bidirectional).unwrap();

        assert_eq!(dvec.len(), 0);
        assert!(dvec.is_empty());
    }

    #[test]
    fn test_dvec_prepare_read_all() {
        init(&NopOsal);

        let dvec = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::FromDevice).unwrap();

        // Should not panic
        dvec.prepare_read_all();
    }

    #[test]
    fn test_dvec_confirm_write_all() {
        init(&NopOsal);

        let dvec = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::ToDevice).unwrap();

        // Should not panic
        dvec.confirm_write_all();
    }

    #[test]
    fn test_dvec_as_ptr() {
        init(&NopOsal);

        let dvec = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::Bidirectional).unwrap();

        let ptr = dvec.as_ptr();

        assert!(!ptr.is_null());
    }

    #[test]
    fn test_dvec_copy_from_slice() {
        init(&NopOsal);

        let mut dvec = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::ToDevice).unwrap();

        let src = [1u32, 2, 3, 4, 5];
        dvec.copy_from_slice(&src);

        assert_eq!(dvec[0], 1);
        assert_eq!(dvec[1], 2);
        assert_eq!(dvec[2], 3);
        assert_eq!(dvec[3], 4);
        assert_eq!(dvec[4], 5);
    }

    #[test]
    fn test_dvec_as_ref() {
        init(&NopOsal);

        let mut dvec = DVec::<u32>::zeros(u64::MAX, 5, 4, Direction::Bidirectional).unwrap();

        dvec.set(0, 10);
        dvec.set(1, 20);
        dvec.set(2, 30);

        let slice: &[u32] = dvec.as_ref();

        assert_eq!(slice.len(), 5);
        assert_eq!(slice[0], 10);
        assert_eq!(slice[1], 20);
        assert_eq!(slice[2], 30);
    }

    #[test]
    fn test_dvec_directions() {
        init(&NopOsal);

        let _to_device = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::ToDevice).unwrap();
        let _from_device = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::FromDevice).unwrap();
        let _bidirectional = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::Bidirectional).unwrap();

        // All should work without panicking
    }

    #[test]
    fn test_dvec_different_alignments() {
        init(&NopOsal);

        let _align4 = DVec::<u32>::zeros(u64::MAX, 10, 4, Direction::Bidirectional).unwrap();
        let _align8 = DVec::<u32>::zeros(u64::MAX, 10, 8, Direction::Bidirectional).unwrap();
        let _align16 = DVec::<u32>::zeros(u64::MAX, 10, 16, Direction::Bidirectional).unwrap();

        // All should work without panicking
    }
}
