use std::{
    fmt::Display,
    mem,
    ops::Deref,
    ptr,
    sync::{Arc, Mutex},
};
use vvdec_sys::*;

pub struct Params {
    params: vvdecParams,
}

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_num_threads(&mut self, num_threads: i32) {
        self.params.threads = num_threads;
    }

    pub fn set_num_parse_threads(&mut self, num_parse_threads: i32) {
        self.params.parseThreads = num_parse_threads;
    }

    pub fn set_verify_picture_hash(&mut self, verify_picture_hash: bool) {
        self.params.verifyPictureHash = verify_picture_hash;
    }

    pub fn set_remove_padding(&mut self, remove_padding: bool) {
        self.params.removePadding = remove_padding;
    }

    pub fn set_error_handling(&mut self, error_handling: ErrorHandling) {
        self.params.errHandlingFlags = error_handling.into_ffi();
    }
}

#[derive(Debug)]
pub enum ErrorHandling {
    Off,
    TryContinue,
}

impl ErrorHandling {
    fn into_ffi(&self) -> vvdecErrHandlingFlags {
        use ErrorHandling::*;
        match self {
            Off => vvdecErrHandlingFlags_VVDEC_ERR_HANDLING_OFF,
            TryContinue => vvdecErrHandlingFlags_VVDEC_ERR_HANDLING_TRY_CONTINUE,
        }
    }
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
    #[error("unknown error with code {0}")]
    Unknown(i32),
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
            _ => Unknown(code),
        }
    }
}

// TODO
pub enum FrameFormat {}

// TODO
pub enum ColorFormat {}

#[derive(Debug, Clone)]
pub struct Frame {
    inner: Arc<InnerFrame>,
}

#[derive(Debug)]
pub struct Plane {
    _frame: Frame,
    plane: vvdecPlane,
}

impl Plane {
    fn new(frame: Frame, plane: vvdecPlane) -> Self {
        Self {
            _frame: frame,
            plane,
        }
    }

    fn width(&self) -> u32 {
        self.plane.width
    }

    fn height(&self) -> u32 {
        self.plane.height
    }

    fn stride(&self) -> u32 {
        self.plane.stride
    }

    fn bytes_per_sample(&self) -> u32 {
        self.plane.bytesPerSample
    }
}

impl Display for Plane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Plane(width: {}, height: {}, stride: {}, bytes_per_sample: {})",
            self.width(),
            self.height(),
            self.stride(),
            self.bytes_per_sample()
        )
    }
}

impl AsRef<[u8]> for Plane {
    fn as_ref(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.plane.ptr as *const u8,
                self.plane.stride as usize
                    * self.plane.height as usize
                    * self.plane.bytesPerSample as usize,
            )
        }
    }
}

impl Deref for Plane {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl Frame {
    pub fn plane(&self, index: usize) -> Option<Plane> {
        (0..self.num_planes())
            .contains(&(index as u32))
            .then(|| Plane::new(self.clone(), self.inner.planes[index]))
    }

    pub fn num_planes(&self) -> u32 {
        self.inner.numPlanes
    }

    pub fn width(&self) -> u32 {
        self.inner.width
    }

    pub fn height(&self) -> u32 {
        self.inner.height
    }

    pub fn bit_depth(&self) -> u32 {
        self.inner.bitDepth
    }

    pub fn frame_format(&self) -> FrameFormat {
        todo!()
    }

    pub fn color_format(&self) -> ColorFormat {
        todo!()
    }

    pub fn sequence_number(&self) -> u64 {
        self.inner.sequenceNumber
    }

    pub fn cts(&self) -> Option<u64> {
        self.inner.ctsValid.then(|| self.inner.cts)
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Frame(num planes: {}, width: {}, height: {}, bit depth: {}, \
            sequence number: {}, cts: {})",
            self.num_planes(),
            self.width(),
            self.height(),
            self.bit_depth(),
            self.sequence_number(),
            self.cts().unwrap_or_default()
        )
    }
}

#[derive(Debug)]
pub struct InnerFrame {
    decoder: Decoder,
    frame: ptr::NonNull<vvdecFrame>,
}

impl Deref for InnerFrame {
    type Target = vvdecFrame;

    fn deref(&self) -> &Self::Target {
        unsafe { self.frame.as_ref() }
    }
}

impl InnerFrame {
    fn new(decoder: Decoder, frame: ptr::NonNull<vvdecFrame>) -> Self {
        // println!("new frame: {:?}", frame.as_ptr());
        Self { decoder, frame }
    }
}

impl Drop for InnerFrame {
    fn drop(&mut self) {
        unsafe {
            vvdec_frame_unref(
                self.decoder.inner.lock().unwrap().decoder.as_ptr(),
                self.frame.as_ptr(),
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct Decoder {
    inner: Arc<Mutex<InnerDecoder>>,
}

#[derive(Debug)]
struct InnerDecoder {
    decoder: ptr::NonNull<vvdecDecoder>,
}

impl Drop for InnerDecoder {
    fn drop(&mut self) {
        unsafe {
            vvdec_decoder_close(self.decoder.as_ptr());
        }
    }
}

impl Decoder {
    pub fn new() -> Option<Self> {
        let default_params = Params::default();
        Self::with_params(default_params)
    }

    pub fn with_params(mut params: Params) -> Option<Self> {
        let decoder = unsafe { vvdec_decoder_open(&mut params.params) };

        ptr::NonNull::new(decoder).map(|decoder| Self {
            inner: Arc::new(Mutex::new(InnerDecoder { decoder })),
        })
    }

    pub fn decode(
        &mut self,
        data: &[u8],
        cts: Option<u64>,
        dts: Option<u64>,
        is_random_access_point: bool,
    ) -> Result<Option<Frame>, Error> {
        let mut au = vvdecAccessUnit {
            payload: data.as_ptr() as *mut u8,
            payloadSize: data.len() as i32,
            payloadUsedSize: data.len() as i32,
            cts: cts.unwrap_or_default(),
            dts: dts.unwrap_or_default(),
            ctsValid: cts.is_some(),
            dtsValid: dts.is_some(),
            rap: is_random_access_point,
        };

        let mut frame: *mut vvdecFrame = ptr::null_mut();

        let ret = unsafe {
            vvdec_decode(
                self.inner.lock().unwrap().decoder.as_ptr(),
                &mut au,
                &mut frame,
            )
        };

        if ret != vvdecErrorCodes_VVDEC_OK {
            return Err(Error::new(ret));
        }

        Ok(ptr::NonNull::new(frame).map(|f| Frame {
            inner: Arc::new(InnerFrame::new(self.clone(), f)),
        }))
    }

    pub fn flush(&mut self) -> Result<Frame, Error> {
        let mut frame: *mut vvdecFrame = ptr::null_mut();

        let ret = unsafe { vvdec_flush(self.inner.lock().unwrap().decoder.as_ptr(), &mut frame) };

        if ret != vvdecErrorCodes_VVDEC_OK {
            return Err(Error::new(ret));
        }

        assert!(frame != ptr::null_mut());
        Ok(Frame {
            inner: Arc::new(InnerFrame::new(self.clone(), unsafe {
                ptr::NonNull::new_unchecked(frame)
            })),
        })
    }
}
