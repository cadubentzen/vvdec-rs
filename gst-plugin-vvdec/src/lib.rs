// Copyright (C) 2023 Carlos Bentzen <cadubentzen@gmail.com>
//
// Licensed under the BSD 3-Clause Clear License <LICENSE.txt>.
// This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: BSD-3-Clause-Clear
use gst::glib;

mod dec;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    dec::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    vvdec,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
