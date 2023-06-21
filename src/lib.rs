use std::{
    fmt::Display,
    mem,
    ops::Deref,
    ptr,
    sync::{Arc, Mutex},
};
use vvdec_sys::*;

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

        assert!(!frame.is_null());
        Ok(Frame {
            inner: Arc::new(InnerFrame::new(self.clone(), unsafe {
                ptr::NonNull::new_unchecked(frame)
            })),
        })
    }
}

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
        self.params.errHandlingFlags = error_handling.to_ffi();
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

#[derive(Debug, Clone)]
pub struct Frame {
    inner: Arc<InnerFrame>,
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
        FrameFormat::new(self.inner.frameFormat)
    }

    pub fn color_format(&self) -> ColorFormat {
        ColorFormat::new(self.inner.colorFormat)
    }

    pub fn sequence_number(&self) -> u64 {
        self.inner.sequenceNumber
    }

    pub fn cts(&self) -> Option<u64> {
        self.inner.ctsValid.then_some(self.inner.cts)
    }

    pub fn pic_attributes(&self) -> Option<PictureAttributes> {
        ptr::NonNull::new(self.inner.picAttributes).map(PictureAttributes::new)
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

pub struct PictureAttributes {
    pub nal_type: NalType,
    pub slice_type: SliceType,
    pub is_ref_pic: bool,
    pub temporal_layer: u32,
    pub poc: u64,
}

impl PictureAttributes {
    fn new(pic_attributes: ptr::NonNull<vvdecPicAttributes>) -> Self {
        let &vvdecPicAttributes {
            nalType,
            sliceType,
            isRefPic,
            temporalLayer,
            poc,
            // Those fields are not used yet. May be added
            // to the API if proven useful.
            bits: _,
            vui: _,
            hrd: _,
            olsHrd: _,
        } = unsafe { pic_attributes.as_ref() };
        Self {
            nal_type: NalType::new(nalType),
            slice_type: SliceType::new(sliceType),
            is_ref_pic: isRefPic,
            temporal_layer: temporalLayer,
            poc,
        }
    }
}

#[derive(Debug)]
pub enum NalType {
    CodedSliceTrail,
    CodedSliceStsa,
    CodedSliceRadl,
    CodedSliceRasl,
    ReservedVcl4,
    ReservedVcl5,
    ReservedVcl6,
    CodedSliceIdrWRadl,
    CodedSliceIdrNLp,
    CodedSliceCra,
    CodedSliceGdr,
    ReservedIrapVcl11,
    ReservedIrapVcl12,
    Dci,
    Vps,
    Sps,
    Pps,
    PrefixAps,
    SuffixAps,
    Ph,
    AccessUnitDelimiter,
    Eos,
    Eob,
    PrefixSei,
    SuffixSei,
    Fd,
    ReservedNvcl26,
    ReservedNvcl27,
    Unspecified28,
    Unspecified29,
    Unspecified30,
    Unspecified31,
    Invalid,
    Unknown(u32),
}

impl NalType {
    fn new(nal_type: vvdecNalType) -> Self {
        use NalType::*;
        #[allow(non_upper_case_globals)]
        match nal_type {
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_TRAIL => CodedSliceTrail,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_STSA => CodedSliceStsa,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_RADL => CodedSliceRadl,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_RASL => CodedSliceRasl,
            vvdecNalType_VVC_NAL_UNIT_RESERVED_VCL_4 => ReservedVcl4,
            vvdecNalType_VVC_NAL_UNIT_RESERVED_VCL_5 => ReservedVcl5,
            vvdecNalType_VVC_NAL_UNIT_RESERVED_VCL_6 => ReservedVcl6,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_IDR_W_RADL => CodedSliceIdrWRadl,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_IDR_N_LP => CodedSliceIdrNLp,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_CRA => CodedSliceCra,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_GDR => CodedSliceGdr,
            vvdecNalType_VVC_NAL_UNIT_RESERVED_IRAP_VCL_11 => ReservedIrapVcl11,
            vvdecNalType_VVC_NAL_UNIT_RESERVED_IRAP_VCL_12 => ReservedIrapVcl12,
            vvdecNalType_VVC_NAL_UNIT_DCI => Dci,
            vvdecNalType_VVC_NAL_UNIT_VPS => Vps,
            vvdecNalType_VVC_NAL_UNIT_SPS => Sps,
            vvdecNalType_VVC_NAL_UNIT_PPS => Pps,
            vvdecNalType_VVC_NAL_UNIT_PREFIX_APS => PrefixAps,
            vvdecNalType_VVC_NAL_UNIT_SUFFIX_APS => SuffixAps,
            vvdecNalType_VVC_NAL_UNIT_PH => Ph,
            vvdecNalType_VVC_NAL_UNIT_ACCESS_UNIT_DELIMITER => AccessUnitDelimiter,
            vvdecNalType_VVC_NAL_UNIT_EOS => Eos,
            vvdecNalType_VVC_NAL_UNIT_EOB => Eob,
            vvdecNalType_VVC_NAL_UNIT_PREFIX_SEI => PrefixSei,
            vvdecNalType_VVC_NAL_UNIT_SUFFIX_SEI => SuffixSei,
            vvdecNalType_VVC_NAL_UNIT_FD => Fd,
            vvdecNalType_VVC_NAL_UNIT_RESERVED_NVCL_26 => ReservedNvcl26,
            vvdecNalType_VVC_NAL_UNIT_RESERVED_NVCL_27 => ReservedNvcl27,
            vvdecNalType_VVC_NAL_UNIT_UNSPECIFIED_28 => Unspecified28,
            vvdecNalType_VVC_NAL_UNIT_UNSPECIFIED_29 => Unspecified29,
            vvdecNalType_VVC_NAL_UNIT_UNSPECIFIED_30 => Unspecified30,
            vvdecNalType_VVC_NAL_UNIT_UNSPECIFIED_31 => Unspecified31,
            vvdecNalType_VVC_NAL_UNIT_INVALID => Invalid,
            _ => Unknown(nal_type),
        }
    }
}

pub enum SliceType {
    I,
    P,
    B,
    Unknown(u32),
}

impl SliceType {
    fn new(slice_type: vvdecSliceType) -> Self {
        use SliceType::*;
        #[allow(non_upper_case_globals)]
        match slice_type {
            vvdecSliceType_VVDEC_SLICETYPE_I => I,
            vvdecSliceType_VVDEC_SLICETYPE_P => P,
            vvdecSliceType_VVDEC_SLICETYPE_B => B,
            _ => Unknown(slice_type),
        }
    }
}

#[derive(Debug)]
pub enum ErrorHandling {
    Off,
    TryContinue,
}

impl ErrorHandling {
    fn to_ffi(&self) -> vvdecErrHandlingFlags {
        use ErrorHandling::*;
        match self {
            Off => vvdecErrHandlingFlags_VVDEC_ERR_HANDLING_OFF,
            TryContinue => vvdecErrHandlingFlags_VVDEC_ERR_HANDLING_TRY_CONTINUE,
        }
    }
}

#[derive(Debug)]
pub enum FrameFormat {
    Invalid,
    Progressive,
    TopField,
    BottomField,
    TopBottom,
    BottomTop,
    TopBottomTop,
    BottomTopBotttom,
    FrameDouble,
    FrameTriple,
    TopPairedWithPrevious,
    BottomPairedWithPrevious,
    TopPairedWithNext,
    BottomPairedWithNext,
    Unknown(i32),
}

impl FrameFormat {
    fn new(frame_format: vvdecFrameFormat) -> Self {
        use FrameFormat::*;
        #[allow(non_upper_case_globals)]
        match frame_format {
            vvdecFrameFormat_VVDEC_FF_INVALID => Invalid,
            vvdecFrameFormat_VVDEC_FF_PROGRESSIVE => Progressive,
            vvdecFrameFormat_VVDEC_FF_TOP_FIELD => TopField,
            vvdecFrameFormat_VVDEC_FF_BOT_FIELD => BottomField,
            vvdecFrameFormat_VVDEC_FF_TOP_BOT => TopBottom,
            vvdecFrameFormat_VVDEC_FF_BOT_TOP => BottomTop,
            vvdecFrameFormat_VVDEC_FF_TOP_BOT_TOP => TopBottomTop,
            vvdecFrameFormat_VVDEC_FF_BOT_TOP_BOT => BottomTopBotttom,
            vvdecFrameFormat_VVDEC_FF_FRAME_DOUB => FrameDouble,
            vvdecFrameFormat_VVDEC_FF_FRAME_TRIP => FrameTriple,
            vvdecFrameFormat_VVDEC_FF_TOP_PW_PREV => TopPairedWithPrevious,
            vvdecFrameFormat_VVDEC_FF_BOT_PW_PREV => BottomPairedWithPrevious,
            vvdecFrameFormat_VVDEC_FF_TOP_PW_NEXT => TopPairedWithNext,
            vvdecFrameFormat_VVDEC_FF_BOT_PW_NEXT => BottomPairedWithNext,
            _ => Unknown(frame_format),
        }
    }
}

#[derive(Debug)]
pub enum ColorFormat {
    Invalid,
    Yuv400Planar,
    Yuv420Planar,
    Yuv422Planar,
    Yuv444Planar,
    Unknown(i32),
}

impl ColorFormat {
    fn new(color_format: vvdecColorFormat) -> Self {
        use ColorFormat::*;
        #[allow(non_upper_case_globals)]
        match color_format {
            vvdecColorFormat_VVDEC_CF_INVALID => Invalid,
            vvdecColorFormat_VVDEC_CF_YUV400_PLANAR => Yuv400Planar,
            vvdecColorFormat_VVDEC_CF_YUV420_PLANAR => Yuv420Planar,
            vvdecColorFormat_VVDEC_CF_YUV422_PLANAR => Yuv422Planar,
            vvdecColorFormat_VVDEC_CF_YUV444_PLANAR => Yuv444Planar,
            _ => Unknown(color_format),
        }
    }
}
