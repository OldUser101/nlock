// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use anyhow::{Result, anyhow, bail};
use cairo::SurfacePattern;
use tracing::warn;

use crate::{
    auth::AuthState,
    cairo_ext::CairoExt,
    config::NLockConfig,
    util::{BackgroundImageScale, BackgroundType},
};

pub const DEFAULT_DPI: f64 = 96.0;
pub const DEFAULT_SCALE: f64 = 1.0;

pub struct NLockRenderBackgroundArgs<'a> {
    pub buf_height: f64,
    pub buf_width: f64,
    pub context: &'a cairo::Context,
    pub image: Option<&'a cairo::ImageSurface>,
}

impl<'a> NLockRenderBackgroundArgs<'a> {
    fn get_buffer_dimensions(&self) -> Result<(f64, f64)> {
        if self.buf_width <= 0.0 || self.buf_height <= 0.0 {
            bail!(
                "Invalid width or height, {}x{}",
                self.buf_width,
                self.buf_height
            );
        }

        Ok((self.buf_width, self.buf_height))
    }
}

pub struct NLockRenderOverlayArgs<'a> {
    pub auth_state: AuthState,
    pub buf_height: f64,
    pub buf_width: f64,
    pub context: &'a cairo::Context,
    pub pwd_len: usize,
}

impl<'a> NLockRenderOverlayArgs<'a> {
    fn get_buffer_dimensions(&self) -> Result<(f64, f64)> {
        if self.buf_width <= 0.0 || self.buf_height <= 0.0 {
            bail!(
                "Invalid width or height, {}x{}",
                self.buf_width,
                self.buf_height
            );
        }

        Ok((self.buf_width, self.buf_height))
    }
}

#[derive(Default)]
pub struct NLockRenderer {
    dpi: Option<f64>,
    scale: Option<f64>,
    subpixel_order: Option<cairo::SubpixelOrder>,
}

impl NLockRenderer {
    pub fn set_dpi<T>(&mut self, dpi: T)
    where
        T: Into<f64>,
    {
        let dpi: f64 = dpi.into();

        if dpi.is_infinite() || dpi.is_nan() || dpi <= 0.0 {
            warn!("Invalid DPI {dpi}, falling back to {DEFAULT_DPI}");
            self.dpi = Some(DEFAULT_DPI);
        } else {
            self.dpi = Some(dpi);
        }
    }

    pub fn set_subpixel_order(&mut self, order: cairo::SubpixelOrder) {
        self.subpixel_order = Some(order);
    }

    pub fn set_scale(&mut self, scale: f64) {
        if scale > 0.0 {
            self.scale = Some(scale);
        }
    }

    fn clear_background(&self, context: &cairo::Context) -> Result<()> {
        context.save()?;
        context.set_operator(cairo::Operator::Source);
        context.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        context.paint()?;
        context.restore()?;

        Ok(())
    }

    fn reset_cairo_context(&self, context: &cairo::Context) -> Result<()> {
        context.set_antialias(cairo::Antialias::Best);
        self.clear_background(context)?;
        context.identity_matrix();

        Ok(())
    }

    fn configure_cairo_font(&self, config: &NLockConfig, context: &cairo::Context) -> Result<()> {
        let mut fo = cairo::FontOptions::new()?;
        fo.set_hint_style(cairo::HintStyle::Full);
        fo.set_antialias(cairo::Antialias::Subpixel);
        fo.set_subpixel_order(self.subpixel_order.unwrap_or(cairo::SubpixelOrder::Default));

        context.set_font_options(&fo);
        context.select_font_face(
            &config.font.family,
            cairo::FontSlant::from(config.font.slant),
            cairo::FontWeight::from(config.font.weight),
        );

        let dpi = self.dpi.unwrap_or(DEFAULT_DPI);
        let scale = self.scale.unwrap_or(DEFAULT_SCALE);
        context.set_font_size((config.font.size / 72.0) * dpi * scale);

        Ok(())
    }

    fn draw_rounded_rect(context: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
        context.new_sub_path();
        context.arc(x + w - r, y + r, r, -90f64.to_radians(), 0f64.to_radians());
        context.arc(
            x + w - r,
            y + h - r,
            r,
            0f64.to_radians(),
            90f64.to_radians(),
        );
        context.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
        context.arc(x + r, y + r, r, 180f64.to_radians(), 270f64.to_radians());
        context.close_path();
    }

    fn set_frame_border_color(
        &self,
        config: &NLockConfig,
        context: &cairo::Context,
        auth_state: AuthState,
    ) {
        match auth_state {
            AuthState::Idle => context.ext_set_source_rgba(config.colors.frame_border_idle),
            AuthState::Success => context.ext_set_source_rgba(config.colors.frame_border_success),
            AuthState::Fail => context.ext_set_source_rgba(config.colors.frame_border_fail),
        }
    }

    fn draw_background_image(
        &self,
        context: &cairo::Context,
        image: &cairo::ImageSurface,
        buf_width: f64,
        buf_height: f64,
        mode: BackgroundImageScale,
    ) -> Result<()> {
        let width = image.width() as f64;
        let height = image.height() as f64;

        match mode {
            BackgroundImageScale::Stretch => {
                context.scale(buf_width / width, buf_height / height);
                context.set_source_surface(image, 0.0, 0.0)?;
            }
            BackgroundImageScale::Center => {
                context.set_source_surface(
                    image,
                    (buf_width / 2.0 - width / 2.0).floor(),
                    (buf_height / 2.0 - height / 2.0).floor(),
                )?;
            }
            BackgroundImageScale::Tile => {
                let pattern = SurfacePattern::create(image);
                pattern.set_extend(cairo::Extend::Repeat);
                context.set_source(pattern)?;
            }
            BackgroundImageScale::Fit => {
                let buf_ratio = buf_width / buf_height;
                let bg_ratio = width / height;

                if buf_ratio > bg_ratio {
                    let scale = buf_height / height;
                    context.scale(scale, scale);
                    context.set_source_surface(
                        image,
                        buf_width / 2.0 / scale - width / 2.0,
                        0.0,
                    )?;
                } else {
                    let scale = buf_width / width;
                    context.scale(scale, scale);
                    context.set_source_surface(
                        image,
                        0.0,
                        buf_height / 2.0 / scale - height / 2.0,
                    )?;
                }
            }
            BackgroundImageScale::Fill => {
                let buf_ratio = buf_width / buf_height;
                let bg_ratio = width / height;

                if buf_ratio > bg_ratio {
                    let scale = buf_width / width;
                    context.scale(scale, scale);
                    context.set_source_surface(
                        image,
                        0.0,
                        buf_height / 2.0 / scale - height / 2.0,
                    )?;
                } else {
                    let scale = buf_height / height;
                    context.scale(scale, scale);
                    context.set_source_surface(
                        image,
                        buf_width / 2.0 / scale - width / 2.0,
                        0.0,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn draw_overlay(
        &self,
        config: &NLockConfig,
        context: &cairo::Context,
        auth_state: AuthState,
        pwd_len: usize,
        buf_width: f64,
        buf_height: f64,
    ) -> Result<()> {
        // Reset the context for fresh rendering
        self.reset_cairo_context(context)?;

        let scale = self.scale.unwrap_or(DEFAULT_SCALE);

        // Draw border colour
        context.save()?;
        self.set_frame_border_color(config, context, auth_state);
        context.set_line_width(config.frame.border * scale);

        let frame_offset = (config.frame.border * scale) / 2.0;
        let frame_w = buf_width - (frame_offset * 2.0);
        let frame_h = buf_height - (frame_offset * 2.0);

        Self::draw_rounded_rect(
            context,
            frame_offset,
            frame_offset,
            frame_w,
            frame_h,
            config.frame.radius * scale,
        );
        context.stroke()?;
        context.restore()?;

        // Skip drawing input box if the password is empty and config flag set
        if pwd_len == 0 && config.input.hide_when_empty {
            return Ok(());
        }

        self.configure_cairo_font(config, context)?;

        let fe = context.font_extents()?;

        let padding_x = config.input.padding_x * buf_width;
        let padding_y = config.input.padding_y * buf_height;

        // Calculate text extents here, so input box width can be determined
        let text = config.input.mask_char.repeat(pwd_len);
        let text_ext = context.text_extents(text.as_str())?;

        let mut inner_w = buf_width * config.input.width;

        if config.input.fit_to_content {
            // Cap computed width to specified width
            inner_w = text_ext.width().min(inner_w);
        }

        let inner_h = fe.height();
        let inner_x = (buf_width - inner_w) / 2.0;
        let inner_y = (buf_height - inner_h) / 2.0;

        let outer_h = inner_h + (padding_y * 2.0) + (config.input.border * scale);
        let outer_w = inner_w + (padding_x * 2.0) + (config.input.border * scale);
        let outer_x = (buf_width - outer_w) / 2.0;
        let outer_y = (buf_height - outer_h) / 2.0;

        context.save()?;

        // Draw the outer rectangle, including padding
        // Outer rectangle should have rounded corners
        Self::draw_rounded_rect(
            context,
            outer_x,
            outer_y,
            outer_w,
            outer_h,
            config.input.radius * outer_h, // radius is relative, Cairo requires absolute
        );
        context.ext_set_source_rgba(config.colors.input_bg);
        context.fill_preserve()?;
        context.ext_set_source_rgba(config.colors.input_border);
        context.set_line_width(config.input.border * scale);
        context.stroke_preserve()?;
        context.clip();

        // Clip text to the inner rectangle
        context.rectangle(inner_x, inner_y, inner_w, inner_h);
        context.clip();

        let text_x = inner_x + (inner_w - text_ext.width()) / 2.0 - text_ext.x_bearing();
        let text_y = inner_y + (inner_h - fe.descent()) / 2.0 + fe.ascent() / 2.0;

        // Actually draw the text
        context.ext_set_source_rgba(config.colors.text);
        context.move_to(text_x, text_y);
        context.show_text(text.as_str())?;

        context.restore()?;

        Ok(())
    }

    pub fn render_background(
        &mut self,
        config: &NLockConfig,
        args: NLockRenderBackgroundArgs,
    ) -> Result<()> {
        let (buf_width, buf_height) = args.get_buffer_dimensions()?;

        self.reset_cairo_context(args.context)?;

        match config.general.bg_type {
            BackgroundType::Color => {
                args.context.ext_set_source_rgba(config.colors.bg);
                args.context.set_operator(cairo::Operator::Source);
            }
            BackgroundType::Image => {
                let image = args
                    .image
                    .ok_or(anyhow!("Surface in image mode, but no image set!"))?;
                self.draw_background_image(
                    args.context,
                    image,
                    buf_width,
                    buf_height,
                    config.image.scale,
                )?;
            }
        }
        args.context.paint()?;

        Ok(())
    }

    pub fn render_overlay(
        &mut self,
        config: &NLockConfig,
        args: NLockRenderOverlayArgs,
    ) -> Result<()> {
        let (buf_width, buf_height) = args.get_buffer_dimensions()?;

        self.draw_overlay(
            config,
            args.context,
            args.auth_state,
            args.pwd_len,
            buf_width,
            buf_height,
        )?;

        Ok(())
    }
}
