//! System Integration Tests
//!
//! This module contains system-level integration tests that verify the DMA API
//! behavior in real or highly simulated environments.

use dma_api::*;

/// Aarch64-specific hardware integration test
///
/// This test verifies that the DMA API works correctly with actual
/// aarch64 cache operations.
#[test]
#[cfg(target_arch = "aarch64")]
fn test_aarch64_cache_operations() {
    use dma_api::osal::arch;

    init(&RealOsal);

    // Test data
    let mut data = [1u32, 2, 3, 4];
    let addr = core::ptr::NonNull::from(&data[0]).cast();

    // Test cache flush
    arch::flush(addr, 16);

    // Test cache invalidate
    arch::invalidate(addr, 16);

    // Verify data integrity after cache operations
    assert_eq!(data, [1, 2, 3, 4]);
}

/// Aarch64-specific DMA slice integration test
#[test]
#[cfg(target_arch = "aarch64")]
fn test_aarch64_slice_integration() {
    init(&RealOsal);

    let data = [1u32, 2, 3, 4];
    let slice = DSlice::from(&data, Direction::ToDevice);

    // Verify bus address is valid
    assert!(slice.bus_addr() != 0);

    // Verify data access works
    assert_eq!(slice[0], 1);
    assert_eq!(slice[1], 2);
    assert_eq!(slice[2], 3);
    assert_eq!(slice[3], 4);
}

/// System integration test for memory pool with real environment
///
/// This test verifies that the memory pool works correctly in a system environment
/// with actual memory allocation and deallocation.
#[test]
#[cfg(feature = "alloc")]
fn test_system_pool_integration() {
    init(&RealOsal);

    let config = DVecConfig {
        dma_mask: u64::MAX,
        align: 0x1000,
        size: 4096,
        direction: Direction::Bidirectional,
    };

    // Create pool
    let pool = DVecPool::new_pool(config, 10);

    // Allocate and use buffers in realistic pattern
    for _ in 0..5 {
        let mut buff = pool.alloc().unwrap();
        for i in 0..10 {
            buff.set(i, i as u8);
        }
        assert_eq!(buff[0], 0);
        assert_eq!(buff[9], 9);
    }

    // Pool should still work after multiple allocations
    let buff = pool.alloc().unwrap();
    assert_eq!(buff.len(), 4096);
}

/// Cross-component integration test
///
/// This test verifies that different DMA components can work together correctly.
#[test]
#[cfg(feature = "alloc")]
fn test_cross_component_integration() {
    init(&RealOsal);

    // Test 1: DBox + DVec integration
    let mut metadata = DBox::zero(u64::MAX, Direction::Bidirectional).unwrap();
    metadata.write(42u32);

    let mut data = DVec::zeros(u64::MAX, 10, 4, Direction::ToDevice).unwrap();
    data.set(0, metadata.read());

    assert_eq!(data[0], 42);

    // Test 2: DSlice + DVec integration
    let src = [1u32, 2, 3, 4, 5];
    let slice = DSlice::from(&src, Direction::ToDevice);

    let mut dest = DVec::zeros(u64::MAX, 5, 4, Direction::FromDevice).unwrap();

    for i in 0..5 {
        dest.set(i, slice[i]);
    }

    assert_eq!(dest[0], 1);
    assert_eq!(dest[4], 5);

    // Test 3: Pool + multiple components
    let config = DVecConfig {
        dma_mask: u64::MAX,
        align: 0x1000,
        size: 1024,
        direction: Direction::Bidirectional,
    };
    let pool = DVecPool::new_pool(config, 3);

    {
        let mut buff1 = pool.alloc().unwrap();
        let mut dbox = DBox::zero(u64::MAX, Direction::ToDevice).unwrap();
        dbox.write(100);

        // Copy from DBox to pool buffer
        buff1.set(0, dbox.read() as u8);
        assert_eq!(buff1[0], 100);
    }

    // Buffer should be returned to pool and reusable
    let buff2 = pool.alloc().unwrap();
    assert_eq!(buff2.len(), 1024);
}

/// Memory stress test under system conditions
///
/// This test verifies memory management stability under realistic workload.
#[test]
#[cfg(feature = "alloc")]
fn test_system_memory_stress() {
    init(&RealOsal);

    // Create multiple pools
    let config1 = DVecConfig {
        dma_mask: u64::MAX,
        align: 8,
        size: 256,
        direction: Direction::Bidirectional,
    };

    let config2 = DVecConfig {
        dma_mask: u64::MAX,
        align: 8,
        size: 256,
        direction: Direction::ToDevice,
    };

    let pool1 = DVecPool::new_pool(config1, 5);
    let pool2 = DVecPool::new_pool(config2, 5);

    // Stress test with alternating allocations
    for i in 0..10 {
        let pool = if i % 2 == 0 { &pool1 } else { &pool2 };
        let mut buff = pool.alloc().unwrap();

        // Write and verify data
        buff.set(0, (i % 256) as u8);
        assert_eq!(buff[0], (i % 256) as u8);
    }
}

/// Direction-specific cache behavior test
///
/// This test verifies that different DMA directions trigger appropriate
/// cache operations in a system environment.
#[test]
#[cfg(target_arch = "aarch64")]
fn test_direction_cache_behavior() {
    init(&RealOsal);

    let mut buffer = [0u32; 4];

    // Test ToDevice direction (should flush cache)
    {
        let mut slice = DSliceMut::from(&mut buffer, Direction::ToDevice);
        slice.set(0, 42);
        slice.set(1, 43);
        slice.confirm_write_all();
        assert_eq!(slice[0], 42);
    }

    // Test FromDevice direction (should invalidate cache)
    {
        let slice = DSlice::from(&buffer, Direction::FromDevice);
        assert_eq!(slice[0], 42);
        assert_eq!(slice[1], 43);
    }

    // Test Bidirectional direction (should handle both)
    {
        let mut slice = DSliceMut::from(&mut buffer, Direction::Bidirectional);
        slice.set(2, 44);
        slice.confirm_write_all();
        assert_eq!(slice[2], 44);
    }
}

/// Large buffer system test
///
/// This test verifies that the DMA API handles large buffers correctly
/// in a system environment.
#[test]
#[cfg(feature = "alloc")]
fn test_large_buffer_handling() {
    init(&RealOsal);

    // Test with 1MB buffer
    let size = 1024 * 1024;
    let mut dvec = DVec::<u8>::zeros(u64::MAX, size, 0x1000, Direction::Bidirectional).unwrap();

    // Verify size
    assert_eq!(dvec.len(), size);

    // Write pattern
    for i in 0..256 {
        for j in 0..4096 {
            dvec.set(i * 4096 + j, i as u8);
        }
    }

    // Verify pattern
    for i in 0..256 {
        assert_eq!(dvec[i * 4096], i as u8);
        assert_eq!(dvec[i * 4096 + 4095], i as u8);
    }
}

/// Real OSAL implementation for system testing
///
/// This implementation provides identity mapping for system-level testing
/// without requiring actual hardware support.
struct RealOsal;

impl Osal for RealOsal {
    fn map(&self, addr: core::ptr::NonNull<u8>, _size: usize, _direction: Direction) -> u64 {
        // Identity mapping: virtual address = physical address
        addr.as_ptr() as _
    }

    fn unmap(&self, _addr: core::ptr::NonNull<u8>, _size: usize) {
        // No-op for system testing with identity mapping
    }

    fn flush(&self, _addr: core::ptr::NonNull<u8>, _size: usize) {
        // System-level cache flush would happen here
        // For testing purposes, we rely on architecture-specific implementations
    }

    fn invalidate(&self, _addr: core::ptr::NonNull<u8>, _size: usize) {
        // System-level cache invalidation would happen here
        // For testing purposes, we rely on architecture-specific implementations
    }
}
