// Copyright (C) 2023 Carlos Bentzen <cadubentzen@gmail.com>
//
// Licensed under the BSD 3-Clause Clear License <LICENSE.txt>.
// This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: BSD-3-Clause-Clear

use std::sync::{Mutex, MutexGuard};

use gst::glib;
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use once_cell::sync::Lazy;

struct State {
    decoder: vvdec::Decoder,
    video_meta_supported: bool,
    output_info: Option<gst_video::VideoInfo>,
    input_state: gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
}

#[derive(Default)]
pub struct VVdeC {
    state: Mutex<Option<State>>,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "vvdec",
        gst::DebugColorFlags::empty(),
        Some("VVdeC VVC decoder"),
    )
});

type StateGuard<'s> = MutexGuard<'s, Option<State>>;

impl VVdeC {
    fn decode<'s>(
        &'s self,
        mut state_guard: StateGuard,
        input_buffer: gst::Buffer,
    ) -> Result<(), gst::FlowError> {
        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;

        let cts = input_buffer.pts().map(|ts| *ts as u64);
        let dts = input_buffer.dts().map(|ts| *ts as u64);

        let input_data = input_buffer
            .into_mapped_buffer_readable()
            .map_err(|_| gst::FlowError::Error)?;

        match state.decoder.decode(input_data, cts, dts, false) {
            Ok(Some(frame)) => {
                drop(self.handle_decoded_frame(state_guard, &frame)?);
            }
            Ok(None) | Err(vvdec::Error::TryAgain) => (),
            Err(err) => {
                gst::warning!(CAT, imp: self, "decoder returned {:?}", err);
                return Err(gst::FlowError::Error);
            }
        }

        Ok(())
    }

    fn handle_decoded_frame<'s>(
        &'s self,
        state_guard: StateGuard<'s>,
        decoded_frame: &vvdec::Frame,
    ) -> Result<StateGuard, gst::FlowError> {
        gst::trace!(
            CAT,
            imp: self,
            "Handling decoded frame {}",
            decoded_frame.sequence_number()
        );

        let mut state_guard = self.handle_resolution_changes(state_guard, decoded_frame)?;
        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;

        let instance = self.obj();
        let output_state = instance
            .output_state()
            .expect("Output state not set. Shouldn't happen!");
        let offset = decoded_frame.sequence_number() as i32;

        let frame = instance.frame(offset);
        if let Some(mut frame) = frame {
            let output_buffer = self.decoded_frame_as_buffer(state, decoded_frame, output_state)?;
            frame.set_output_buffer(output_buffer);
            drop(state_guard);
            // finish_frame may trigger another decide_allocation call which locks the mutex,
            // so we need to drop the guard in this portion.
            instance.finish_frame(frame)?;
            gst::trace!(CAT, imp: self, "Finished frame {offset}");
            Ok(self.state.lock().unwrap())
        } else {
            gst::warning!(CAT, imp: self, "No frame found for offset {offset}");
            Ok(state_guard)
        }
    }

    fn handle_resolution_changes<'s>(
        &'s self,
        mut state_guard: StateGuard<'s>,
        frame: &vvdec::Frame,
    ) -> Result<StateGuard<'s>, gst::FlowError> {
        let format = self.gst_video_format_from_vvdec_frame(frame);
        if format == gst_video::VideoFormat::Unknown {
            return Err(gst::FlowError::NotNegotiated);
        }

        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
        let need_negotiate = {
            match state.output_info {
                Some(ref i) => {
                    (i.width() != frame.width())
                        || (i.height() != frame.height() || (i.format() != format))
                }
                None => true,
            }
        };
        if !need_negotiate {
            return Ok(state_guard);
        }

        gst::info!(
            CAT,
            imp: self,
            "Negotiating format {:?} frame dimensions {}x{}",
            format,
            frame.width(),
            frame.height()
        );

        let input_state = state.input_state.clone();
        drop(state_guard);

        // The mutex needs to have been dropped in this portion, because it will
        // trigger a `decide_allocation` call which also needs to lock the mutex.
        // Not dropping the mutex would otherwise dead-lock.
        let instance = self.obj();
        let output_state =
            instance.set_output_state(format, frame.width(), frame.height(), Some(&input_state))?;
        instance.negotiate(output_state)?;
        let out_state = instance.output_state().unwrap();

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
        state.output_info = Some(out_state.info());

        gst::trace!(CAT, imp: self, "Negotiated format");

        Ok(state_guard)
    }

    fn gst_video_format_from_vvdec_frame(&self, frame: &vvdec::Frame) -> gst_video::VideoFormat {
        let color_format = frame.color_format();
        let bit_depth = frame.bit_depth();

        let format_desc = match (&color_format, bit_depth) {
            (vvdec::ColorFormat::Yuv400Planar, _) => todo!("implement grayscale"),
            (vvdec::ColorFormat::Yuv420Planar, 8) => "I420",
            (vvdec::ColorFormat::Yuv422Planar, 8) => "Y42B",
            (vvdec::ColorFormat::Yuv444Planar, 8) => "Y444",
            #[cfg(target_endian = "little")]
            (vvdec::ColorFormat::Yuv420Planar, 10) => "I420_10LE",
            #[cfg(target_endian = "little")]
            (vvdec::ColorFormat::Yuv422Planar, 10) => "I422_10LE",
            #[cfg(target_endian = "little")]
            (vvdec::ColorFormat::Yuv444Planar, 10) => "Y444_10LE",
            _ => {
                gst::warning!(
                    CAT,
                    imp: self,
                    "Unsupported VVdeC format {:?}/{:?}",
                    color_format,
                    bit_depth
                );
                return gst_video::VideoFormat::Unknown;
            }
        };

        format_desc
            .parse::<gst_video::VideoFormat>()
            .unwrap_or_else(|_| {
                gst::warning!(CAT, imp: self, "Unsupported VVdeC format: {}", format_desc);
                gst_video::VideoFormat::Unknown
            })
    }

    fn forward_pending_frames<'s>(
        &'s self,
        mut state_guard: StateGuard<'s>,
    ) -> Result<(), gst::FlowError> {
        loop {
            let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
            match state.decoder.flush() {
                Ok(frame) => state_guard = self.handle_decoded_frame(state_guard, &frame)?,
                Err(vvdec::Error::Eof) => return Ok(()),
                Err(err) => {
                    gst::warning!(
                        CAT,
                        imp: self,
                        "Decoder returned error while flushing: {err}"
                    );
                    return Err(gst::FlowError::Error);
                }
            }
        }
    }

    fn flush_decoder(&self, state: &mut State) {
        loop {
            match state.decoder.flush() {
                Ok(_) => (),
                Err(vvdec::Error::Eof) => break,
                Err(err) => {
                    gst::warning!(CAT, imp: self, "Error when flushing: {err}");
                    // FIXME: will the decoder recover after pushing more frames here or
                    // would we need to reinitialize it?
                    break;
                }
            }
        }
    }

    fn decoded_frame_as_buffer(
        &self,
        state: &mut State,
        frame: &vvdec::Frame,
        output_state: gst_video::VideoCodecState<gst_video::video_codec_state::Readable>,
    ) -> Result<gst::Buffer, gst::FlowError> {
        let video_meta_supported = state.video_meta_supported;

        let mut out_buffer = gst::Buffer::new();
        let mut_buffer = out_buffer.get_mut().unwrap();

        let info = output_state.info();
        // TODO: implement grayscale
        let components = [
            vvdec::PlaneComponent::Y,
            vvdec::PlaneComponent::U,
            vvdec::PlaneComponent::V,
        ];

        let mut offsets = vec![];
        let mut strides = vec![];
        let mut acc_offset: usize = 0;

        for component in components {
            let dest_stride: u32 = info.stride()[component as usize].try_into().unwrap();
            let plane = frame.plane(component);
            let src_stride = plane.stride();
            let mem = if video_meta_supported || dest_stride == src_stride {
                gst::Memory::from_slice(plane)
            } else {
                gst::trace!(
                    gst::CAT_PERFORMANCE,
                    imp: self,
                    "Copying decoded video frame component {:?}",
                    component
                );

                let src_slice = plane.as_ref();
                let height = plane.height();
                let mem = gst::Memory::with_size((dest_stride * height) as usize);
                let mut writable_mem = mem
                    .into_mapped_memory_writable()
                    .map_err(|_| gst::FlowError::Error)?;
                let chunk_len = std::cmp::min(src_stride, dest_stride) as usize;

                for (out_line, in_line) in writable_mem
                    .as_mut_slice()
                    .chunks_exact_mut(dest_stride.try_into().unwrap())
                    .zip(src_slice.chunks_exact(src_stride.try_into().unwrap()))
                {
                    out_line.copy_from_slice(&in_line[..chunk_len]);
                }

                writable_mem.into_memory()
            };
            let mem_size = mem.size();
            mut_buffer.append_memory(mem);

            strides.push(src_stride as i32);
            offsets.push(acc_offset);
            acc_offset += mem_size;
        }

        if video_meta_supported {
            gst_video::VideoMeta::add_full(
                out_buffer.get_mut().unwrap(),
                gst_video::VideoFrameFlags::empty(),
                info.format(),
                info.width(),
                info.height(),
                &offsets,
                &strides[..],
            )
            .unwrap();
        }

        Ok(out_buffer)
    }
}

fn video_output_formats() -> impl IntoIterator<Item = gst_video::VideoFormat> {
    // TODO: implement grayscale
    [
        gst_video::VideoFormat::I420,
        gst_video::VideoFormat::Y42b,
        gst_video::VideoFormat::Y444,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::I42010le,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::I42210le,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::Y44410le,
        // FIXME: not sure whether VVdeC has ever been tested
        // in big-endian platform. If so, then the lines below
        // can be uncommented.
        // #[cfg(target_endian = "big")]
        // gst_video::VideoFormat::I42010be,
        // #[cfg(target_endian = "big")]
        // gst_video::VideoFormat::I42210be,
        // #[cfg(target_endian = "big")]
        // gst_video::VideoFormat::Y44410be,
    ]
}

#[glib::object_subclass]
impl ObjectSubclass for VVdeC {
    const NAME: &'static str = "GstVVdeC";
    type Type = super::VVdeC;
    type ParentType = gst_video::VideoDecoder;
}

impl ObjectImpl for VVdeC {}

impl GstObjectImpl for VVdeC {}

impl ElementImpl for VVdeC {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "VVdeC VVC Decoder",
                "Codec/Decoder/Video",
                "Decode VVC video streams with VVdeC",
                "Carlos Bentzen <cadubentzen@gmail.com",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let sink_caps = gst::Caps::builder("video/x-h266")
                .field("stream-format", "byte-stream")
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_video::VideoCapsBuilder::new()
                .format_list(video_output_formats())
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl VideoDecoderImpl for VVdeC {
    fn set_format(
        &self,
        input_state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> Result<(), gst::LoggableError> {
        let mut state_guard = self.state.lock().unwrap();

        let Some(decoder) = vvdec::Decoder::new() else {
            return Err(gst::loggable_error!(CAT, "Failed to create decoder instance"));
        };

        *state_guard = Some(State {
            decoder,
            video_meta_supported: false,
            output_info: None,
            input_state: input_state.clone(),
        });

        self.parent_set_format(input_state)
    }

    fn handle_frame(
        &self,
        frame: gst_video::VideoCodecFrame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(
            CAT,
            imp: self,
            "Decoding frame {}",
            frame.system_frame_number()
        );

        let input_buffer = frame
            .input_buffer_owned()
            .expect("frame without input buffer");

        {
            let state_guard = self.state.lock().unwrap();
            self.decode(state_guard, input_buffer)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::info!(CAT, imp: self, "Stopping");

        {
            let mut state_guard = self.state.lock().unwrap();
            *state_guard = None;
        }

        self.parent_stop()
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp: self, "Draining");

        {
            let mut state_guard = self.state.lock().unwrap();
            if state_guard.as_mut().is_some() {
                self.forward_pending_frames(state_guard)?;
            }
        }

        self.parent_drain()
    }

    fn finish(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp: self, "Finishing");

        {
            let mut state_guard = self.state.lock().unwrap();
            if state_guard.as_mut().is_some() {
                self.forward_pending_frames(state_guard)?;
            }
        }

        self.parent_finish()
    }

    fn flush(&self) -> bool {
        gst::info!(CAT, imp: self, "Flushing");

        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                self.flush_decoder(state);
            }
        }

        true
    }

    fn decide_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        gst::trace!(CAT, imp: self, "Deciding allocation");

        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.video_meta_supported = query
                    .find_allocation_meta::<gst_video::VideoMeta>()
                    .is_some();
                gst::trace!(
                    CAT,
                    imp: self,
                    "Video meta support: {}",
                    state.video_meta_supported
                );
            }
        }

        self.parent_decide_allocation(query)
    }
}
