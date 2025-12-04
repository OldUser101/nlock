// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use cairo::ImageSurface;
use gdk_pixbuf::Pixbuf;
use std::io::Cursor;

pub trait ImageSurfaceExt {
    fn create_from_pixbuf(pixbuf: &Pixbuf) -> Result<ImageSurface, cairo::Error>;
}

impl ImageSurfaceExt for ImageSurface {
    /// Create an `ImageSurface` from a `Pixbuf` with in-memory PNG conversion.
    ///
    /// I only had to write this because the GTK people decided to remove or
    /// deprecate all other APIs that do this :(
    fn create_from_pixbuf(pixbuf: &Pixbuf) -> Result<ImageSurface, cairo::Error> {
        let png = pixbuf
            .save_to_bufferv("png", &[])
            .map_err(|_| cairo::Error::InvalidFormat)?;
        let mut cursor = Cursor::new(png);
        ImageSurface::create_from_png(&mut cursor).map_err(|_| cairo::Error::ReadError)
    }
}
