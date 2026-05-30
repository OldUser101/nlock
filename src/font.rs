pub trait FontInfo {
    fn font_height(&self) -> f64;
}

pub trait TextInfo {
    fn text_height(&self) -> f64;
    fn text_width(&self) -> f64;
    fn text_x(&self) -> f64;
    fn text_y(&self) -> f64;
}

pub trait ShowText {
    fn show_text(&self, context: &cairo::Context) -> anyhow::Result<()>;
}

#[cfg(not(feature = "pango"))]
mod cairo_backend {
    use anyhow::Result;
    use clap::ValueEnum;
    use serde::Deserialize;

    use crate::{
        config::NLockConfig,
        font::{FontInfo, ShowText, TextInfo},
        render::{DEFAULT_DPI, DEFAULT_SCALE},
    };

    pub struct NLockFont {
        font_height: f64,
        text: String,
        extents: cairo::TextExtents,
    }

    impl NLockFont {
        pub fn new(
            config: &NLockConfig,
            context: &cairo::Context,
            dpi: Option<f64>,
            scale: Option<f64>,
            subpixel: Option<cairo::SubpixelOrder>,
        ) -> Result<Self> {
            let dpi = dpi.unwrap_or(DEFAULT_DPI);
            let scale = scale.unwrap_or(DEFAULT_SCALE);
            let subpixel = subpixel.unwrap_or(cairo::SubpixelOrder::Default);

            let mut fo = cairo::FontOptions::new()?;
            fo.set_hint_style(cairo::HintStyle::Full);
            fo.set_antialias(cairo::Antialias::Subpixel);
            fo.set_subpixel_order(subpixel);

            context.set_font_options(&fo);
            context.select_font_face(
                &config.font.family,
                config.font.slant.into(),
                config.font.weight.into(),
            );
            context.set_font_size((config.font.size / 72.0) * dpi * scale);

            let fe = context.font_extents()?;
            let extents = context.text_extents("")?;

            Ok(Self {
                font_height: fe.height(),
                text: "".to_string(),
                extents,
            })
        }

        pub fn set_text<T>(&mut self, context: &cairo::Context, text: T) -> Result<()>
        where
            T: AsRef<str>,
        {
            self.text = text.as_ref().to_string();
            self.extents = context.text_extents(text.as_ref())?;
            Ok(())
        }
    }

    impl FontInfo for NLockFont {
        fn font_height(&self) -> f64 {
            self.font_height
        }
    }

    impl TextInfo for NLockFont {
        fn text_height(&self) -> f64 {
            self.extents.height()
        }

        fn text_width(&self) -> f64 {
            self.extents.width()
        }

        fn text_x(&self) -> f64 {
            self.extents.x_bearing()
        }

        fn text_y(&self) -> f64 {
            self.extents.y_bearing()
        }
    }

    impl ShowText for NLockFont {
        fn show_text(&self, context: &cairo::Context) -> Result<()> {
            context.show_text(&self.text)?;
            Ok(())
        }
    }

    #[derive(Debug, Deserialize, Copy, Clone, ValueEnum)]
    #[serde(rename_all = "lowercase")]
    pub enum FontSlant {
        Normal,
        Italic,
        Oblique,
    }

    impl From<FontSlant> for cairo::FontSlant {
        fn from(value: FontSlant) -> Self {
            match value {
                FontSlant::Normal => Self::Normal,
                FontSlant::Italic => Self::Italic,
                FontSlant::Oblique => Self::Oblique,
            }
        }
    }

    #[derive(Debug, Deserialize, Copy, Clone, ValueEnum)]
    #[serde(rename_all = "lowercase")]
    pub enum FontWeight {
        Normal,
        Bold,
    }

    impl From<FontWeight> for cairo::FontWeight {
        fn from(value: FontWeight) -> Self {
            match value {
                FontWeight::Normal => Self::Normal,
                FontWeight::Bold => Self::Bold,
            }
        }
    }
}

#[cfg(feature = "pango")]
mod pango_backend {
    use anyhow::Result;
    use clap::ValueEnum;
    use pango::Rectangle;
    use pangocairo::functions::{create_layout, show_layout};
    use serde::Deserialize;

    use crate::{
        config::NLockConfig,
        font::{FontInfo, ShowText, TextInfo},
        render::{DEFAULT_DPI, DEFAULT_SCALE},
    };

    pub struct NLockFont {
        layout: pango::Layout,
        metrics: pango::FontMetrics,
        text: String,
        extents: Rectangle,
    }

    impl NLockFont {
        pub fn new(
            config: &NLockConfig,
            context: &cairo::Context,
            dpi: Option<f64>,
            scale: Option<f64>,
        ) -> Self {
            let dpi = dpi.unwrap_or(DEFAULT_DPI);
            let scale = scale.unwrap_or(DEFAULT_SCALE);

            let mut fd = pango::FontDescription::new();
            fd.set_family(&config.font.family);
            fd.set_style(config.font.slant.into());
            fd.set_weight(config.font.weight.into());
            fd.set_absolute_size(((config.font.size / 72.0) * dpi * scale) * PANGO_SCALE as f64);

            let layout = create_layout(context);
            layout.set_font_description(Some(&fd));

            let p_ctx = layout.context();
            let metrics = p_ctx.metrics(Some(&fd), None);

            layout.set_text("");
            let extents = layout.pixel_extents().0;

            Self {
                layout,
                metrics,
                text: "".to_string(),
                extents,
            }
        }

        pub fn set_text<T>(&mut self, text: T)
        where
            T: AsRef<str>,
        {
            self.text = text.as_ref().to_string();
            self.layout.set_text(text.as_ref());
            self.extents = self.layout.pixel_extents().0;
        }
    }

    impl FontInfo for NLockFont {
        fn font_height(&self) -> f64 {
            (pango_pixels(self.metrics.ascent()) + pango_pixels(self.metrics.descent())) as f64
        }
    }

    impl TextInfo for NLockFont {
        fn text_height(&self) -> f64 {
            self.extents.height() as f64
        }

        fn text_width(&self) -> f64 {
            self.extents.width() as f64
        }

        fn text_x(&self) -> f64 {
            self.extents.x() as f64
        }

        fn text_y(&self) -> f64 {
            self.extents.y() as f64
        }
    }

    impl ShowText for NLockFont {
        fn show_text(&self, context: &cairo::Context) -> Result<()> {
            show_layout(context, &self.layout);
            Ok(())
        }
    }

    #[derive(Debug, Deserialize, Copy, Clone, ValueEnum)]
    #[serde(rename_all = "lowercase")]
    pub enum FontSlant {
        Normal,
        Italic,
        Oblique,
    }

    impl From<FontSlant> for pango::Style {
        fn from(value: FontSlant) -> Self {
            match value {
                FontSlant::Normal => Self::Normal,
                FontSlant::Italic => Self::Italic,
                FontSlant::Oblique => Self::Oblique,
            }
        }
    }

    #[derive(Debug, Deserialize, Copy, Clone, ValueEnum)]
    #[serde(rename_all = "lowercase")]
    pub enum FontWeight {
        Thin,
        Ultralight,
        Light,
        Semilight,
        Book,
        Normal,
        Medium,
        Semibold,
        Bold,
        Ultrabold,
        Heavy,
        Ultraheavy,
    }

    impl From<FontWeight> for pango::Weight {
        fn from(value: FontWeight) -> Self {
            match value {
                FontWeight::Thin => Self::Thin,
                FontWeight::Ultralight => Self::Ultralight,
                FontWeight::Light => Self::Light,
                FontWeight::Semilight => Self::Semilight,
                FontWeight::Book => Self::Book,
                FontWeight::Normal => Self::Normal,
                FontWeight::Medium => Self::Medium,
                FontWeight::Semibold => Self::Semibold,
                FontWeight::Bold => Self::Bold,
                FontWeight::Ultrabold => Self::Ultrabold,
                FontWeight::Heavy => Self::Heavy,
                FontWeight::Ultraheavy => Self::Ultraheavy,
            }
        }
    }

    // Pango scale factor
    const PANGO_SCALE: i32 = 1024;

    #[inline]
    /// Convert Pango units to pixels
    fn pango_pixels(d: i32) -> i32 {
        (d + 512) >> 10
    }
}

#[cfg(not(feature = "pango"))]
pub use cairo_backend::*;
#[cfg(feature = "pango")]
pub use pango_backend::*;
