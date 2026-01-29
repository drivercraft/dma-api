use dma_api::*;

// All tests in this file require the `alloc` feature
#[test]
#[cfg(feature = "alloc")]
fn test_read() {
    init(&Impled);

    let mut dma: DVec<u32> = DVec::zeros(u64::MAX, 10, 0x1000, Direction::FromDevice).unwrap();

    dma.set(0, 1);

    let o = dma.get(0).unwrap();

    assert_eq!(o, 1);
}

#[test]
#[cfg(feature = "alloc")]
fn test_write() {
    init(&Impled);

    let mut dma: DVec<u32> = DVec::zeros(u64::MAX, 10, 0x1000, Direction::ToDevice).unwrap();

    dma.set(0, 1);

    let o = dma.get(0).unwrap();

    assert_eq!(o, 1);
}
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)] // Used only in cfg(feature = "alloc") tests
struct Foo {
    foo: u32,
    bar: u32,
}

#[test]
#[cfg(feature = "alloc")]
fn test_modify() {
    init(&Impled);
    let mut dma: DBox<Foo> = DBox::zero(u64::MAX, Direction::Bidirectional).unwrap();

    dma.modify(|f| f.bar = 1);

    assert_eq!(dma.read(), Foo { foo: 0, bar: 1 });
}

#[test]
#[cfg(feature = "alloc")]
fn test_deref() {
    init(&Impled);
    let mut dma: DVec<u32> = DVec::zeros(u64::MAX, 10, 0x1000, Direction::FromDevice).unwrap();

    dma.set(0, 1);

    let foo = &dma;

    assert_eq!(foo[0], 1);
}

#[test]
#[cfg(feature = "alloc")]
fn test_copy() {
    init(&Impled);
    let mut dma: DVec<u32> = DVec::zeros(u64::MAX, 0x40, 0x1000, Direction::Bidirectional).unwrap();

    println!("new dma ok");

    let src = [1u32; 0x40];

    dma.copy_from_slice(&src);

    println!("copy ok");

    for (i, &v) in src.iter().enumerate() {
        assert_eq!(dma[i], v);
    }
}

#[test]
#[cfg(feature = "alloc")]
fn test_index() {
    init(&Impled);
    let dma: DVec<u32> = DVec::zeros(u64::MAX, 0x40, 0x1000, Direction::Bidirectional).unwrap();

    println!("new dma ok");

    let a = dma[0];

    assert_eq!(a, 0);
}

#[test]
fn test_slice() {
    init(&Impled);
    let src = [1u32; 0x40];
    let dma = DSlice::from(src.as_ref(), Direction::Bidirectional);

    assert_eq!(dma.len(), src.len());

    for (i, &v) in src.iter().enumerate() {
        assert_eq!(dma[i], v);
    }
}

#[test]
fn test_slice_index() {
    init(&Impled);
    let src = [1u32; 0x40];
    let dma = DSlice::from(src.as_ref(), Direction::Bidirectional);

    assert_eq!(dma[1], 1);
}

#[test]
fn test_slice_mut() {
    init(&Impled);
    let mut src = [1u32; 0x40];
    let dma = DSliceMut::from(src.as_mut(), Direction::Bidirectional);

    dma.set(0, 2);

    assert_eq!(dma[0], 2);
}

#[test]
#[cfg(feature = "alloc")]
fn test_from_vec() {
    init(&Impled);
    let value = vec![1, 2, 3];
    let dma = DVec::from_vec(u64::MAX, value, Direction::FromDevice).unwrap();

    assert_eq!(dma[1], 2);

    let v = dma.to_vec();

    println!("to vec");

    assert_eq!(v, vec![1, 2, 3]);
}

struct Impled;

impl Osal for Impled {
    fn map(&self, addr: core::ptr::NonNull<u8>, size: usize, direction: Direction) -> u64 {
        println!("map @{:?}, size {size:#x}, {direction:?}", addr);
        addr.as_ptr() as usize as _
    }

    fn unmap(&self, addr: core::ptr::NonNull<u8>, size: usize) {
        println!("unmap @{:?}, size {size:#x}", addr);
    }

    fn flush(&self, addr: core::ptr::NonNull<u8>, size: usize) {
        println!("flush @{:?}, size {size:#x}", addr);
    }

    fn invalidate(&self, addr: core::ptr::NonNull<u8>, size: usize) {
        println!("invalidate @{:?}, size {size:#x}", addr);
    }
}
