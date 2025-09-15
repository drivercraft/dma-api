use core::ptr::NonNull;

use aarch64_cpu_ext::cache::{dcache_range, CacheOp};

pub fn flush(addr: NonNull<u8>, size: usize) {
    dcache_range(CacheOp::Clean, addr.as_ptr() as _, size);
}

pub fn invalidate(addr: NonNull<u8>, size: usize) {
    dcache_range(CacheOp::Invalidate, addr.as_ptr() as _, size);
}
