// Copyright (C) 2023 Carlos Bentzen <cadubentzen@gmail.com>
//
// Licensed under the BSD 3-Clause Clear License <LICENSE.txt>.
// This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: BSD-3-Clause-Clear

use std::sync::Mutex;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use once_cell::sync::Lazy;

struct State {
    decoder: vvdec::Decoder,
    video_meta_supported: bool,
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

impl VVdeC {
    fn decode(
        &self,
        state: &mut State,
        input_buffer: gst::Buffer,
    ) -> Result<Option<vvdec::Frame>, gst::FlowError> {
        let cts = input_buffer.pts().map(|ts| *ts as u64);
        let dts = input_buffer.dts().map(|ts| *ts as u64);

        let input_data = input_buffer
            .into_mapped_buffer_readable()
            .map_err(|_| gst::FlowError::Error)?;

        // FIXME: handle TryAgain case
        state
            .decoder
            .decode(input_data, cts, dts, false)
            .map_err(|err| {
                gst::warning!(CAT, imp: self, "decoder returned {:?}", err);
                gst::FlowError::Error
            })
    }

    fn handle_decoded_frame(
        &self,
        state: &mut State,
        decoded_frame: &vvdec::Frame,
    ) -> Result<(), gst::FlowError> {
        // TODO: handle resolution changes
        gst::trace!(
            CAT,
            imp: self,
            "Handling decoded frame {}",
            decoded_frame.sequence_number()
        );

        let instance = self.obj();
        let output_state = instance
            .output_state()
            .expect("Output state not set. Shouldn't happen!");
        let offset = decoded_frame.sequence_number() as i32;

        let frame = instance.frame(offset);
        if let Some(mut frame) = frame {
            let output_buffer = self.decoded_frame_as_buffer(decoded_frame)?;
            frame.set_output_buffer(output_buffer);
            instance.finish_frame(frame)?;
        } else {
            gst::warning!(CAT, imp: self, "No frame found for offset {offset}");
        }
        Ok(())
    }

    fn forward_pending_frames(&self, state: &mut State) -> Result<(), gst::FlowError> {
        todo!()
    }

    fn flush_decoder(&self, state: &mut State) {
        loop {
            match state.decoder.flush() {
                Ok(_) => (),
                Err(vvdec::Error::Eof) => break,
                Err(err) => {
                    gst::warning!(CAT, imp: self, "Error when flushing: {err}");
                    // FIXME: will the decoder recover after pushing more frames here or
                    // would need to reinitialize it?
                    break;
                }
            }
        }
    }

    fn decoded_frame_as_buffer(
        &self,
        decoded_frame: &vvdec::Frame,
    ) -> Result<gst::Buffer, gst::FlowError> {
        todo!()
    }
}

fn video_output_formats() -> impl IntoIterator<Item = gst_video::VideoFormat> {
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
            let mut state_guard = self.state.lock().unwrap();
            let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
            if let Some(decoded_frame) = self.decode(state, input_buffer)? {
                self.handle_decoded_frame(state, &decoded_frame)?;
            }
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
            if let Some(state) = state_guard.as_mut() {
                self.forward_pending_frames(state)?;
            }
        }

        self.parent_drain()
    }

    fn finish(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp: self, "Finishing");

        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                self.forward_pending_frames(state)?;
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
        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.video_meta_supported = query
                    .find_allocation_meta::<gst_video::VideoMeta>()
                    .is_some();
            }
        }

        self.parent_decide_allocation(query)
    }
}
