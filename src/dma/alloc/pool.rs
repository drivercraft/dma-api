//! DMA vector memory pool
//!
//! This module provides the `DVecPool` type for efficient management and reuse of DMA vector memory.
//! Memory pools can avoid frequent memory allocation and deallocation operations, improving performance.
//!
//! # Example
//!
//! ```
//! use dma_api::*;
//!
//! init(&NopOsal);
//!
//! let config = DVecConfig {
//!     dma_mask: u64::MAX,
//!     align: 0x1000,
//!     size: 4096,
//!     direction: Direction::Bidirectional,
//! };
//!
//! // Create a memory pool with a capacity of 10
//! let pool = DVecPool::new_pool(config, 10);
//!
//! // Allocate buffers from the pool
//! let mut buff1 = pool.alloc().unwrap();
//! let mut buff2 = pool.alloc().unwrap();
//!
//! // Use buffers...
//! buff1.set(0, 1);
//!
//! // When buff1 goes out of scope, it will be automatically returned to the pool
//! ```

use core::ops::{Deref, DerefMut};

use alloc::{
    collections::VecDeque,
    sync::{Arc, Weak},
};
use spin::Mutex;

use crate::{DVec, Direction};

/// DMA vector memory pool configuration
///
/// Defines the creation parameters for DMA vectors in the memory pool.
///
/// # Fields
///
/// * `dma_mask` - DMA mask, specifying the address range the device can access
/// * `align` - Memory alignment in bytes
/// * `size` - Size of each DMA vector in bytes
/// * `direction` - DMA transfer direction
#[derive(Debug, Clone)]
pub struct DVecConfig {
    pub dma_mask: u64,
    pub align: usize,
    pub size: usize,
    pub direction: Direction,
}

/// DMA vector memory pool
///
/// This type manages a set of pre-allocated DMA vectors, providing an efficient memory reuse mechanism.
/// Using a memory pool can avoid frequent memory allocation and deallocation operations.
///
/// # Example
///
/// ```
/// use dma_api::*;
///
/// init(&NopOsal);
///
/// let config = DVecConfig {
///     dma_mask: u64::MAX,
///     align: 0x1000,
///     size: 4096,
///     direction: Direction::Bidirectional,
/// };
///
/// let pool = DVecPool::new_pool(config, 10);
/// let mut buff = pool.alloc().unwrap();
/// buff.set(0, 42);
/// ```
#[derive(Clone)]
pub struct DVecPool {
    inner: Arc<Mutex<Inner>>,
}

/// DMA buffer
///
/// This type wraps a `DVec<u8>` and holds a weak reference to the memory pool it belongs to.
/// When the buffer goes out of scope, it will be automatically returned to the memory pool.
///
/// Note: This type implements the `Deref` and `DerefMut` traits,
/// so it can be used just like `DVec<u8>`.
pub struct DBuff {
    data: Option<DVec<u8>>,
    pool: Weak<Mutex<Inner>>,
}

unsafe impl Send for DBuff {}

/// Allows using `DBuff` like `DVec<u8>`
impl Deref for DBuff {
    type Target = DVec<u8>;

    fn deref(&self) -> &Self::Target {
        self.data.as_ref().unwrap()
    }
}

/// Allows mutably using `DBuff` like `DVec<u8>`
impl DerefMut for DBuff {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.as_mut().unwrap()
    }
}

/// Automatically return buffer to memory pool
///
/// When `DBuff` is dropped, it will be automatically returned to the memory pool it belongs to.
/// If the memory pool has been destroyed, the buffer will also be freed.
impl Drop for DBuff {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            if let Some(pool) = self.pool.upgrade() {
                let mut inner = pool.lock();
                inner.dealloc(data);
            }
        }
    }
}

struct Inner {
    config: DVecConfig,
    pool: VecDeque<DVec<u8>>,
}

impl Inner {
    fn alloc(&mut self) -> Option<DVec<u8>> {
        self.pool.pop_front()
    }

    fn dealloc(&mut self, dvec: DVec<u8>) {
        self.pool.push_back(dvec);
    }
}

impl DVecPool {
    /// Create a new DMA vector memory pool
    ///
    /// This method pre-allocates the specified number of DMA vectors into the pool.
    ///
    /// # Parameters
    ///
    /// * `config` - Memory pool configuration, defines the creation parameters for DMA vectors
    /// * `cap` - Initial capacity of the memory pool
    ///
    /// # Returns
    ///
    /// Returns the initialized memory pool
    ///
    /// # Example
    ///
    /// ```
    /// use dma_api::*;
    ///
    /// let config = DVecConfig {
    ///     dma_mask: u64::MAX,
    ///     align: 0x1000,
    ///     size: 4096,
    ///     direction: Direction::Bidirectional,
    /// };
    ///
    /// let pool = DVecPool::new_pool(config, 10);
    /// ```
    pub fn new_pool(config: DVecConfig, cap: usize) -> DVecPool {
        let mut pool = VecDeque::with_capacity(cap);
        for _ in 0..cap {
            if let Ok(dvec) =
                DVec::zeros(config.dma_mask, config.size, config.align, config.direction)
            {
                pool.push_back(dvec);
            }
        }

        DVecPool {
            inner: Arc::new(Mutex::new(Inner { pool, config })),
        }
    }

    /// Allocate a DMA buffer from the memory pool
    ///
    /// If a buffer is available in the pool, it will be reused; otherwise, a new buffer will be created.
    ///
    /// # Returns
    ///
    /// Returns the allocated `DBuff`, or an error on failure
    ///
    /// # Example
    ///
    /// ```
    /// use dma_api::*;
    ///
    /// let pool = DVecPool::new_pool(config, 10);
    /// let mut buff = pool.alloc().unwrap();
    ///
    /// // Automatically returned after use
    /// {
    ///     let mut buff = pool.alloc().unwrap();
    ///     buff.set(0, 42);
    /// } // At this point, buff is automatically returned to the pool
    /// ```
    pub fn alloc(&self) -> Result<DBuff, crate::dma::alloc::DError> {
        let config = {
            let mut inner = self.inner.lock();
            if let Some(dvec) = inner.alloc() {
                return Ok(DBuff {
                    data: Some(dvec),
                    pool: Arc::downgrade(&self.inner),
                });
            } else {
                inner.config.clone()
            }
        };

        let dvec = DVec::zeros(config.dma_mask, config.size, config.align, config.direction)?;
        Ok(DBuff {
            data: Some(dvec),
            pool: Arc::downgrade(&self.inner),
        })
    }
}
