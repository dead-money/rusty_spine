mod blend_states;
mod pipeline;
mod spine;
mod texture;

pub use blend_states::*;
pub use pipeline::*;
pub use spine::*;
pub use texture::*;

use glam::{Mat4, Vec2};
use miniquad::*;
use rusty_spine::{AttachmentType, Physics};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    vec,
};

fn main() {
    rusty_spine::extension::set_create_texture_cb(example_create_texture_cb);

    let texture_delete_queue: Arc<Mutex<Vec<Texture>>> = Arc::new(Mutex::new(vec![]));
    let texture_delete_queue_cb = texture_delete_queue.clone();

    rusty_spine::extension::set_dispose_texture_cb(move |atlas_page| unsafe {
        if let Some(SpineTexture::Loaded(texture)) =
            atlas_page.renderer_object().get::<SpineTexture>()
        {
            texture_delete_queue_cb.lock().unwrap().push(*texture);
        }
        atlas_page.renderer_object().dispose::<SpineTexture>();
    });

    miniquad::start(conf::Conf::default(), |ctx| {
        Box::new(Stage::new(ctx, texture_delete_queue))
    });
}

struct Stage {
    spine: Spine,
    spine_demos: Vec<SpineDemo>,
    current_spine_demo: usize,
    pipeline: Pipeline,
    last_frame_time: f64,
    // bindings: Bindings,
    texture_delete_queue: Arc<Mutex<Vec<Texture>>>,
    screen_size: Vec2,
}

impl Stage {
    fn new(ctx: &mut Context, texture_delete_queue: Arc<Mutex<Vec<Texture>>>) -> Stage {
        let spine_demos = vec![
            // SpineDemo {
            //     atlas_path: "assets/spineboy/export/spineboy.atlas",
            //     skeleton_path: SpineSkeletonPath::Binary(
            //         "assets/spineboy/export/spineboy-pro.skel",
            //     ),
            //     animation: "portal",
            //     position: Vec2::new(0., -220.),
            //     scale: 0.5,
            //     skin: None,
            //     backface_culling: true,
            // },
            SpineDemo {
                atlas_path: "assets/alien/export/alien.atlas",
                skeleton_path: SpineSkeletonPath::Json("assets/alien/export/alien-pro.json"),
                animation: "death",
                position: Vec2::new(0., -260.),
                scale: 0.3,
                skin: None,
                backface_culling: true,
            },
        ];

        let current_spine_demo = 0;
        let spine = Spine::load(ctx, spine_demos[current_spine_demo]);

        let pipeline = Self::create_pipeline(ctx);

        // let mut text_system = text::TextSystem::new();
        // let demo_text =
        //     text_system.create_text(ctx, "press space for next demo", 32. * ctx.dpi_scale());

        // let bindings = Bindings {
        //     vertex_buffers: vec![spine.buffers.vertex_buffer],
        //     index_buffer: spine.buffers.index_buffer,
        //     images: vec![Texture::empty()],
        // };

        Stage {
            spine,
            spine_demos,
            current_spine_demo,
            pipeline,
            last_frame_time: date::now(),
            // bindings,
            texture_delete_queue,
            screen_size: Vec2::new(800., 600.),
        }
    }

    fn create_pipeline(ctx: &mut Context) -> Pipeline {
        let shader =
            Shader::new(ctx, VERTEX, FRAGMENT, shader_meta()).expect("failed to build shader");

        Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("position0", VertexFormat::Float2),
                VertexAttribute::new("position1", VertexFormat::Float2),
                VertexAttribute::new("position2", VertexFormat::Float2),
                VertexAttribute::new("position3", VertexFormat::Float2),
                // VertexAttribute::new("dark_color", VertexFormat::Float4),
                VertexAttribute::new("bone_weights", VertexFormat::Float4),
                VertexAttribute::new("bone_indices", VertexFormat::Float4),
                VertexAttribute::new("color", VertexFormat::Float4),
                VertexAttribute::new("uv", VertexFormat::Float2),
            ],
            shader,
        )
    }

    fn ensure_textures_loaded(&mut self, ctx: &mut Context) {
        let skeleton = &self.spine.controller.skeleton;
        for slot_index in 0..skeleton.slots_count() {
            let Some(slot) = skeleton.draw_order_at_index(slot_index) else {
                continue;
            };

            if !slot.bone().active() {
                continue;
            }

            let Some(attachment) = slot.attachment() else {
                continue;
            };

            let renderer_object = match attachment.attachment_type() {
                AttachmentType::Region => {
                    if let Some(region_attachment) = attachment.as_region() {
                        Some(region_attachment.renderer_object_exact())
                    } else {
                        None
                    }
                }
                AttachmentType::Mesh => {
                    if let Some(mesh_attachment) = attachment.as_mesh() {
                        Some(mesh_attachment.renderer_object_exact())
                    } else {
                        None
                    }
                }
                _ => None,
            };

            let Some(renderer_object) = renderer_object else {
                continue;
            };

            let spine_texture = unsafe { &mut *(renderer_object as *mut SpineTexture) };

            if let SpineTexture::NeedsToBeLoaded {
                path,
                min_filter,
                mag_filter,
                x_wrap,
                y_wrap,
                format,
            } = spine_texture
            {
                use image::io::Reader as ImageReader;
                let image = ImageReader::open(&path)
                    .unwrap_or_else(|_| panic!("failed to open image: {}", &path))
                    .decode()
                    .unwrap_or_else(|_| panic!("failed to decode image: {}", &path));
                let texture_params = TextureParams {
                    width: image.width(),
                    height: image.height(),
                    format: *format,
                    ..Default::default()
                };
                let texture = match format {
                    TextureFormat::RGB8 => {
                        Texture::from_data_and_format(ctx, &image.to_rgb8(), texture_params)
                    }
                    TextureFormat::RGBA8 => {
                        Texture::from_data_and_format(ctx, &image.to_rgba8(), texture_params)
                    }
                    _ => unreachable!(),
                };
                texture.set_filter_min_mag(ctx, *min_filter, *mag_filter);
                texture.set_wrap_xy(ctx, *x_wrap, *y_wrap);
                *spine_texture = SpineTexture::Loaded(texture);
            }
        }
    }

    fn view(&self) -> Mat4 {
        Mat4::orthographic_rh_gl(
            self.screen_size.x * -0.5,
            self.screen_size.x * 0.5,
            self.screen_size.y * -0.5,
            self.screen_size.y * 0.5,
            0.,
            1.,
        )
    }
}

impl EventHandler for Stage {
    fn update(&mut self, _ctx: &mut Context) {
        let now = date::now();
        let dt = ((now - self.last_frame_time) as f32).max(0.001);
        self.spine.controller.update(dt, Physics::Update);
        self.last_frame_time = now;
    }

    fn draw(&mut self, ctx: &mut Context) {
        self.ensure_textures_loaded(ctx);

        // Delete textures that are no longer used. The delete call needs to happen here, before
        // rendering, or it may not actually delete the texture.
        for texture_delete in self.texture_delete_queue.lock().unwrap().drain(..) {
            texture_delete.delete();
        }

        ctx.begin_default_pass(Default::default());
        ctx.clear(Some((0.1, 0.1, 0.1, 1.0)), None, None);
        ctx.apply_pipeline(&self.pipeline);

        // Spine data is clockwise by default!
        ctx.set_cull_face(self.spine.cull_face);

        let skeleton = &self.spine.controller.skeleton;

        // Extract bone transforms from the skeleton.
        let mut bones = [Mat4::IDENTITY; 100];
        for bone in skeleton.bones() {
            let bone_index = bone.data().index();

            let transform = Mat4::from_cols_array_2d(&[
                [bone.a(), bone.b(), 0.0, 0.0],
                [bone.c(), bone.d(), 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [bone.world_x(), bone.world_y(), 0.0, 1.0],
            ]);

            bones[bone_index] = transform;
        }

        ctx.apply_uniforms(&Uniforms {
            world: self.spine.world,
            view: self.view(),
            bones,
        });

        for slot_index in 0..skeleton.slots_count() {
            let Some(slot) = skeleton.draw_order_at_index(slot_index) else {
                continue;
            };

            let slot_name = slot.data().name().to_string();

            let BlendStates {
                alpha_blend,
                color_blend,
            } = slot
                .data()
                .blend_mode()
                .get_blend_states(self.spine.controller.settings.premultiplied_alpha);
            ctx.set_blend(Some(color_blend), Some(alpha_blend));

            let bone = slot.bone();
            let bone_index = bone.data().index();

            if !bone.active() {
                continue;
            }

            // Find the buffer metadata for this slot.
            let Some(slot_meta) = self.spine.buffers.slot_meta.get(&slot_index) else {
                continue;
            };

            let Some(attachment) = slot.attachment() else {
                continue;
            };

            let renderer_object = unsafe {
                match attachment.attachment_type() {
                    AttachmentType::Region => {
                        if let Some(region_attachment) = attachment.as_region() {
                            Some(region_attachment.renderer_object_exact())
                        } else {
                            None
                        }
                    }
                    AttachmentType::Mesh => {
                        if let Some(mesh_attachment) = attachment.as_mesh() {
                            Some(mesh_attachment.renderer_object_exact())
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            };

            let Some(renderer_object) = renderer_object else {
                continue;
            };

            let spine_texture = unsafe { &mut *(renderer_object as *mut SpineTexture) };

            let bindings = if let SpineTexture::Loaded(texture) = spine_texture {
                Bindings {
                    vertex_buffers: vec![self.spine.buffers.vertex_buffer],
                    index_buffer: self.spine.buffers.index_buffer,
                    images: vec![*texture],
                }
            } else {
                continue;
            };

            ctx.apply_bindings(&bindings);

            // let bone_transform = bones[bone_index];
            // println!("bone_transform: {:?} {}", bone_transform, bone_index);
            // println!("slot meta: {:?}", slot_meta);

            ctx.draw(slot_meta.index_start, slot_meta.index_count, 1);
        }

        ctx.end_render_pass();
        ctx.commit_frame();
    }

    fn resize_event(&mut self, ctx: &mut Context, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height) / ctx.dpi_scale();
    }
}

const MAX_MESH_VERTICES: usize = 10000;
const MAX_MESH_INDICES: usize = 20000;
const MAX_BONES: usize = 200;

#[derive(Debug)]
pub struct SlotMeta {
    // pub vertex_start: u32,
    // pub vertex_count: u32,
    pub index_start: i32,
    pub index_count: i32,
}

pub struct SkeletonBuffers {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub slot_meta: HashMap<usize, SlotMeta>,
}

/// An instance of this enum is created for each loaded [`rusty_spine::atlas::AtlasPage`] upon
/// loading a [`rusty_spine::Atlas`]. To see how this is done, see the [`main`] function of this
/// example. It utilizes the following callbacks which must be set only once in an application:
/// - [`rusty_spine::extension::set_create_texture_cb`]
/// - [`rusty_spine::extension::set_dispose_texture_cb`]
///
/// The implementation in this example defers loading by setting the texture to
/// [`SpineTexture::NeedsToBeLoaded`] and handling it later, but in other applications, it may be
/// possible to load the textures immediately, or on another thread.

/// Holds all data related to load and demonstrate a particular Spine skeleton.
#[derive(Clone, Copy)]
pub struct SpineDemo {
    atlas_path: &'static str,
    skeleton_path: SpineSkeletonPath,
    animation: &'static str,
    position: Vec2,
    scale: f32,
    skin: Option<&'static str>,
    backface_culling: bool,
}

#[derive(Clone, Copy)]
pub enum SpineSkeletonPath {
    Binary(&'static str),
    Json(&'static str),
}
