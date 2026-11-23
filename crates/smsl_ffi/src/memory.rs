use crate::{boundary::*, *};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SmslBytesView {
    pub data: *const u8,
    pub len: usize,
}

pub struct SmslBytes(Vec<u8>);

api!([EncodingError] smsl_memory_compress(source: SmslString) -> *mut SmslBytes {
    Ok(own(SmslBytes(smsl_core::memory::compress(text(source)?)?)))
});
api!([EncodingError] smsl_memory_decompress(source: SmslBytesView) -> *mut SmslBuffer {
    Ok(own(SmslBuffer(smsl_core::memory::decompress(slice(source.data, source.len)?)?)))
});

/// # Safety
/// The handle must be live; the returned view lasts until it is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn smsl_bytes_view(value: *const SmslBytes) -> SmslBytesView {
    let value = unsafe { &*value };
    SmslBytesView {
        data: value.0.as_ptr(),
        len: value.0.len(),
    }
}

free!(smsl_bytes_free, SmslBytes);
