//! DMA buffer slices
//!
//! This module provides two types of DMA buffer slices:
//! - `DSlice<'a, T>`: Read-only DMA buffer slice
//! - `DSliceMut<'a, T>`: Mutable DMA buffer slice
//!
//! These types automatically handle cache operations (flush/invalidate) required for DMA transfers
//! and provide bus addresses for device use.
//!
//! # Examples
//!
//! ```
//! use dma_api::*;
//!
//! // Initialize OSAL for doctest
//! init(&NopOsal);
//!
//! // Read-only slice - for data transfer to device
//! let data = [1u32, 2, 3, 4];
//! let slice = DSlice::from(&data, Direction::ToDevice);
//! assert_eq!(slice[0], 1);
//!
//! // Mutable slice - for receiving data from device
//! let mut buffer = [0u32; 4];
//! let mut slice_mut = DSliceMut::from(&mut buffer, Direction::FromDevice);
//! slice_mut.prepare_read_all(); // Prepare for reading from device
//! ```

use core::{
    marker::PhantomData,
    mem::{size_of, size_of_val},
    ops::Index,
    ptr::NonNull,
};

use crate::{flush, map, unmap, Direction};

/// DMA read-only buffer slice
///
/// This type wraps an immutable slice `&'a [T]` and provides memory management functionality
/// required for DMA transfers. It automatically handles virtual-to-physical address mapping
/// and cache flush operations.
///
/// # Type Parameters
///
/// * `'a` - The lifetime of the slice, bound to the lifetime of underlying data
/// * `T` - The type of slice elements, must be `Sized`
///
/// # Examples
///
/// ```
/// use dma_api::*;
///
/// init(&NopOsal);
///
/// let data = [1u32, 2, 3, 4];
/// let slice = DSlice::from(&data, Direction::ToDevice);
///
/// // Get bus address for device use
/// let addr = slice.bus_addr();
///
/// // Access data
/// assert_eq!(slice[0], 1);
/// ```
#[repr(transparent)]
pub struct DSlice<'a, T> {
    inner: DSliceCommon<'a, T>,
}

impl<'a, T> DSlice<'a, T> {
    /// Returns the number of elements in the slice
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns the mapped bus address (physical address)
    ///
    /// This address can be passed directly to DMA controller
    pub fn bus_addr(&self) -> u64 {
        self.inner.bus_addr
    }

    /// Returns whether the slice is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Create a DMA read-only slice from a regular slice
    ///
    /// # Parameters
    ///
    /// * `value` - Source slice reference
    /// * `direction` - DMA transfer direction
    ///
    /// # Examples
    ///
    /// ```
    /// use dma_api::*;
    ///
    /// init(&NopOsal);
    ///
    /// let data = [1u32, 2, 3, 4];
    /// let slice = DSlice::from(&data, Direction::ToDevice);
    /// assert_eq!(slice[0], 1);
    /// ```
    pub fn from(value: &'a [T], direction: Direction) -> Self {
        Self {
            inner: DSliceCommon::new(value, direction),
        }
    }

    /// Prepare to read the entire slice
    ///
    /// For `FromDevice` or `Bidirectional` directions, this method invalidates cache
    /// to ensure CPU can read latest data written by device.
    pub fn prepare_read_all(&self) {
        self.inner.prepare_read_all();
    }

    /// Confirm writing of the entire slice
    ///
    /// For `ToDevice` or `Bidirectional` directions, this method flushes cache
    /// to ensure data is written back to memory.
    pub fn confirm_write_all(&self) {
        self.inner.confirm_write_all();
    }
}

impl<T> Index<usize> for DSlice<'_, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.inner.index(index)
    }
}

impl<T> AsRef<[T]> for DSlice<'_, T> {
    fn as_ref(&self) -> &[T] {
        self.inner.as_ref()
    }
}

/// DMA mutable buffer slice
///
/// This type wraps a mutable slice `&'a mut [T]` and provides memory management
/// functionality required for DMA transfers. In addition to read operations,
/// it supports safe write operations and automatically handles cache coherence.
///
/// # Type Parameters
///
/// * `'a` - The lifetime of slice, bound to lifetime of underlying data
/// * `T` - The type of slice elements, must be `Sized`
///
/// # Examples
///
/// ```
/// use dma_api::*;
///
/// init(&NopOsal);
///
/// let mut buffer = [0u32; 4];
/// let mut slice = DSliceMut::from(&mut buffer, Direction::Bidirectional);
///
/// // Write data (cache is automatically flushed)
/// slice.set(0, 42);
///
/// // Read data
/// let value = slice[0];
/// assert_eq!(value, 42);
/// ```
#[repr(transparent)]
pub struct DSliceMut<'a, T> {
    inner: DSliceCommon<'a, T>,
}

impl<'a, T> DSliceMut<'a, T> {
    /// Create a DMA mutable slice from a regular mutable slice
    ///
    /// # Parameters
    ///
    /// * `value` - Source mutable slice reference
    /// * `direction` - DMA transfer direction
    ///
    /// # Examples
    ///
    /// ```
    /// use dma_api::*;
    ///
    /// init(&NopOsal);
    ///
    /// let mut buffer = [0u32; 4];
    /// let mut slice = DSliceMut::from(&mut buffer, Direction::ToDevice);
    ///
    /// slice.set(0, 42);
    /// slice.confirm_write_all();
    ///
    /// assert_eq!(slice[0], 42);
    /// ```
    pub fn from(value: &'a mut [T], direction: Direction) -> Self {
        Self {
            inner: DSliceCommon::new(value, direction),
        }
    }

    /// Returns the mapped bus address (physical address)
    ///
    /// This address can be passed directly to DMA controller
    pub fn bus_addr(&self) -> u64 {
        self.inner.bus_addr
    }

    /// Returns the number of elements in the slice
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the slice is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Set the value at the specified index
    ///
    /// This method uses volatile write to ensure data is immediately written to memory,
    /// and flushes cache based on DMA direction.
    ///
    /// # Parameters
    ///
    /// * `index` - Element index, must be within valid range
    /// * `value` - The value to write
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of range
    pub fn set(&self, index: usize, value: T) {
        assert!(index < self.len());

        unsafe {
            let ptr = self.inner.addr.add(index);

            ptr.write_volatile(value);

            self.inner
                .direction
                .confirm_write(ptr.cast(), size_of::<T>());
        }
    }

    /// Prepare to read the entire slice
    ///
    /// For `FromDevice` or `Bidirectional` directions, this method invalidates cache
    /// to ensure CPU can read latest data written by device.
    pub fn prepare_read_all(&self) {
        self.inner.prepare_read_all();
    }

    /// Confirm writing of the entire slice
    ///
    /// For `ToDevice` or `Bidirectional` directions, this method flushes cache
    /// to ensure data is written back to memory.
    pub fn confirm_write_all(&self) {
        self.inner.confirm_write_all();
    }
}

impl<T> Index<usize> for DSliceMut<'_, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.inner.index(index)
    }
}

impl<T> AsRef<[T]> for DSliceMut<'_, T> {
    fn as_ref(&self) -> &[T] {
        self.inner.as_ref()
    }
}

struct DSliceCommon<'a, T> {
    addr: NonNull<T>,
    size: usize,
    bus_addr: u64,
    direction: Direction,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> DSliceCommon<'a, T> {
    fn new(s: &'a [T], direction: Direction) -> Self {
        let size = size_of_val(s);
        let ptr = unsafe { NonNull::new_unchecked(s.as_ptr() as usize as *mut T) };
        let bus_addr = map(ptr.cast(), size, direction);

        flush(ptr.cast(), size);

        Self {
            addr: ptr,
            size,
            bus_addr,
            direction,
            _marker: PhantomData,
        }
    }

    fn len(&self) -> usize {
        self.size / size_of::<T>()
    }

    fn index(&self, index: usize) -> &T {
        assert!(index < self.len());

        let ptr = unsafe { self.addr.add(index) };

        self.direction.prepare_read(ptr.cast(), size_of::<T>());

        unsafe { ptr.as_ref() }
    }

    fn prepare_read_all(&self) {
        self.direction.prepare_read(self.addr.cast(), self.size);
    }

    fn confirm_write_all(&self) {
        self.direction.confirm_write(self.addr.cast(), self.size);
    }
}

/// 释放 DMA 映射
///
/// 当 `DSliceCommon` 被销毁时，自动解除虚拟地址到物理地址的映射。
impl<T> Drop for DSliceCommon<'_, T> {
    fn drop(&mut self) {
        unmap(self.addr.cast(), self.size);
    }
}

/// 将 `DSliceCommon` 转换为 Rust 切片引用
///
/// 转换前会自动准备读取操作（无效化缓存）。
impl<T> AsRef<[T]> for DSliceCommon<'_, T> {
    fn as_ref(&self) -> &[T] {
        self.prepare_read_all();
        unsafe { core::slice::from_raw_parts_mut(self.addr.as_ptr(), self.len()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;

    #[test]
    fn test_dslice_creation() {
        init(&crate::NopOsal);

        let data = [1u32, 2, 3, 4];
        let slice = DSlice::from(&data, Direction::ToDevice);

        assert_eq!(slice.len(), 4);
        assert!(!slice.is_empty());
        assert_eq!(slice.bus_addr(), data.as_ptr() as u64);
    }

    #[test]
    fn test_dslice_access() {
        init(&crate::NopOsal);

        let data = [1u32, 2, 3, 4];
        let slice = DSlice::from(&data, Direction::ToDevice);

        assert_eq!(slice[0], 1);
        assert_eq!(slice[1], 2);
        assert_eq!(slice[2], 3);
        assert_eq!(slice[3], 4);
    }

    #[test]
    fn test_dslice_empty() {
        init(&crate::NopOsal);

        let data: [u32; 0] = [];
        let slice = DSlice::from(&data, Direction::ToDevice);

        assert_eq!(slice.len(), 0);
        assert!(slice.is_empty());
    }

    #[test]
    fn test_dslice_mut_creation() {
        init(&crate::NopOsal);

        let mut buffer = [0u32; 4];
        let slice = DSliceMut::from(&mut buffer, Direction::FromDevice);

        assert_eq!(slice.len(), 4);
        assert!(!slice.is_empty());
    }

    #[test]
    fn test_dslice_mut_set() {
        init(&crate::NopOsal);

        let mut buffer = [0u32; 4];
        let slice = DSliceMut::from(&mut buffer, Direction::ToDevice);

        slice.set(0, 42);
        slice.set(1, 43);
        slice.set(2, 44);
        slice.set(3, 45);

        assert_eq!(slice[0], 42);
        assert_eq!(slice[1], 43);
        assert_eq!(slice[2], 44);
        assert_eq!(slice[3], 45);
    }

    #[test]
    fn test_dslice_mut_get() {
        init(&crate::NopOsal);

        let mut buffer = [1u32, 2, 3, 4];
        let slice = DSliceMut::from(&mut buffer, Direction::FromDevice);

        // Use index access for valid indices
        assert_eq!(slice[0], 1);
        assert_eq!(slice[1], 2);
        assert_eq!(slice[2], 3);
        assert_eq!(slice[3], 4);
    }

    #[test]
    fn test_dslice_mut_write_all() {
        init(&crate::NopOsal);

        let mut buffer = [0u32; 4];
        let slice = DSliceMut::from(&mut buffer, Direction::ToDevice);

        for i in 0..4 {
            slice.set(i, (i * 10) as u32);
        }

        assert_eq!(slice[0], 0);
        assert_eq!(slice[1], 10);
        assert_eq!(slice[2], 20);
        assert_eq!(slice[3], 30);
    }

    #[test]
    fn test_dslice_as_ref() {
        init(&crate::NopOsal);

        let data = [1u32, 2, 3, 4];
        let slice = DSlice::from(&data, Direction::ToDevice);

        let slice_ref: &[u32] = slice.as_ref();

        assert_eq!(slice_ref.len(), 4);
        assert_eq!(slice_ref[0], 1);
        assert_eq!(slice_ref[3], 4);
    }

    #[test]
    fn test_dslice_mut_as_ref() {
        init(&crate::NopOsal);

        let mut buffer = [1u32, 2, 3, 4];
        let slice = DSliceMut::from(&mut buffer, Direction::Bidirectional);

        let slice_ref: &[u32] = slice.as_ref();

        assert_eq!(slice_ref.len(), 4);
        assert_eq!(slice_ref[0], 1);
        assert_eq!(slice_ref[3], 4);
    }

    #[test]
    fn test_dslice_directions() {
        init(&crate::NopOsal);

        let data = [1u32, 2, 3, 4];

        // Test all directions
        let _to_device = DSlice::from(&data, Direction::ToDevice);
        let _from_device = DSlice::from(&data, Direction::FromDevice);
        let _bidirectional = DSlice::from(&data, Direction::Bidirectional);

        // All should work without panicking
    }

    #[test]
    fn test_dslice_mut_directions() {
        init(&crate::NopOsal);

        // Test all directions with separate buffers
        let mut buffer1 = [0u32; 4];
        let _to_device = DSliceMut::from(&mut buffer1, Direction::ToDevice);

        let mut buffer2 = [0u32; 4];
        let _from_device = DSliceMut::from(&mut buffer2, Direction::FromDevice);

        let mut buffer3 = [0u32; 4];
        let _bidirectional = DSliceMut::from(&mut buffer3, Direction::Bidirectional);

        // All should work without panicking
    }

    #[test]
    fn test_dslice_mut_confirm_write_all() {
        init(&crate::NopOsal);

        let mut buffer = [0u32; 4];
        let slice = DSliceMut::from(&mut buffer, Direction::ToDevice);

        slice.set(0, 100);
        slice.confirm_write_all();

        assert_eq!(slice[0], 100);
    }

    #[test]
    fn test_dslice_mut_prepare_read_all() {
        init(&crate::NopOsal);

        let mut buffer = [1u32, 2, 3, 4];
        let slice = DSliceMut::from(&mut buffer, Direction::FromDevice);

        slice.prepare_read_all();

        // Should work without panicking
        assert_eq!(slice[0], 1);
    }
}
