use glam::{Mat4, Vec2, Vec4};
use miniquad::*;
use rusty_spine::atlas::{AtlasFilter, AtlasFormat, AtlasPage, AtlasWrap};
use std::sync::{Arc, Mutex};

pub fn example_create_texture_cb(atlas_page: &mut AtlasPage, path: &str) {
    fn convert_filter(filter: AtlasFilter) -> FilterMode {
        match filter {
            AtlasFilter::Linear => FilterMode::Linear,
            AtlasFilter::Nearest => FilterMode::Nearest,
            filter => {
                println!("Unsupported texture filter mode: {filter:?}");
                FilterMode::Linear
            }
        }
    }
    fn convert_wrap(wrap: AtlasWrap) -> TextureWrap {
        match wrap {
            AtlasWrap::ClampToEdge => TextureWrap::Clamp,
            AtlasWrap::MirroredRepeat => TextureWrap::Mirror,
            AtlasWrap::Repeat => TextureWrap::Repeat,
            wrap => {
                println!("Unsupported texture wrap mode: {wrap:?}");
                TextureWrap::Clamp
            }
        }
    }
    fn convert_format(format: AtlasFormat) -> TextureFormat {
        match format {
            AtlasFormat::RGB888 => TextureFormat::RGB8,
            AtlasFormat::RGBA8888 => TextureFormat::RGBA8,
            format => {
                println!("Unsupported texture format: {format:?}");
                TextureFormat::RGBA8
            }
        }
    }
    atlas_page
        .renderer_object()
        .set(SpineTexture::NeedsToBeLoaded {
            path: path.to_owned(),
            min_filter: convert_filter(atlas_page.min_filter()),
            mag_filter: convert_filter(atlas_page.mag_filter()),
            x_wrap: convert_wrap(atlas_page.u_wrap()),
            y_wrap: convert_wrap(atlas_page.v_wrap()),
            format: convert_format(atlas_page.format()),
        });
}

#[derive(Debug)]
pub enum SpineTexture {
    NeedsToBeLoaded {
        path: String,
        min_filter: FilterMode,
        mag_filter: FilterMode,
        x_wrap: TextureWrap,
        y_wrap: TextureWrap,
        format: TextureFormat,
    },
    Loaded(Texture),
}
