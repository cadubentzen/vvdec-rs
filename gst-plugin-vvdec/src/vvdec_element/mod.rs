// Copyright (C) 2023 Carlos Bentzen <cadubentzen@gmail.com>
//
// Licensed under the BSD 3-Clause Clear License <LICENSE.txt>.
// This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: BSD-3-Clause-Clear

use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct VVdeC(ObjectSubclass<imp::VVdeC>) @extends gst_video::VideoDecoder, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    let rank = gst::Rank::Primary + 1;

    gst::Element::register(Some(plugin), "vvdec", rank, VVdeC::static_type())
}
