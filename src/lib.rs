#![forbid(missing_docs)]

//! # Rust bindings for VVdeC
//!
//! This crate provides safe Rust bindings for VVdeC.
//!
//! ```
//! use vvdec::{Decoder, Error, Frame, PlaneComponent};
//!
//! let mut decoder = Decoder::new().unwrap();
//! let data: &[u8] = &[0u8; 64]; // Replace this with actual VVC bitstream data.
//!
//! if let Ok(Some(frame)) = decoder.decode(data) {
//!     process_frame(frame);
//! }
//!
//! // At the end, call flush until Eof
//! loop {
//!     match decoder.flush() {
//!         Ok(frame) => process_frame(frame),
//!         Err(Error::Eof) => break,
//!         Err(err) => panic!("{err}"),
//!     }
//! }
//!
//! fn process_frame(frame: Frame) {
//!     let y_plane = frame.plane(PlaneComponent::Y).unwrap();
//!     let y_plane_data: &[u8] = y_plane.as_ref();
//! }
//! ```

use std::{
    mem,
    ops::Deref,
    ptr,
    sync::{Arc, Mutex},
};
use vvdec_sys::*;

/// VVC decoder
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

/// Access unit for VVC bitstream data.
pub struct AccessUnit<A> {
    /// The payload data.
    pub payload: A,
    /// Composition timestamp.
    pub cts: Option<u64>,
    /// Decoding timestamp.
    pub dts: Option<u64>,
    /// Is it an random access point?
    pub is_random_access_point: bool,
}

impl<A> AccessUnit<A> {
    /// Create a new access unit with no cts, dts and not a random access point.
    pub fn new(payload: A) -> Self {
        Self {
            payload,
            cts: None,
            dts: None,
            is_random_access_point: false,
        }
    }
}

impl<A> From<A> for AccessUnit<A> {
    fn from(value: A) -> Self {
        Self::new(value)
    }
}

impl Decoder {
    /// Create a new VVC decoder with default settings
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Create decoder builder
    pub fn builder() -> DecoderBuilder {
        DecoderBuilder::new()
    }

    fn with_params(params: &mut vvdecParams) -> Result<Self, Error> {
        let decoder = unsafe { vvdec_decoder_open(params) };

        ptr::NonNull::new(decoder)
            .map(|decoder| Self {
                inner: Arc::new(Mutex::new(InnerDecoder { decoder })),
            })
            .ok_or(Error::FailedToOpen)
    }

    /// Decode input data
    ///
    /// The decode function takes bitstream VVC data in the Annex-B
    /// format, which is prefixed by 0x000001 or 0x00000001.
    ///
    /// On success, it can optionally return a decoded frame, but may also
    /// not return anything, for example if it needs more data.
    pub fn decode<A, I>(&mut self, access_unit: I) -> Result<Option<Frame>, Error>
    where
        A: AsRef<[u8]>,
        I: Into<AccessUnit<A>>,
    {
        let AccessUnit {
            payload,
            cts,
            dts,
            is_random_access_point,
        } = access_unit.into();
        let payload = payload.as_ref();
        let mut au = vvdecAccessUnit {
            payload: payload.as_ptr() as *mut u8,
            payloadSize: payload.len() as i32,
            payloadUsedSize: payload.len() as i32,
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

    /// Flush the decoder
    ///
    /// It will flush the internally stored decoder state, returning frames
    /// until it returns Err(Error::Eof), or fails with another error.
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

unsafe impl Sync for Decoder {}
unsafe impl Send for Decoder {}

/// Decoder builder
pub struct DecoderBuilder {
    params: vvdecParams,
}

impl DecoderBuilder {
    /// Create a new DecoderBuilder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a Decoder instance.
    pub fn build(&mut self) -> Result<Decoder, Error> {
        Decoder::with_params(&mut self.params)
    }

    /// Set number of threads.
    pub fn num_threads(&mut self, num_threads: i32) -> &mut Self {
        self.params.threads = num_threads;
        self
    }

    /// Set number of threads for parsing.
    pub fn num_parse_threads(&mut self, num_parse_threads: i32) -> &mut Self {
        self.params.parseThreads = num_parse_threads;
        self
    }

    /// Remove padding in the decoded buffers.
    pub fn remove_padding(&mut self, remove_padding: bool) -> &mut Self {
        self.params.removePadding = remove_padding;
        self
    }
}

impl Default for DecoderBuilder {
    fn default() -> Self {
        unsafe {
            let mut params: vvdecParams = mem::zeroed();
            vvdec_params_default(&mut params);
            Self { params }
        }
    }
}

/// An error that has occurred when using a Decoder.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    /// Failed to open decoder.
    #[error("failed to open decoder")]
    FailedToOpen,
    /// Unspecified malfunction.
    #[error("unspecified malfunction")]
    Unspecified,
    /// Internal allocation error.
    #[error("internal allocation error")]
    Allocate,
    /// Decoder input error.
    #[error("decoder input error")]
    DecInput,
    /// Allocated memory too small to receive decoded data. After allocating sufficient memory the failed call can be repeated.
    #[error("allocated memory too small to receive decoded data. After allocating sufficient memory the failed call can be repeated.")]
    EnoughMem,
    /// Inconsistent or invalid parameters.
    #[error("inconsistent or invalid parameters")]
    Parameter,
    /// Unsupported request.
    #[error("unsupported request")]
    NotSupported,
    /// Decoder requires restart.
    #[error("decoder requires restart")]
    RestartRequired,
    /// Unsupported CPU.
    #[error("unsupported CPU")]
    Cpu,
    /// Decoder needs more input and cannot return a picture.
    #[error("decoder needs more input and cannot return a picture")]
    TryAgain,
    /// End of file.
    #[error("end of file")]
    Eof,
    /// Unknown error
    #[error("unknown error with code {0}")]
    Unknown(i32),
}

impl Error {
    fn new(code: i32) -> Error {
        use Error::*;
        #[allow(non_upper_case_globals)]
        match code {
            vvdecErrorCodes_VVDEC_ERR_UNSPECIFIED => Unspecified,
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

/// A raw frame returned by the Decoder.
#[derive(Debug, Clone)]
pub struct Frame {
    inner: Arc<InnerFrame>,
}

impl Frame {
    /// Get plane from the specified component.
    pub fn plane(&self, component: PlaneComponent) -> Option<Plane> {
        Plane::new(self.clone(), component)
    }

    /// Get number of planes.
    pub fn num_planes(&self) -> u32 {
        self.inner.numPlanes
    }

    /// Get frame width.
    pub fn width(&self) -> u32 {
        self.inner.width
    }

    /// Get frame height.
    pub fn height(&self) -> u32 {
        self.inner.height
    }

    /// Get bit depth.
    pub fn bit_depth(&self) -> u32 {
        self.inner.bitDepth
    }

    /// Sequence number of the frame.
    pub fn sequence_number(&self) -> u64 {
        self.inner.sequenceNumber
    }

    /// Get composition timestamp.
    pub fn cts(&self) -> Option<u64> {
        self.inner.ctsValid.then_some(self.inner.cts)
    }

    /// Get frame format.
    pub fn frame_format(&self) -> FrameFormat {
        FrameFormat::new(self.inner.frameFormat)
    }

    /// Get color format.
    pub fn color_format(&self) -> ColorFormat {
        ColorFormat::new(self.inner.colorFormat)
    }

    /// Get picture attributes.
    pub fn picture_attributes(&self) -> Option<PictureAttributes> {
        ptr::NonNull::new(self.inner.picAttributes).map(PictureAttributes::new)
    }
}

unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

#[derive(Debug)]
struct InnerFrame {
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

/// A plane from a Frame.
///
/// A plane can only be created via Frame::plane.
#[derive(Debug)]
pub struct Plane {
    frame: Frame,
    component: PlaneComponent,
}

impl Plane {
    fn new(frame: Frame, component: PlaneComponent) -> Option<Self> {
        (component.to_ffi() < frame.num_planes()).then_some(Self { frame, component })
    }

    #[inline]
    fn inner(&self) -> vvdecPlane {
        self.frame.inner.planes[self.component.to_ffi() as usize]
    }

    /// Get plane width.
    pub fn width(&self) -> u32 {
        self.inner().width
    }

    /// Get plane height.
    pub fn height(&self) -> u32 {
        self.inner().height
    }

    /// Get plane stride, in bytes.
    pub fn stride(&self) -> u32 {
        self.inner().stride
    }

    /// Get number of bytes per sample.
    pub fn bytes_per_sample(&self) -> u32 {
        self.inner().bytesPerSample
    }
}

impl AsRef<[u8]> for Plane {
    fn as_ref(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.inner().ptr as *const u8,
                self.stride() as usize * self.height() as usize,
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

unsafe impl Send for Plane {}
unsafe impl Sync for Plane {}

/// A plane component
#[derive(Debug, Clone, Copy)]
pub enum PlaneComponent {
    /// The luma component
    Y,
    /// The U chroma component
    U,
    /// The V chroma component
    V,
}

impl PlaneComponent {
    #[inline]
    fn to_ffi(self) -> u32 {
        match self {
            PlaneComponent::Y => vvdecComponentType_VVDEC_CT_Y,
            PlaneComponent::U => vvdecComponentType_VVDEC_CT_U,
            PlaneComponent::V => vvdecComponentType_VVDEC_CT_V,
        }
    }
}

impl From<PlaneComponent> for usize {
    fn from(value: PlaneComponent) -> Self {
        value.to_ffi() as usize
    }
}

/// Picture attributes.
#[derive(Debug)]
pub struct PictureAttributes {
    /// NAL type.
    pub nal_type: NalType,
    /// Slice type.
    pub slice_type: SliceType,
    /// Is this a reference picture?
    pub is_ref_pic: bool,
    /// Temporal layer index
    pub temporal_layer: u32,
    /// Picture order count
    pub poc: i64,
    /// Number of compressed bits
    pub num_compressed_bits: u32,
    /// VUI parameters
    pub vui: Option<Vui>,
    /// HRD parameters
    pub hrd: Option<Hrd>,
}

impl PictureAttributes {
    fn new(pic_attributes: ptr::NonNull<vvdecPicAttributes>) -> Self {
        let &vvdecPicAttributes {
            nalType,
            sliceType,
            isRefPic,
            temporalLayer,
            poc,
            bits,
            vui,
            hrd,
            ..
        } = unsafe { pic_attributes.as_ref() };
        Self {
            nal_type: NalType::new(nalType),
            slice_type: SliceType::new(sliceType),
            is_ref_pic: isRefPic,
            temporal_layer: temporalLayer,
            poc,
            num_compressed_bits: bits,
            vui: ptr::NonNull::new(vui).map(Vui::new),
            hrd: ptr::NonNull::new(hrd).map(Hrd::new),
        }
    }
}

/// NAL type.
#[derive(Debug)]
pub enum NalType {
    /// Coded slice trail.
    CodedSliceTrail,
    /// Coded slice STSA.
    CodedSliceStsa,
    /// Coded slice RADL.
    CodedSliceRadl,
    /// Coded slice RASL.
    CodedSliceRasl,
    /// Coded slice IDR W RADL.
    CodedSliceIdrWRadl,
    /// Coded slice IDR N LP.
    CodedSliceIdrNLp,
    /// Coded slice CRA.
    CodedSliceCra,
    /// Coded slice GDR.
    CodedSliceGdr,
    /// DCI.
    Dci,
    /// VPS.
    Vps,
    /// SPS.
    Sps,
    /// PPS.
    Pps,
    /// Prefix APS.
    PrefixAps,
    /// Suffix APS.
    SuffixAps,
    /// PH.
    Ph,
    /// Access Unit delimiter.
    AccessUnitDelimiter,
    /// End-of-stream.
    Eos,
    /// EOB.
    Eob,
    /// Prefix SEI.
    PrefixSei,
    /// Suffix SEI.
    SuffixSei,
    /// FD.
    Fd,
    /// Invalid.
    Invalid,
    /// Unknown.
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
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_IDR_W_RADL => CodedSliceIdrWRadl,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_IDR_N_LP => CodedSliceIdrNLp,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_CRA => CodedSliceCra,
            vvdecNalType_VVC_NAL_UNIT_CODED_SLICE_GDR => CodedSliceGdr,
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
            vvdecNalType_VVC_NAL_UNIT_INVALID => Invalid,
            _ => Unknown(nal_type),
        }
    }
}

/// Slice type.
#[derive(Debug)]
pub enum SliceType {
    /// I-slice.
    I,
    /// P-slice.
    P,
    /// B-slice.
    B,
    /// Unknown.
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

/// Frame format.
#[derive(Debug)]
pub enum FrameFormat {
    /// Invalid
    Invalid,
    /// Progressive
    Progressive,
    /// Top-field
    TopField,
    /// Bottom-field
    BottomField,
    /// Top-bottom
    TopBottom,
    /// Bottom-top
    BottomTop,
    /// Top-bottom-top
    TopBottomTop,
    /// Bottom-top-botttom
    BottomTopBotttom,
    /// Frame-double
    FrameDouble,
    /// Frame-triple
    FrameTriple,
    /// Top paired with previous
    TopPairedWithPrevious,
    /// Bottom paired with previous
    BottomPairedWithPrevious,
    /// Top paired with next
    TopPairedWithNext,
    /// Bottom paired with next
    BottomPairedWithNext,
    /// Unknown
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

/// Color format.
#[derive(Debug)]
pub enum ColorFormat {
    /// Invalid.
    Invalid,
    /// YUV400 in planar format (Grayscale).
    Yuv400Planar,
    /// YUV420 in planar format.
    Yuv420Planar,
    /// YUV422 in planar format.
    Yuv422Planar,
    /// YUV444 in planar format.
    Yuv444Planar,
    /// Unknown.
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

/// HRD parameters.
#[derive(Debug)]
pub struct Hrd {
    /// Number of units in tick.
    pub num_units_in_tick: u32,
    /// Time scale.
    pub time_scale: u32,
}

impl Hrd {
    fn new(hrd: ptr::NonNull<vvdecHrd>) -> Self {
        let hrd = unsafe { hrd.as_ref() };
        let vvdecHrd {
            numUnitsInTick,
            timeScale,
            ..
        } = *hrd;

        Self {
            num_units_in_tick: numUnitsInTick,
            time_scale: timeScale,
        }
    }
}

/// Sample Aspect Ratio.
#[derive(Debug)]
pub enum SampleAspectRatio {
    /// Indicator mode.
    Indicator(i32),
    /// Width and Height mode.
    WidthHeight {
        /// Width.
        width: i32,
        /// Height.
        height: i32,
    },
}

impl SampleAspectRatio {
    fn new(aspect_ratio_idc: i32, sar_width: i32, sar_height: i32) -> Self {
        if aspect_ratio_idc == 255 {
            Self::WidthHeight {
                width: sar_width,
                height: sar_height,
            }
        } else {
            Self::Indicator(aspect_ratio_idc)
        }
    }
}

/// VUI parameters.
#[derive(Debug)]
pub struct Vui {
    /// Sample aspect ratio
    pub sample_aspect_ratio: Option<SampleAspectRatio>,
    /// Is sample aspect ratio constant?
    pub is_aspect_ratio_constant: bool,
}

impl Vui {
    fn new(vui: ptr::NonNull<vvdecVui>) -> Self {
        let vui = unsafe { vui.as_ref() };

        let vvdecVui {
            aspectRatioInfoPresentFlag,
            aspectRatioConstantFlag,
            nonPackedFlag: _,
            nonProjectedFlag: _,
            aspectRatioIdc,
            sarWidth,
            sarHeight,
            ..
        } = *vui;

        Self {
            sample_aspect_ratio: aspectRatioInfoPresentFlag.then_some(SampleAspectRatio::new(
                aspectRatioIdc,
                sarWidth,
                sarHeight,
            )),
            is_aspect_ratio_constant: aspectRatioConstantFlag,
        }
    }
}
