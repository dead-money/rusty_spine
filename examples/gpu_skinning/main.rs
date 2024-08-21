mod blend_states;
mod pipeline;
mod spine;
mod texture;

pub use blend_states::*;
pub use pipeline::*;
pub use spine::*;
pub use texture::*;

use glam::{Mat4, Vec2, Vec3};
use miniquad::*;
use rusty_spine::{AttachmentType, Physics, Skeleton};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    vec,
};

// I think I've hit the limits of what I can do with miniquad.
// Too much use of uniforms is causing shader issues. Probably need SSBOs.

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
    texture_delete_queue: Arc<Mutex<Vec<Texture>>>,
    screen_size: Vec2,
    grid_size: usize,
    last_fps_print: f64,
    frame_count: u32,
    fps: f64,
}

impl Stage {
    fn new(ctx: &mut Context, texture_delete_queue: Arc<Mutex<Vec<Texture>>>) -> Stage {
        let spine_demos = vec![
            SpineDemo {
                atlas_path: "assets/spineboy/export/spineboy.atlas",
                skeleton_path: SpineSkeletonPath::Binary(
                    "assets/spineboy/export/spineboy-pro.skel",
                ),
                animation: "portal",
                position: Vec2::new(0., -220.),
                scale: 0.5,
                skin: None,
                backface_culling: true,
            },
            // SpineDemo {
            //     atlas_path: "assets/windmill/export/windmill.atlas",
            //     skeleton_path: SpineSkeletonPath::Json("assets/windmill/export/windmill-ess.json"),
            //     animation: "animation",
            //     position: Vec2::new(0., -80.),
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
            // SpineDemo {
            //     atlas_path: "assets/celestial-circus/export/celestial-circus-pma.atlas",
            //     skeleton_path: SpineSkeletonPath::Json(
            //         "assets/celestial-circus/export/celestial-circus-pro.json",
            //     ),
            //     animation: "swing",
            //     position: Vec2::new(0., -120.),
            //     scale: 0.2,
            //     skin: None,
            //     backface_culling: true,
            // },
        ];

        let current_spine_demo = 0;
        let spine = Spine::load(ctx, spine_demos[current_spine_demo]);

        let pipeline = create_pipeline(ctx);

        Stage {
            spine,
            spine_demos,
            current_spine_demo,
            pipeline,
            last_frame_time: date::now(),
            texture_delete_queue,
            screen_size: Vec2::new(800., 600.),
            grid_size: 1,
            last_fps_print: date::now(),
            frame_count: 0,
            fps: 0.0,
        }
    }

    fn ensure_textures_loaded(&mut self, ctx: &mut Context) {
        let skeleton = &self.spine.controller.skeleton;
        for slot_index in 0..skeleton.slots_count() {
            let Some(slot) = skeleton.draw_order_at_index(slot_index) else {
                continue;
            };

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

    pub fn create_view_transform(&self, row: usize, col: usize) -> Mat4 {
        let grid_size = Vec2::splat(self.grid_size as f32);

        let cell_size = self.screen_size / grid_size;
        let cell_position = Vec2::new(col as f32 * cell_size.x, row as f32 * cell_size.y);
        let cell_center = cell_position + cell_size * 0.75;

        let ortho = self.view();

        let translation = Mat4::from_translation(Vec3::new(
            cell_center.x - self.screen_size.x * 0.5,
            cell_center.y - self.screen_size.y * 0.5,
            0.0,
        ));

        let scale = Mat4::from_scale(Vec3::new(1.0 / grid_size.x, 1.0 / grid_size.y, 1.0));

        ortho * translation * scale
    }

    fn render_scene(&self, ctx: &mut Context, skeleton: &Skeleton) {
        for slot_index in 0..skeleton.slots_count() {
            let Some(slot) = skeleton.draw_order_at_index(slot_index) else {
                continue;
            };

            let Some(attachment) = slot.attachment() else {
                continue;
            };

            let attachment_name = attachment.name();

            let BlendStates {
                alpha_blend,
                color_blend,
            } = slot
                .data()
                .blend_mode()
                .get_blend_states(self.spine.controller.settings.premultiplied_alpha);
            ctx.set_blend(Some(color_blend), Some(alpha_blend));

            // Find the buffer metadata for this slot
            let Some(attachment_meta) = self.spine.buffers.attachments.get(attachment_name) else {
                continue;
            };

            let renderer_object = if let Some(region_attachment) = attachment.as_region() {
                Some(region_attachment.renderer_object_exact())
            } else if let Some(mesh_attachment) = attachment.as_mesh() {
                Some(mesh_attachment.renderer_object_exact())
            } else {
                continue;
            };

            let Some(renderer_object) = renderer_object else {
                continue;
            };

            let spine_texture = unsafe { &mut *(renderer_object as *mut SpineTexture) };

            if let SpineTexture::Loaded(texture) = spine_texture {
                let bindings = Bindings {
                    vertex_buffers: vec![self.spine.buffers.vertex_buffer],
                    index_buffer: self.spine.buffers.index_buffer,
                    images: vec![*texture],
                };
                ctx.apply_bindings(&bindings);

                ctx.draw(attachment_meta.index_start, attachment_meta.index_count, 1);
            }
        }
    }
}

impl EventHandler for Stage {
    fn update(&mut self, _ctx: &mut Context) {
        let now = date::now();
        let dt = ((now - self.last_frame_time) as f32).max(0.001);
        self.spine.controller.update(dt, Physics::Update);

        if (date::now() - self.last_fps_print) >= 0.5 {
            println!(
                "{:.2} FPS -- {} Spines",
                1.0 / dt,
                self.grid_size * self.grid_size
            );
            self.last_fps_print = date::now();
        }

        self.last_frame_time = now;
    }

    fn draw(&mut self, ctx: &mut Context) {
        self.ensure_textures_loaded(ctx);

        // Delete textures that are no longer used
        for texture_delete in self.texture_delete_queue.lock().unwrap().drain(..) {
            texture_delete.delete();
        }

        ctx.begin_default_pass(Default::default());
        ctx.clear(Some((0.1, 0.1, 0.1, 1.0)), None, None);
        ctx.apply_pipeline(&self.pipeline);

        // ctx.set_cull_face(self.spine.cull_face);
        ctx.set_cull_face(CullFace::Nothing);

        let skeleton = &self.spine.controller.skeleton;

        // Extract bone transforms from the skeleton.
        let mut bones = [Mat4::IDENTITY; BONES];
        for bone in skeleton.bones() {
            let bone_index = bone.data().index();

            let transform = Mat4::from_cols_array_2d(&[
                [bone.a(), bone.c(), 0.0, 0.0],
                [bone.b(), bone.d(), 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [bone.world_x(), bone.world_y(), 0.0, 1.0],
            ]);

            bones[bone_index] = transform;
        }

        // Build a map of the attachments currently in use.
        // Also note which slot is assigned to which bone.
        let mut attachment_slots = [0; ATTACHMENT_SLOTS];
        let mut slot_bones = [0; SLOT_BONES];
        for slot in skeleton.slots() {
            let slot_index = slot.data().index();
            let bone_index = slot.bone().data().index();
            slot_bones[slot_index] = bone_index as i32;

            let Some(attachment) = slot.attachment() else {
                continue;
            };

            let attachment_name = attachment.name();
            let Some(attachment_meta) = self.spine.buffers.attachments.get(attachment_name) else {
                continue;
            };

            let attachment_index = attachment_meta.attachment_index as usize;
            attachment_slots[attachment_index] = slot_index as i32;
        }

        // Extract the deform buffers from the skeleton.
        let mut deform_cursor: usize = 0;
        let mut deform_offsets = [-1 as i32; DEFORM_OFFSETS];
        let mut deform = [0.0; DEFORM_SIZE * 2];
        for slot in skeleton.slots() {
            let slot_index = slot.data().index();

            if slot.deform_count() == 0 {
                deform_offsets[slot_index] = -1;
            } else {
                deform_offsets[slot_index] = deform_cursor as i32;

                unsafe {
                    let src = slot.deform();
                    let count = slot.deform_count() as usize;
                    let dst = &mut deform[deform_cursor..deform_cursor + count];
                    std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), count);
                    deform_cursor += count;
                }
            }
        }

        let mut uniforms = Uniforms {
            world: self.spine.world,
            view: self.view(),
            bones,
            deform,
            deform_offsets,
            attachment_slots,
            slot_bones,
        };

        for row in 0..self.grid_size {
            for col in 0..self.grid_size {
                ctx.apply_uniforms(&uniforms);

                // Render the scene for this grid cell
                self.render_scene(ctx, skeleton);
            }
        }

        ctx.end_render_pass();
        ctx.commit_frame();
    }

    fn resize_event(&mut self, ctx: &mut Context, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height) / ctx.dpi_scale();
    }

    fn key_down_event(
        &mut self,
        ctx: &mut Context,
        keycode: KeyCode,
        _keymods: KeyMods,
        repeat: bool,
    ) {
        match keycode {
            KeyCode::Equal | KeyCode::KpAdd => {
                self.grid_size = (self.grid_size + 1).min(100);
            }
            KeyCode::Minus | KeyCode::KpSubtract => {
                self.grid_size = (self.grid_size - 1).max(1);
            }
            _ => {}
        }

        if !repeat && keycode == KeyCode::Space {
            self.current_spine_demo = (self.current_spine_demo + 1) % self.spine_demos.len();
            self.spine = Spine::load(ctx, self.spine_demos[self.current_spine_demo]);
        }
    }
}

#[derive(Debug)]
pub struct AttachmentMeta {
    pub index_start: i32,
    pub index_count: i32,
    pub attachment_index: i32,
}

pub struct SkeletonBuffers {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub attachments: HashMap<String, AttachmentMeta>,
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
