#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{ptr::NonNull, sync::atomic::AtomicBool};

mod dma;
mod osal;

#[cfg(feature = "alloc")]
pub use dma::alloc::{pool::*, r#box::DBox, vec::DVec, DError};

pub use dma::slice::{DSlice, DSliceMut};
pub use osal::NopOsal;

/// DMA transfer direction
///
/// Defines the data flow direction for DMA transfers, used to indicate the type of cache operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum Direction {
    /// Data transfer from memory to device (write to device)
    ToDevice,
    /// Data transfer from device to memory (read from device)
    FromDevice,
    /// Bidirectional transfer, cache operations required for both read and write
    Bidirectional,
}

/// Operating System Abstraction Layer
///
/// This trait defines the low-level operating system interfaces required for DMA operations.
/// Users need to implement this trait to provide concrete memory management functionality,
/// including virtual-to-physical address mapping, cache flush, and invalidation operations.
///
/// # Example
///
/// ```
/// use dma_api::*;
///
/// struct MyOsal;
///
/// impl Osal for MyOsal {
///     fn map(&self, addr: core::ptr::NonNull<u8>, size: usize, direction: Direction) -> u64 {
///         // Implement virtual-to-physical address mapping
///         addr.as_ptr() as usize as u64
///     }
///
///     fn unmap(&self, addr: core::ptr::NonNull<u8>, size: usize) {
///         // Implement address unmapping
///     }
///
///     fn flush(&self, addr: core::ptr::NonNull<u8>, size: usize) {
///         // Implement cache flush
///     }
///
///     fn invalidate(&self, addr: core::ptr::NonNull<u8>, size: usize) {
///         // Implement cache invalidation
///     }
/// }
/// ```
pub trait Osal {
    /// Map virtual address to physical address
    ///
    /// # Parameters
    ///
    /// * `addr` - The virtual address to map
    /// * `size` - The size in bytes to map
    /// * `direction` - The DMA transfer direction
    ///
    /// # Returns
    ///
    /// Returns the mapped physical address (bus address)
    fn map(&self, addr: NonNull<u8>, size: usize, direction: Direction) -> u64;

    /// Unmap virtual address to physical address mapping
    ///
    /// # Parameters
    ///
    /// * `addr` - The virtual address to unmap
    /// * `size` - The size in bytes to unmap
    fn unmap(&self, addr: NonNull<u8>, size: usize);

    /// Write cache back to memory
    ///
    /// This operation ensures that data in the CPU cache has been written to main memory.
    /// It is typically called before DMA write operations to ensure the device can read correct data.
    ///
    /// # Parameters
    ///
    /// * `addr` - The starting address for cache flush
    /// * `size` - The size in bytes to flush
    fn flush(&self, addr: NonNull<u8>, size: usize) {
        osal::arch::flush(addr, size)
    }

    /// Invalidate cache
    ///
    /// This operation makes the data in the CPU cache invalid.
    /// It is typically called before DMA read operations to ensure the CPU can read
    /// the latest data written by the device from memory.
    ///
    /// # Parameters
    ///
    /// * `addr` - The starting address for cache invalidation
    /// * `size` - The size in bytes to invalidate
    fn invalidate(&self, addr: NonNull<u8>, size: usize) {
        osal::arch::invalidate(addr, size)
    }

    /// Allocate memory that meets DMA requirements
    ///
    /// # Parameters
    ///
    /// * `dma_mask` - DMA mask, specifying the physical address range the device can access
    /// * `layout` - Memory layout, specifying required alignment and size
    ///
    /// # Returns
    ///
    /// Returns a pointer to the allocated memory. Returns a null pointer if allocation fails.
    ///
    /// # Safety
    ///
    /// This function is unsafe because undefined behavior can result if the caller does not
    /// properly handle the returned pointer.
    /// The caller must ensure:
    /// - The pointer is eventually deallocated using the corresponding `dealloc` method
    /// - The memory is not accessed after deallocation
    #[cfg(feature = "alloc")]
    unsafe fn alloc(&self, dma_mask: u64, layout: core::alloc::Layout) -> *mut u8 {
        let _ = dma_mask;
        alloc::alloc::alloc(layout)
    }

    /// Deallocate previously allocated memory
    ///
    /// # Parameters
    ///
    /// * `ptr` - The memory pointer to deallocate, must be allocated via the `alloc` method
    /// * `layout` - Memory layout, must be the same as used during allocation
    ///
    /// # Safety
    ///
    /// This function is unsafe because undefined behavior can result if the caller does not
    /// ensure that `ptr` was allocated by a previous call to the `alloc` method with the same `layout`.
    /// The caller must ensure:
    /// - `ptr` was allocated via the `alloc` method
    /// - `layout` is the same as used during allocation
    /// - The memory is not accessed after deallocation
    #[cfg(feature = "alloc")]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        alloc::alloc::dealloc(ptr, layout)
    }
}

static mut OSAL: &'static dyn Osal = &osal::NopOsal;
static INIT: AtomicBool = AtomicBool::new(false);

/// Initialize DMA API
///
/// This function must be called before using any DMA functionality to set up the OSAL implementation.
/// This function is thread-safe; multiple calls will only use the OSAL implementation from the first call.
///
/// # Parameters
///
/// * `osal` - A reference to an implementation of the `Osal` trait
///
/// # Example
///
/// ```
/// use dma_api::*;
///
/// struct MyOsal;
/// impl Osal for MyOsal {
///     // Implement the required methods...
///     # fn map(&self, addr: std::ptr::NonNull<u8>, size: usize, direction: Direction) -> u64 { addr.as_ptr() as _ }
///     # fn unmap(&self, addr: std::ptr::NonNull<u8>, size: usize) {}
///     # fn flush(&self, addr: std::ptr::NonNull<u8>, size: usize) {}
///     # fn invalidate(&self, addr: std::ptr::NonNull<u8>, size: usize) {}
/// }
///
/// init(&MyOsal);
/// ```
///
/// # Notes
///
/// - Must be called before using any DMA functionality
/// - Repeated calls have no effect (the OSAL from the first call will be used)
pub fn init(osal: &'static dyn Osal) {
    if INIT.load(core::sync::atomic::Ordering::Acquire) {
        return;
    }

    unsafe {
        OSAL = osal;
    }
    INIT.store(true, core::sync::atomic::Ordering::Release);
}

fn get_osal() -> &'static dyn Osal {
    if !INIT.load(core::sync::atomic::Ordering::Acquire) {
        panic!("dma-api not initialized");
    }
    unsafe { OSAL }
}

fn map(addr: NonNull<u8>, size: usize, direction: Direction) -> u64 {
    get_osal().map(addr, size, direction)
}

fn unmap(addr: NonNull<u8>, size: usize) {
    get_osal().unmap(addr, size)
}

fn invalidate(addr: NonNull<u8>, size: usize) {
    get_osal().invalidate(addr, size)
}

fn flush(addr: NonNull<u8>, size: usize) {
    get_osal().flush(addr, size)
}

#[cfg(feature = "alloc")]
fn alloc(dma_mask: u64, layout: core::alloc::Layout) -> *mut u8 {
    unsafe { get_osal().alloc(dma_mask, layout) }
}

#[cfg(feature = "alloc")]
fn dealloc(ptr: *mut u8, layout: core::alloc::Layout) {
    unsafe { get_osal().dealloc(ptr, layout) }
}
