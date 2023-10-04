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
        data: impl AsRef<[u8]>,
        cts: Option<u64>,
        dts: Option<u64>,
        is_random_access_point: bool,
    ) -> Result<Option<Frame>, Error> {
        let data = data.as_ref();
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

unsafe impl Sync for Decoder {}
unsafe impl Send for Decoder {}

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
    pub fn plane(&self, component: PlaneComponent) -> Plane {
        Plane::new(self.clone(), component)
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

    pub fn sequence_number(&self) -> u64 {
        self.inner.sequenceNumber
    }

    pub fn cts(&self) -> Option<u64> {
        self.inner.ctsValid.then_some(self.inner.cts)
    }

    pub fn frame_format(&self) -> FrameFormat {
        FrameFormat::new(self.inner.frameFormat)
    }

    pub fn color_format(&self) -> ColorFormat {
        ColorFormat::new(self.inner.colorFormat)
    }

    pub fn picture_attributes(&self) -> Option<PictureAttributes> {
        ptr::NonNull::new(self.inner.picAttributes).map(PictureAttributes::new)
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Frame(num planes: {}, width: {}, height: {}, bit depth: {}, \
            sequence number: {}, cts: {}, frame format: {:?}, color format: {:?}, pic attributes: {:#?})",
            self.num_planes(),
            self.width(),
            self.height(),
            self.bit_depth(),
            self.sequence_number(),
            self.cts().unwrap_or_default(),
            self.frame_format(),
            self.color_format(),
            self.picture_attributes()
        )
    }
}

unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

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
    frame: Frame,
    component: PlaneComponent,
}

impl Plane {
    fn new(frame: Frame, component: PlaneComponent) -> Self {
        Self { frame, component }
    }

    #[inline]
    fn inner(&self) -> vvdecPlane {
        self.frame.inner.planes[self.component.to_ffi() as usize]
    }

    pub fn width(&self) -> u32 {
        self.inner().width
    }

    pub fn height(&self) -> u32 {
        self.inner().height
    }

    pub fn stride(&self) -> u32 {
        self.inner().stride
    }

    pub fn bytes_per_sample(&self) -> u32 {
        self.inner().bytesPerSample
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

#[derive(Debug, Clone, Copy)]
pub enum PlaneComponent {
    Y,
    U,
    V,
}

impl PlaneComponent {
    #[inline]
    fn to_ffi(&self) -> u32 {
        match self {
            PlaneComponent::Y => vvdecComponentType_VVDEC_CT_Y,
            PlaneComponent::U => vvdecComponentType_VVDEC_CT_U,
            PlaneComponent::V => vvdecComponentType_VVDEC_CT_V,
        }
    }
}

impl From<PlaneComponent> for usize {
    fn from(value: PlaneComponent) -> Self {
        match value {
            PlaneComponent::Y => 0,
            PlaneComponent::U => 1,
            PlaneComponent::V => 2,
        }
    }
}

#[derive(Debug)]
pub struct PictureAttributes {
    pub nal_type: NalType,
    pub slice_type: SliceType,
    pub is_ref_pic: bool,
    pub temporal_layer: u32,
    pub poc: i64,
    pub num_compressed_bits: u32,
    pub vui: Option<Vui>,
    pub hrd: Option<Hrd>,
    pub ols_hrd: Option<OlsHrd>,
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
            olsHrd,
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
            ols_hrd: ptr::NonNull::new(olsHrd).map(OlsHrd::new),
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct Hrd {
    pub num_units_in_tick: u32,
    pub time_scale: u32,
    pub general_nal_hrd_params_present_flag: bool,
    pub general_vcl_hrd_params_present_flag: bool,
    pub general_same_pic_timing_in_all_ols_flag: bool,
    pub tick_divisor: u32,
    pub general_decoding_unit_hrd_params_present_flag: bool,
    pub bit_rate_scale: u32,
    pub cpb_size_scale: u32,
    pub cpb_size_du_scale: u32,
    pub hrd_cpb_cnt: u32,
}

impl Hrd {
    pub fn new(hrd: ptr::NonNull<vvdecHrd>) -> Self {
        let hrd = unsafe { hrd.as_ref() };
        let vvdecHrd {
            numUnitsInTick,
            timeScale,
            generalNalHrdParamsPresentFlag,
            generalVclHrdParamsPresentFlag,
            generalSamePicTimingInAllOlsFlag,
            tickDivisor,
            generalDecodingUnitHrdParamsPresentFlag,
            bitRateScale,
            cpbSizeScale,
            cpbSizeDuScale,
            hrdCpbCnt,
        } = *hrd;

        Self {
            num_units_in_tick: numUnitsInTick,
            time_scale: timeScale,
            general_nal_hrd_params_present_flag: generalNalHrdParamsPresentFlag,
            general_vcl_hrd_params_present_flag: generalVclHrdParamsPresentFlag,
            general_same_pic_timing_in_all_ols_flag: generalSamePicTimingInAllOlsFlag,
            tick_divisor: tickDivisor,
            general_decoding_unit_hrd_params_present_flag: generalDecodingUnitHrdParamsPresentFlag,
            bit_rate_scale: bitRateScale,
            cpb_size_scale: cpbSizeScale,
            cpb_size_du_scale: cpbSizeDuScale,
            hrd_cpb_cnt: hrdCpbCnt,
        }
    }
}

#[derive(Debug)]
pub enum SampleAspectRatio {
    Indicator(i32),
    WidthHeight(i32, i32),
}

impl SampleAspectRatio {
    fn new(aspect_ratio_idc: i32, sar_width: i32, sar_height: i32) -> Self {
        if aspect_ratio_idc == 255 {
            Self::WidthHeight(sar_width, sar_height)
        } else {
            Self::Indicator(aspect_ratio_idc)
        }
    }
}

#[derive(Debug)]
pub struct Vui {
    pub sample_aspect_ratio: Option<SampleAspectRatio>,
    pub aspect_ratio_constant_flag: bool,
    pub non_packed_flag: bool,
    pub non_projected_flag: bool,
    pub colour_description_present_flag: bool,
    pub colour_primaries: i32,
    pub transfer_characteristics: i32,
    pub matrix_coefficients: i32,
    pub progressive_source_flag: bool,
    pub interlaced_source_flag: bool,
    pub chroma_loc_info_present_flag: bool,
    pub chroma_sample_loc_type_top_field: i32,
    pub chroma_sample_loc_type_bottom_field: i32,
    pub chroma_sample_loc_type: i32,
    pub overscan_info_present_flag: bool,
    pub overscan_appropriate_flag: bool,
    pub video_signal_type_present_flag: bool,
    pub video_full_range_flag: bool,
}

impl Vui {
    pub fn new(vui: ptr::NonNull<vvdecVui>) -> Self {
        let vui = unsafe { vui.as_ref() };

        let vvdecVui {
            aspectRatioInfoPresentFlag,
            aspectRatioConstantFlag,
            nonPackedFlag,
            nonProjectedFlag,
            aspectRatioIdc,
            sarWidth,
            sarHeight,
            colourDescriptionPresentFlag,
            colourPrimaries,
            transferCharacteristics,
            matrixCoefficients,
            progressiveSourceFlag,
            interlacedSourceFlag,
            chromaLocInfoPresentFlag,
            chromaSampleLocTypeTopField,
            chromaSampleLocTypeBottomField,
            chromaSampleLocType,
            overscanInfoPresentFlag,
            overscanAppropriateFlag,
            videoSignalTypePresentFlag,
            videoFullRangeFlag,
        } = *vui;

        Self {
            sample_aspect_ratio: aspectRatioInfoPresentFlag.then_some(SampleAspectRatio::new(
                aspectRatioIdc,
                sarWidth,
                sarHeight,
            )),
            aspect_ratio_constant_flag: aspectRatioConstantFlag,
            non_packed_flag: nonPackedFlag,
            non_projected_flag: nonProjectedFlag,
            colour_description_present_flag: colourDescriptionPresentFlag,
            colour_primaries: colourPrimaries,
            transfer_characteristics: transferCharacteristics,
            matrix_coefficients: matrixCoefficients,
            progressive_source_flag: progressiveSourceFlag,
            interlaced_source_flag: interlacedSourceFlag,
            chroma_loc_info_present_flag: chromaLocInfoPresentFlag,
            chroma_sample_loc_type_top_field: chromaSampleLocTypeTopField,
            chroma_sample_loc_type_bottom_field: chromaSampleLocTypeBottomField,
            chroma_sample_loc_type: chromaSampleLocType,
            overscan_info_present_flag: overscanInfoPresentFlag,
            overscan_appropriate_flag: overscanAppropriateFlag,
            video_signal_type_present_flag: videoSignalTypePresentFlag,
            video_full_range_flag: videoFullRangeFlag,
        }
    }
}

#[derive(Debug)]
pub struct OlsHrd {
    pub fixed_pic_rate_general_flag: bool,
    pub fixed_pic_rate_within_cvs_flag: bool,
    pub element_duration_in_tc: u32,
    pub low_delay_hrd_flag: bool,
}

impl OlsHrd {
    pub fn new(ols_hrd: ptr::NonNull<vvdecOlsHrd>) -> Self {
        let ols_hrd = unsafe { ols_hrd.as_ref() };

        let vvdecOlsHrd {
            fixedPicRateGeneralFlag,
            fixedPicRateWithinCvsFlag,
            elementDurationInTc,
            lowDelayHrdFlag,
            bitRateValueMinus1: _,
            cpbSizeValueMinus1: _,
            ducpbSizeValueMinus1: _,
            duBitRateValueMinus1: _,
            cbrFlag: _,
        } = *ols_hrd;

        Self {
            fixed_pic_rate_general_flag: fixedPicRateGeneralFlag,
            fixed_pic_rate_within_cvs_flag: fixedPicRateWithinCvsFlag,
            element_duration_in_tc: elementDurationInTc,
            low_delay_hrd_flag: lowDelayHrdFlag,
        }
    }
}
