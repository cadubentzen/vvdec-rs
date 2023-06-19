use std::{mem, ptr};
use vvdec_sys::*;

pub struct Params {
    params: vvdecParams,
}

impl Default for Params {
    fn default() -> Self {
        unsafe {
            let mut params: vvdecParams = mem::zeroed();
            vvdec_params_default(&mut params);
            Self { params }
        }
    }
}

// TODO
pub enum Error {}

// TODO
pub struct Frame {}

pub struct Decoder {
    decoder: ptr::NonNull<vvdecDecoder>,
}

impl Decoder {
    pub fn new() -> Option<Self> {
        let default_params = Params::default();
        Self::with_params(default_params)
    }

    pub fn with_params(mut params: Params) -> Option<Self> {
        let decoder = unsafe { vvdec_decoder_open(&mut params.params) };
        ptr::NonNull::new(decoder).map(|decoder| Self { decoder })
    }

    pub fn decode(
        &mut self,
        data: &[u8],
        cts: Option<u64>,
        dts: Option<u64>,
        randomAccessPoint: bool,
    ) -> Result<Option<Frame>, Error> {
        todo!()
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            vvdec_decoder_close(self.decoder.as_ptr());
        }
    }
}
