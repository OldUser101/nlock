// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use anyhow::{Result, anyhow};
use cairo::{Format, ImageSurface};
use gdk_pixbuf::Pixbuf;
use serde::Deserialize;

#[derive(Deserialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundImageScale {
    Stretch,
    Fill,
    Fit,
    Center,
    Tile,
}

pub trait ImageSurfaceExt {
    fn create_from_pixbuf(pixbuf: &Pixbuf) -> Result<ImageSurface>;
}

#[inline]
fn premul(c: u16, a: u16) -> u8 {
    let z = (c * a) + 0x80;
    ((z + (z >> 8)) >> 8) as u8
}

impl ImageSurfaceExt for ImageSurface {
    /// Create an `ImageSurface` from a `Pixbuf`
    ///
    /// The API to do this was removed from GTK :(, so this is a version ported
    /// from swaylock, that essentially does the same thing.
    fn create_from_pixbuf(pixbuf: &Pixbuf) -> Result<ImageSurface> {
        let chan = pixbuf.n_channels();
        if chan < 3 {
            return Err(anyhow!(cairo::Error::InvalidFormat));
        }

        let pixels = pixbuf.read_pixel_bytes();
        let width = pixbuf.width() as usize;
        let height = pixbuf.height() as usize;
        let stride = pixbuf.rowstride() as usize;

        let fmt = if chan == 3 {
            Format::Rgb24
        } else {
            Format::ARgb32
        };

        let mut surface = ImageSurface::create(fmt, width as i32, height as i32)?;
        surface.flush();

        {
            let cstride = surface.stride() as usize;
            let mut cpixels = surface.data()?;

            if chan == 3 {
                for y in 0..height {
                    let goff = y * stride;
                    let coff = y * cstride;

                    let grow = &pixels[goff..goff + 3 * width];
                    let crow = &mut cpixels[coff..coff + 4 * width];

                    for x in 0..width {
                        let src = &grow[3 * x..3 * x + 3];
                        let dst = &mut crow[4 * x..4 * x + 4];

                        if cfg!(target_endian = "little") {
                            dst[0] = src[2];
                            dst[1] = src[1];
                            dst[2] = src[0];
                            dst[3] = 0;
                        } else {
                            dst[0] = 0;
                            dst[1] = src[0];
                            dst[2] = src[1];
                            dst[3] = src[2];
                        }
                    }
                }
            } else {
                for y in 0..height {
                    let goff = y * stride;
                    let coff = y * cstride;

                    let grow = &pixels[goff..goff + 4 * width];
                    let crow = &mut cpixels[coff..coff + 4 * width];

                    for x in 0..width {
                        let src = &grow[4 * x..4 * x + 4];
                        let dst = &mut crow[4 * x..4 * x + 4];

                        let a = src[3] as u16;

                        if cfg!(target_endian = "little") {
                            dst[0] = premul(src[2] as u16, a);
                            dst[1] = premul(src[1] as u16, a);
                            dst[2] = premul(src[0] as u16, a);
                            dst[3] = a as u8;
                        } else {
                            dst[0] = a as u8;
                            dst[1] = premul(src[0] as u16, a);
                            dst[2] = premul(src[1] as u16, a);
                            dst[3] = premul(src[2] as u16, a);
                        }
                    }
                }
            }
        }

        surface.mark_dirty();
        Ok(surface)
    }
}
