// Copyright (C) 2023 Carlos Bentzen <cadubentzen@gmail.com>
//
// Licensed under the BSD 3-Clause Clear License <LICENSE.txt>.
// This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: BSD-3-Clause-Clear

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use once_cell::sync::Lazy;

#[derive(Default)]
pub struct VVdeC {}

impl VVdeC {}

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
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::I42010be,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::I42210be,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::Y44410be,
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

impl VideoDecoderImpl for VVdeC {}
