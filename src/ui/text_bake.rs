use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub padding: u32,
    pub text_color: [u8; 4],
    pub background_color: [u8; 4],
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 24.0,
            line_height: 28.0,
            width: None,
            height: None,
            padding: 4,
            text_color: [255, 255, 255, 255],
            background_color: [0, 0, 0, 0],
        }
    }
}

#[derive(Debug, Clone)]
struct BakedTextImage {
    pub width: u32,
    pub height: u32,
    pub pixels_rgba8: Vec<u8>,
}

pub struct CpuTextBaker {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl CpuTextBaker {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    pub fn bake_rgba8(&mut self, text: &str, style: TextStyle) -> Result<Image> {
        let metrics = Metrics::new(style.font_size, style.line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        {
            let mut buffer = buffer.borrow_with(&mut self.font_system);

            buffer.set_size(style.width, style.height);

            let attrs = Attrs::new();
            buffer.set_text(text, &attrs, Shaping::Advanced, None);

            // Force shaping/layout
            buffer.shape_until_scroll(true);
        }

        // Conservative output size:
        // - if caller supplied width/height, use them
        // - otherwise estimate from layout runs
        let (content_w, content_h) = measure_buffer(&buffer);

        let width = style
            .width
            .map(|v| v.ceil() as u32)
            .unwrap_or(content_w)
            .max(1)
            + style.padding * 2;

        let height = style
            .height
            .map(|v| v.ceil() as u32)
            .unwrap_or(content_h)
            .max(1)
            + style.padding * 2;

        let mut raw_pixels = vec![0u8; (width * height * 4) as usize];

        // We should never have leftover pixels
        let pixels_ref = raw_pixels.as_chunks_mut().0;

        if style.background_color != [0, 0, 0, 0] {
            clear_rgba(pixels_ref, style.background_color);
        }

        let text_color = Color::rgba(
            style.text_color[0],
            style.text_color[1],
            style.text_color[2],
            style.text_color[3],
        );

        {
            let mut buffer = buffer.borrow_with(&mut self.font_system);

            // Re-set size to the final canvas minus padding
            let inner_w = width.saturating_sub(style.padding * 2) as f32;
            let inner_h = height.saturating_sub(style.padding * 2) as f32;
            buffer.set_size(Some(inner_w), Some(inner_h));
            buffer.shape_until_scroll(true);

            let pad_x = style.padding as i32;
            let pad_y = style.padding as i32;

            buffer.draw(&mut self.swash_cache, text_color, |x, y, w, h, color| {
                // cosmic-text gives us rectangles to blend into.
                // We paint them into our RGBA8 image.
                blend_rect(
                    pixels_ref,
                    width,
                    height,
                    x as i32 + pad_x,
                    y as i32 + pad_y,
                    w as i32,
                    h as i32,
                    [color.r(), color.g(), color.b(), color.a()],
                );
            });
        }

        Ok(baked_text_to_bevy_image(BakedTextImage {
            width,
            height,
            pixels_rgba8: raw_pixels,
        }))
    }
}

#[inline]
fn baked_text_to_bevy_image(baked: BakedTextImage) -> Image {
    Image::new(
        Extent3d {
            width: baked.width,
            height: baked.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        baked.pixels_rgba8,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn measure_buffer(buffer: &Buffer) -> (u32, u32) {
    let mut max_right = 0.0f32;
    let mut max_bottom = 0.0f32;

    for run in buffer.layout_runs() {
        let line_top = run.line_top;
        let line_height = run.line_height;

        max_bottom = max_bottom.max(line_top + line_height);

        for glyph in run.glyphs.iter() {
            let right = glyph.x + glyph.w;
            max_right = max_right.max(right);
        }
    }

    (max_right.ceil() as u32, max_bottom.ceil() as u32)
}

#[inline]
fn clear_rgba(pixels: &mut [[u8; 4]], color: [u8; 4]) {
    for px in pixels {
        *px = color;
    }
}

fn blend_rect(
    pixels: &mut [[u8; 4]],
    tex_w: u32,
    tex_h: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    src: [u8; 4],
) {
    let x0 = x.max(0) as u32;
    let y0 = y.max(0) as u32;
    let x1 = (x + w).max(0) as u32;
    let y1 = (y + h).max(0) as u32;

    let x1 = x1.min(tex_w);
    let y1 = y1.min(tex_h);

    for yy in y0..y1 {
        for xx in x0..x1 {
            let idx = (yy * tex_w + xx) as usize;
            alpha_over(&mut pixels[idx], src);
        }
    }
}

fn alpha_over(dst: &mut [u8; 4], src: [u8; 4]) {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;

    let out_a = sa + da * (1.0 - sa);

    let blend = |s: u8, d: u8| -> u8 {
        if out_a <= 0.0 {
            return 0;
        }
        let s = s as f32 / 255.0;
        let d = d as f32 / 255.0;
        let out = (s * sa + d * da * (1.0 - sa)) / out_a;
        (out * 255.0).round().clamp(0.0, 255.0) as u8
    };

    *dst = [
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ];
}
