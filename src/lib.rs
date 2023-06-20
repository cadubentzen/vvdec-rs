use std::{mem, ptr};
use vvdec_sys::*;

pub struct Params {
    params: vvdecParams,
}

// TODO: builder to override params

impl Default for Params {
    fn default() -> Self {
        unsafe {
            let mut params: vvdecParams = mem::zeroed();
            vvdec_params_default(&mut params);
            Self { params }
        }
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("unspecified malfunction")]
    Unspecified,
    #[error("decoder not initialized or tried to initialize multiple times")]
    Initialize,
    #[error("internal allocation error")]
    Allocate,
    #[error("decoder input error, decoder input data error")]
    DecInput,
    #[error("allocated memory to small to receive decoded data. After allocating sufficient memory the failed call can be repeated.")]
    EnoughMem,
    #[error("inconsistent or invalid parameters")]
    Parameter,
    #[error("unsupported request")]
    NotSupported,
    #[error("decoder requires restart")]
    RestartRequired,
    #[error("unsupported CPU SSE 4.1 needed")]
    Cpu,
    #[error("decoder needs more input and cannot return a picture")]
    TryAgain,
    #[error("end of file")]
    Eof,
}

impl Error {
    fn new(code: i32) -> Error {
        use Error::*;
        #[allow(non_upper_case_globals)]
        match code {
            vvdecErrorCodes_VVDEC_ERR_UNSPECIFIED => Unspecified,
            vvdecErrorCodes_VVDEC_ERR_INITIALIZE => Initialize,
            vvdecErrorCodes_VVDEC_ERR_ALLOCATE => Allocate,
            vvdecErrorCodes_VVDEC_ERR_DEC_INPUT => DecInput,
            vvdecErrorCodes_VVDEC_NOT_ENOUGH_MEM => EnoughMem,
            vvdecErrorCodes_VVDEC_ERR_PARAMETER => Parameter,
            vvdecErrorCodes_VVDEC_ERR_NOT_SUPPORTED => NotSupported,
            vvdecErrorCodes_VVDEC_ERR_RESTART_REQUIRED => RestartRequired,
            vvdecErrorCodes_VVDEC_ERR_CPU => Cpu,
            vvdecErrorCodes_VVDEC_TRY_AGAIN => TryAgain,
            vvdecErrorCodes_VVDEC_EOF => Eof,
            _ => unreachable!(),
        }
    }
}

// TODO
#[derive(Debug)]
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
        random_access_point: bool,
    ) -> Result<Option<Frame>, Error> {
        let mut au = vvdecAccessUnit {
            payload: data.as_ptr() as *mut u8,
            payloadSize: data.len() as i32,
            payloadUsedSize: data.len() as i32,
            cts: cts.unwrap_or_default(),
            dts: dts.unwrap_or_default(),
            ctsValid: cts.is_some(),
            dtsValid: cts.is_some(),
            rap: random_access_point,
        };

        let mut frame: *mut vvdecFrame = ptr::null_mut();

        let ret = unsafe { vvdec_decode(self.decoder.as_ptr(), &mut au, &mut frame) };

        if ret != vvdecErrorCodes_VVDEC_OK {
            return Err(Error::new(ret));
        }

        if frame != ptr::null_mut() {
            return Ok(Some(Frame {}));
        }
        Ok(None)
    }

    pub fn flush(&mut self) -> Result<Frame, Error> {
        let mut frame: *mut vvdecFrame = ptr::null_mut();

        let ret = unsafe { vvdec_flush(self.decoder.as_ptr(), &mut frame) };

        if ret != vvdecErrorCodes_VVDEC_OK {
            return Err(Error::new(ret));
        }

        assert!(frame != ptr::null_mut());
        Ok(Frame {})
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            vvdec_decoder_close(self.decoder.as_ptr());
        }
    }
}
