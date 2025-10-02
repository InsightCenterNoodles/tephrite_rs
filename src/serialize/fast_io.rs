use bytemuck::{Pod, Zeroable};
use core::{fmt, mem, ptr};

#[inline(always)]
#[cold]
fn oob() -> ! {
    panic!("ByteReader/ByteWriter out of bounds")
}

// MARK: Traits

pub trait ByteSink {
    /// Core write implementation
    fn put_bytes(&mut self, src: &[u8]);

    /// Write a POD by raw bytes (native endian).
    #[inline(always)]
    fn put_pod<T: Pod>(&mut self, v: &T) {
        // Avoid creating a &[u8] slice to help LLVM vectorize
        let p = (v as *const T).cast::<u8>();
        self.put_raw(p, mem::size_of::<T>());
    }

    /// Write a POD slice (length + bytes).
    #[inline(always)]
    fn put_pod_slice<T: Pod>(&mut self, s: &[T]) {
        self.put_usize(s.len());
        let bytes = bytemuck::cast_slice(s);
        self.put_bytes(bytes);
    }

    #[inline(always)]
    fn put_raw(&mut self, p: *const u8, n: usize) {
        self.put_bytes(unsafe { std::slice::from_raw_parts(p, n) });
    }

    // Primitives as native endian POD
    #[inline(always)]
    fn put_u8(&mut self, v: u8) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_i8(&mut self, v: i8) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_u16(&mut self, v: u16) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_i16(&mut self, v: i16) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_u32(&mut self, v: u32) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_i32(&mut self, v: i32) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_u64(&mut self, v: u64) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_i64(&mut self, v: i64) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_f32(&mut self, v: f32) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_f64(&mut self, v: f64) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_usize(&mut self, v: usize) {
        self.put_pod(&v)
    }
    #[inline(always)]
    fn put_isize(&mut self, v: isize) {
        self.put_pod(&v)
    }

    #[inline(always)]
    fn put_bool(&mut self, v: bool) {
        self.put_u8(v as u8);
    }

    #[inline(always)]
    fn put_str(&mut self, s: &str) {
        self.put_usize(s.len());
        self.put_bytes(s.as_bytes());
    }
}

pub trait ByteSource<'a> {
    fn take_bytes(&mut self, n: usize) -> &'a [u8];

    fn take_bytes_to(&mut self, n: &mut [u8]);

    /// Read POD by copy (native endian).
    #[inline(always)]
    fn get_pod<T: Pod>(&mut self) -> T {
        let n = mem::size_of::<T>();
        let bytes = self.take_bytes(n);
        // Safety: T: Pod
        unsafe { ptr::read_unaligned(bytes.as_ptr() as *const T) }
    }

    /// Read POD slice as owned Vec (length + bytes).
    #[inline(always)]
    fn get_pod_vec<T: Pod + Zeroable>(&mut self) -> Vec<T> {
        let count = self.get_usize();
        let Some(nbytes) = count.checked_mul(mem::size_of::<T>()) else {
            oob()
        };
        let bytes = self.take_bytes(nbytes);
        // Owned Vec<T> without per-element loops
        let mut v: Vec<T> = bytemuck::zeroed_vec(count);
        // Safety: len matches, POD move
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), v.as_mut_ptr().cast::<u8>(), nbytes);
        }
        v
    }

    // Primitives
    #[inline(always)]
    fn get_u8(&mut self) -> u8 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_i8(&mut self) -> i8 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_u16(&mut self) -> u16 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_i16(&mut self) -> i16 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_u32(&mut self) -> u32 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_i32(&mut self) -> i32 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_u64(&mut self) -> u64 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_i64(&mut self) -> i64 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_f32(&mut self) -> f32 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_f64(&mut self) -> f64 {
        self.get_pod()
    }
    #[inline(always)]
    fn get_usize(&mut self) -> usize {
        self.get_pod()
    }
    #[inline(always)]
    fn get_isize(&mut self) -> isize {
        self.get_pod()
    }

    #[inline(always)]
    fn get_bool(&mut self) -> bool {
        self.get_u8() != 0
    }

    #[inline(always)]
    fn get_string(&mut self) -> String {
        let n = self.get_usize();
        let b = self.take_bytes(n);
        unsafe { String::from_utf8_unchecked(b.to_vec()) }
    }
}

// MARK: Byte Writer

/// Write raw bytes and primitives to a slice
pub struct ByteWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}
impl<'a> ByteWriter<'a> {
    #[inline(always)]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    #[inline(always)]
    pub fn new_with_position(buf: &'a mut [u8], pos: usize) -> Self {
        Self { buf, pos }
    }

    #[inline(always)]
    pub fn position(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
}

impl<'a> ByteSink for ByteWriter<'a> {
    #[inline(always)]
    fn put_bytes(&mut self, src: &[u8]) {
        let Some(end) = self.pos.checked_add(src.len()) else {
            oob()
        };

        if end > self.buf.len() {
            oob();
        }
        // Safety: we just bounds-checked
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), self.buf.as_mut_ptr().add(self.pos), src.len());
        }
        self.pos = end;
    }
}

// Helpful for debugging
impl fmt::Debug for ByteWriter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ByteWriter")
            .field("pos", &self.pos)
            .field("len", &self.buf.len())
            .finish()
    }
}

// MARK: Byte Reader

/// Read raw bytes and primitives from a slice
pub struct ByteReader<'a> {
    buf: &'a [u8],
    pos: usize,
}
impl<'a> ByteReader<'a> {
    #[inline(always)]
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    #[inline(always)]
    pub fn new_with_position(buf: &'a mut [u8], pos: usize) -> Self {
        Self { buf, pos }
    }

    #[inline(always)]
    pub fn position(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
}

impl<'a> ByteSource<'a> for ByteReader<'a> {
    #[inline(always)]
    fn take_bytes(&mut self, n: usize) -> &'a [u8] {
        let Some(end) = self.pos.checked_add(n) else {
            oob()
        };
        if end > self.buf.len() {
            oob();
        }
        // Safety: bounds checked; split borrow lifetime to 'a
        let p = unsafe { self.buf.get_unchecked(self.pos..end) };
        self.pos = end;
        p
    }

    #[inline(always)]
    fn take_bytes_to(&mut self, n: &mut [u8]) {
        let Some(end) = self.pos.checked_add(n.len()) else {
            oob()
        };
        if end > self.buf.len() {
            oob();
        }
        // Safety: bounds checked; split borrow lifetime to 'a
        let p = unsafe { self.buf.get_unchecked(self.pos..end) };
        self.pos = end;

        unsafe {
            ptr::copy_nonoverlapping(p.as_ptr(), n.as_mut_ptr(), p.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_roundtrip() {
        let mut buffer = vec![0u8; 1024];
        {
            let mut writer = ByteWriter::new(&mut buffer);

            // Test writing primitives
            writer.put_u8(255);
            writer.put_i8(-127);
            writer.put_u16(49152); // 0xC000 in hex
            writer.put_i16(-32768); // 0x8000 in hex
            writer.put_u32(4294967295); // 0xFFFFFFFF in hex
            writer.put_i32(-2147483648); // 0x80000000 in hex
            writer.put_f32(3.14);
            writer.put_f64(2.71828);
            writer.put_usize(usize::MAX);
            writer.put_isize(isize::MIN);

            // Test writing a boolean and string
            writer.put_bool(true);
            writer.put_str("Hello, world!");

            assert_eq!(writer.position(), 64); // Total bytes written
        }

        let mut reader = ByteReader::new(&buffer);

        // Test reading primitives
        assert_eq!(reader.get_u8(), 255);
        assert_eq!(reader.get_i8(), -127);
        assert_eq!(reader.get_u16(), 49152); // 0xC000 in hex
        assert_eq!(reader.get_i16(), -32768); // 0x8000 in hex
        assert_eq!(reader.get_u32(), 4294967295); // 0xFFFFFFFF in hex
        assert_eq!(reader.get_i32(), -2147483648); // 0x80000000 in hex
        assert_eq!(reader.get_f32(), 3.14);
        assert_eq!(reader.get_f64(), 2.71828);
        assert_eq!(reader.get_usize(), usize::MAX);
        assert_eq!(reader.get_isize(), isize::MIN);

        // Test reading a boolean and string
        assert_eq!(reader.get_bool(), true);
        assert_eq!(reader.get_string(), "Hello, world!");

        assert_eq!(reader.position(), 64); // Total bytes read
    }
}
