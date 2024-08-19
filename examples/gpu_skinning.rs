use glam::{Mat4, Vec2, Vec4};
use miniquad::*;
use rusty_spine::{
    atlas::{AtlasFilter, AtlasFormat, AtlasWrap},
    controller::{SkeletonController, SkeletonControllerSettings},
    draw::{ColorSpace, CullDirection},
    AnimationStateData, Atlas, AttachmentType, BlendMode, Physics, Skeleton, SkeletonBinary,
    SkeletonJson,
};
use std::sync::{Arc, Mutex};

const MAX_MESH_VERTICES: usize = 10000;
const MAX_MESH_INDICES: usize = 20000;
const MAX_BONES: usize = 200;

#[repr(C)]
struct Vertex {
    position: Vec2,
    uv: Vec2,
    color: [f32; 4],
    bone_weights: [f32; 4],
    bone_indices: [u8; 4],
}

#[derive(Debug)]
struct AttachmentInfo {
    slot_index: u16,
    vertex_start: u32,
    vertex_count: u32,
    index_start: u32,
    index_count: u32,
}

struct SkeletonBuffers {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    attachment_info: Vec<AttachmentInfo>,
}

mod shader {
    use glam::{Mat4, Vec4};
    use miniquad::*;

    pub const VERTEX: &str = r#"
        #version 100
        attribute vec2 position;
        attribute vec2 uv;
        attribute vec4 color;
        attribute vec4 weights;
        attribute vec4 indices;

        // uniform mat4 mvp;
        uniform mat4 world;
        uniform mat4 view;
        // uniform vec4 bones[200];

        varying lowp vec2 v_uv;
        varying lowp vec4 v_color;

        void main() {
            // vec4 pos = vec4(position, 0.0, 1.0);
            // vec4 skinned_pos = vec4(0.0);

            // for (int i = 0; i < 4; i++) {
            //     int index = int(indices[i]) * 2;
            //     mat4 bone_matrix = mat4(
            //         bones[index], bones[index + 1],
            //         vec4(0.0, 0.0, 1.0, 0.0),
            //         vec4(0.0, 0.0, 0.0, 1.0)
            //     );
            //     skinned_pos += bone_matrix * pos * weights[i];
            // }

            // gl_Position = view * world * skinned_pos;
            gl_Position = view * world * vec4(position, 0, 1);
            v_uv = uv;
            v_color = color;
            // v_color = vec4(position.x, position.y, 0.0, 1.0);
        }
    "#;

    pub const FRAGMENT: &str = r#"
        #version 100
        varying lowp vec2 v_uv;
        varying lowp vec4 v_color;

        uniform sampler2D tex;

        void main() {
            lowp vec4 tex_color = texture2D(tex, v_uv);
            gl_FragColor = v_color * tex_color;
            gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0);
        }
    "#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: vec![
                    UniformDesc::new("world", UniformType::Mat4),
                    UniformDesc::new("view", UniformType::Mat4),
                    // UniformDesc::new("bones", UniformType::Float4),
                ],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        // pub mvp: Mat4,
        pub world: Mat4,
        pub view: Mat4,
        // pub bones: [Vec4; 400],
    }
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
#[derive(Debug)]
enum SpineTexture {
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

/// Holds all data related to load and demonstrate a particular Spine skeleton.
#[derive(Clone, Copy)]
struct SpineDemo {
    atlas_path: &'static str,
    skeleton_path: SpineSkeletonPath,
    animation: &'static str,
    position: Vec2,
    scale: f32,
    skin: Option<&'static str>,
    backface_culling: bool,
}

#[derive(Clone, Copy)]
enum SpineSkeletonPath {
    Binary(&'static str),
    Json(&'static str),
}

/// Holds all data related to rendering Spine skeletons in this example.
struct Spine {
    controller: SkeletonController,
    world: Mat4,
    cull_face: CullFace,
    buffers: SkeletonBuffers,
}

impl Spine {
    pub fn load(ctx: &mut Context, info: SpineDemo) -> Self {
        // Load atlas and auto-detect if the textures are premultiplied
        let atlas = Arc::new(
            Atlas::new_from_file(info.atlas_path)
                .unwrap_or_else(|_| panic!("failed to load atlas file: {}", info.atlas_path)),
        );
        let premultiplied_alpha = atlas.pages().any(|page| page.pma());

        // Load either binary or json skeleton files
        let skeleton_data = Arc::new(match info.skeleton_path {
            SpineSkeletonPath::Binary(path) => {
                let skeleton_binary = SkeletonBinary::new(atlas);
                skeleton_binary
                    .read_skeleton_data_file(path)
                    .unwrap_or_else(|_| panic!("failed to load binary skeleton file: {path}"))
            }
            SpineSkeletonPath::Json(path) => {
                let skeleton_json = SkeletonJson::new(atlas);
                skeleton_json
                    .read_skeleton_data_file(path)
                    .unwrap_or_else(|_| panic!("failed to load json skeleton file: {path}"))
            }
        });

        // Create animation state data from a skeleton
        // If desired, set crossfades at this point
        // See [`rusty_spine::AnimationStateData::set_mix_by_name`]
        let animation_state_data = Arc::new(AnimationStateData::new(skeleton_data.clone()));

        // Instantiate the [`rusty_spine::controller::SkeletonController`] helper class which
        // handles creating the live data ([`rusty_spine::Skeleton`] and
        // [`rusty_spine::AnimationState`] and capable of generating mesh render data.
        // Use of this helper is not required but it does handle a lot of little things for you.
        let mut controller = SkeletonController::new(skeleton_data, animation_state_data)
            .with_settings(SkeletonControllerSettings {
                premultiplied_alpha,
                cull_direction: CullDirection::CounterClockwise,
                color_space: ColorSpace::SRGB,
            });

        controller
            .animation_state
            .set_animation_by_name(0, info.animation, true)
            .unwrap_or_else(|_| panic!("failed to start animation: {}", info.animation));

        controller.settings.premultiplied_alpha = premultiplied_alpha;

        let (vertices, indices, attachment_info) =
            Self::build_skeleton_buffers(&controller.skeleton);

        let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);
        let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &indices);

        Self {
            controller,
            world: Mat4::from_translation(info.position.extend(0.))
                * Mat4::from_scale(Vec2::splat(info.scale).extend(1.)),
            cull_face: match info.backface_culling {
                false => CullFace::Nothing,
                true => CullFace::Back,
            },
            buffers: SkeletonBuffers {
                vertex_buffer,
                index_buffer,
                attachment_info,
            },
        }
    }

    fn build_skeleton_buffers(skeleton: &Skeleton) -> (Vec<Vertex>, Vec<u16>, Vec<AttachmentInfo>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut attachment_info = Vec::new();

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

            let vertex_start = vertices.len() as u32;
            let index_start = indices.len() as u32;

            match attachment.attachment_type() {
                AttachmentType::Region => {
                    if let Some(region_attachment) = attachment.as_region() {
                        let mut region_vertices = Vec::with_capacity(4);
                        let offsets = region_attachment.offset();
                        let vertex_size = 2;

                        let uvs = region_attachment.uvs();

                        for vertex_index in 0..4 {
                            let vertex = Vertex {
                                position: [
                                    offsets[vertex_index * vertex_size],
                                    offsets[vertex_index * vertex_size + 1],
                                ]
                                .into(),
                                color: region_attachment.color().into(),
                                uv: [uvs[vertex_index * 2], uvs[vertex_index * 2 + 1]].into(),
                                bone_weights: [1.0, 0.0, 0.0, 0.0], // Only influenced by one bone
                                bone_indices: [slot_index as u8, 0, 0, 0], // Use slot index as bone index
                            };
                            region_vertices.push(vertex);
                        }

                        // Add vertices to the main vertex list.
                        let base_index = vertices.len() as u16;
                        vertices.extend(region_vertices);

                        // Add indices for two triangles (quad)
                        indices.extend_from_slice(&[
                            base_index,
                            base_index + 1,
                            base_index + 2,
                            base_index + 2,
                            base_index + 3,
                            base_index,
                        ]);
                    }
                }
                AttachmentType::Mesh => {
                    if let Some(mesh_attachment) = attachment.as_mesh() {
                        continue;
                        if mesh_attachment.has_bones() {
                            let vertex_size = 3;
                            let vertex_count = mesh_attachment.vertices().len() / vertex_size;
                            let vertices_data = mesh_attachment.vertices();

                            let uvs = mesh_attachment.uvs();
                            let bones = mesh_attachment.bones();

                            let mut vertex_index = 0 as usize;
                            let mut bone_index = 0 as usize;

                            for vertex_index in 0..vertex_count {
                                // let bone_count = bones[bone_index] as usize;
                                // bone_index += 1;

                                let mut bone_weights = [0.0; 4];
                                let mut bone_indices = [0; 4];
                                let mut position = [0.0, 0.0];

                                position[0] = vertices_data[vertex_index * vertex_size];
                                position[1] = vertices_data[vertex_index * vertex_size + 1];

                                // for j in 0..bone_count.min(4) {
                                //     bone_indices[j] = bones[bone_index + j] as u8;
                                //     let vx = vertices_data[vertex_index * 3];
                                //     let vy = vertices_data[vertex_index * 3 + 1];
                                //     let weight = vertices_data[vertex_index * 3 + 2];
                                //     bone_weights[j] = weight;
                                //     position[0] += vx; // * weight;
                                //     position[1] += vy; // * weight;
                                //     // vertex_index += 1;
                                // }

                                // Skip any additional bones if there are more than 4
                                // if bone_count > 4 {
                                //     vertex_index += bone_count - 4;
                                // }

                                // Normalize weights
                                // let total_weight: f32 = bone_weights.iter().sum();
                                // bone_weights.iter_mut().for_each(|w| *w /= total_weight);

                                let uv = unsafe {
                                    [
                                        *uvs.offset(vertex_index as isize * 2),
                                        *uvs.offset(vertex_index as isize * 2 + 1),
                                    ]
                                };

                                let vertex = Vertex {
                                    position: position.into(),
                                    color: mesh_attachment.color().into(),
                                    uv: uv.into(),
                                    bone_weights,
                                    bone_indices,
                                };

                                vertices.push(vertex);
                            }
                        } else {
                            // Not Skinned
                            let vertex_size = 2;
                            let vertex_count = mesh_attachment.vertices().len() / vertex_size;
                            let vertices_data = mesh_attachment.vertices();

                            let uvs = mesh_attachment.uvs();

                            for vertex_index in 0..vertex_count {
                                let mut position = [0.0, 0.0];

                                position[0] = vertices_data[vertex_index * vertex_size];
                                position[1] = vertices_data[vertex_index * vertex_size + 1];

                                // Get UVs
                                let uv = unsafe {
                                    [
                                        *uvs.offset(vertex_index as isize * 2),
                                        *uvs.offset(vertex_index as isize * 2 + 1),
                                    ]
                                };

                                let vertex = Vertex {
                                    position: position.into(),
                                    color: mesh_attachment.color().into(),
                                    uv: uv.into(),
                                    bone_weights: [1.0, 0.0, 0.0, 0.0], // Only influenced by one bone
                                    bone_indices: [0, 0, 0, 0], // Index 0 represents the slot's bone
                                };

                                vertices.push(vertex);
                            }
                        }

                        let index_count = mesh_attachment.triangles_count() as usize;
                        let indices_data = mesh_attachment.triangles();

                        unsafe {
                            let vertex_offset = vertices.len() as u16;
                            for i in 0..index_count {
                                indices
                                    .push(vertex_offset + *indices_data.offset(i as isize) as u16);
                            }
                        }

                        // for i in (0..mesh_attachment.triangles_count() as isize).step_by(3) {
                        //     unsafe {
                        //         mesh_indices
                        //             .push(vertex_base + *mesh_attachment.triangles().offset(i));
                        //         mesh_indices
                        //             .push(vertex_base + *mesh_attachment.triangles().offset(i + 1));
                        //         mesh_indices
                        //             .push(vertex_base + *mesh_attachment.triangles().offset(i + 2));
                        //         // copy_uvs!(i);
                        //     }
                        // }

                        // indices.extend(mesh_indices);
                    }
                }
                _ => {}
            }

            //
            let metadata = AttachmentInfo {
                slot_index: slot_index as u16,
                vertex_start,
                vertex_count: (vertices.len() as u32 - vertex_start),
                index_start,
                index_count: (indices.len() as u32 - index_start),
            };

            println!("metadata: {:?}", metadata);

            attachment_info.push(metadata);
        }

        (vertices, indices, attachment_info)
    }

    fn get_bone_transforms(&self) -> Vec<Mat4> {
        self.controller
            .skeleton
            .bones()
            .map(|bone| {
                Mat4::from_cols(
                    Vec4::new(bone.a(), bone.c(), 0.0, 0.0),
                    Vec4::new(bone.b(), bone.d(), 0.0, 0.0),
                    Vec4::new(0.0, 0.0, 1.0, 0.0),
                    Vec4::new(bone.world_x(), bone.world_y(), 0.0, 1.0),
                )
            })
            .collect()
    }
}

/// Convert a [`rusty_spine::BlendMode`] to a pair of [`miniquad::BlendState`]s. One for alpha, one
/// for color.
///
/// Spine supports 4 different blend modes:
/// - [`rusty_spine::BlendMode::Additive`]
/// - [`rusty_spine::BlendMode::Multiply`]
/// - [`rusty_spine::BlendMode::Normal`]
/// - [`rusty_spine::BlendMode::Screen`]
///
/// And blend states are different depending on if the texture has premultiplied alpha values.
///
/// So, 8 blend states must be supported. See [`GetBlendStates::get_blend_states`] below.
struct BlendStates {
    alpha_blend: BlendState,
    color_blend: BlendState,
}

trait GetBlendStates {
    fn get_blend_states(&self, premultiplied_alpha: bool) -> BlendStates;
}

impl GetBlendStates for BlendMode {
    fn get_blend_states(&self, premultiplied_alpha: bool) -> BlendStates {
        match self {
            Self::Additive => match premultiplied_alpha {
                // Case 1: Additive Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::One,
                    ),
                },
                // Case 2: Additive Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One),
                    color_blend: BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One),
                },
            },
            Self::Multiply => match premultiplied_alpha {
                // Case 3: Multiply Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::DestinationColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
                // Case 4: Multiply Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::DestinationColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
            },
            Self::Normal => match premultiplied_alpha {
                // Case 5: Normal Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
                // Case 6: Normal Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
            },
            Self::Screen => match premultiplied_alpha {
                // Case 7: Screen Blend Mode, Normal Alpha
                false => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
                // Case 8: Screen Blend Mode, Premultiplied Alpha
                true => BlendStates {
                    alpha_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::OneMinusValue(BlendValue::SourceColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    color_blend: BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                },
            },
        }
    }
}

struct Stage {
    spine: Spine,
    spine_demos: Vec<SpineDemo>,
    current_spine_demo: usize,
    pipeline: Pipeline,
    last_frame_time: f64,
    bindings: Bindings,
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

        let bindings = Bindings {
            vertex_buffers: vec![spine.buffers.vertex_buffer],
            index_buffer: spine.buffers.index_buffer,
            images: vec![Texture::empty()],
        };

        Stage {
            spine,
            spine_demos,
            current_spine_demo,
            pipeline,
            last_frame_time: date::now(),
            bindings,
            texture_delete_queue,
            screen_size: Vec2::new(800., 600.),
        }
    }

    fn create_pipeline(ctx: &mut Context) -> Pipeline {
        let shader = Shader::new(ctx, shader::VERTEX, shader::FRAGMENT, shader::meta())
            .expect("failed to build shader");

        Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("position", VertexFormat::Float2),
                VertexAttribute::new("uv", VertexFormat::Float2),
                VertexAttribute::new("color", VertexFormat::Float4),
                // VertexAttribute::new("dark_color", VertexFormat::Float4),
                VertexAttribute::new("weights", VertexFormat::Float4),
                VertexAttribute::new("indices", VertexFormat::Float4),
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
                // clipper?
                continue;
            }

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
        ctx.clear(Some((0.1, 0.2, 0.3, 1.0)), None, None);

        let skeleton = &self.spine.controller.skeleton;
        for slot_index in 0..skeleton.slots_count() {
            let Some(slot) = skeleton.draw_order_at_index(slot_index) else {
                continue;
            };

            if !slot.bone().active() {
                // clipper? ignore for now
                continue;
            }

            let Some(attachment) = slot.attachment() else {
                continue;
            };

            ctx.apply_pipeline(&self.pipeline);

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

            if let SpineTexture::Loaded(texture) = spine_texture {
                self.bindings.images[0] = *texture;
            }

            ctx.apply_bindings(&self.bindings);

            // Find the buffer metadata for this slot.
            let Some(attachment_info) = self
                .spine
                .buffers
                .attachment_info
                .iter()
                .find(|info| info.slot_index == slot_index as u16)
            else {
                continue;
            };

            // Set up attachment-specific uniforms
            let bone = slot.bone();
            let bone_transform = Mat4::from_cols_array_2d(&[
                [bone.a(), bone.b(), 0.0, 0.0],
                [bone.c(), bone.d(), 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [bone.world_x(), bone.world_y(), 0.0, 1.0],
            ]);

            ctx.apply_uniforms(&shader::Uniforms {
                world: self.spine.world * bone_transform,
                view: self.view(),
                //     bones: bone_data,
            });

            ctx.draw(
                attachment_info.index_start as i32,
                attachment_info.index_count as i32,
                1,
            );

            // let BlendStates {
            //     alpha_blend,
            //     color_blend,
            // } = slot
            //     .data()
            //     .blend_mode
            //     .get_blend_states(self.spine.controller.settings.premultiplied_alpha);
            // ctx.set_blend(Some(color_blend), Some(alpha_blend));

            // let mut out_vertices: Vec<Vertex> = vec![];
            // let mut out_indices = vec![];

            // match attachment.attachment_type() {
            //     AttachmentType::Region => {
            //         if let Some(region_attachment) = attachment.as_region() {
            //             let bones = region_attachment.bones();
            //             let vertices = region_attachment.vertices();
            //             let uvs = region_attachment.uvs();
            //             let color = region_attachment.color();

            //             let bone = slot.bone();
            //             let bone_index = bone.data().index();

            //             // Region attachments typically have 4 vertices?
            //             for i in 0..4 {
            //                 out_vertices.push(Vertex {
            //                     position: Vec2::new(vertices[i][0], vertices[i][1]),
            //                     uv: Vec2::new(uvs[i][0], uvs[i][1]),
            //                     color: color.into(),
            //                     weights: [1.0, 0.0, 0.0, 0.0],
            //                     indices: [bone_index as f32, 0.0, 0.0, 0.0],
            //                 });
            //             }
            //         }
            //     }
            //     AttachmentType::Mesh => {
            //         if let Some(mesh_attachment) = attachment.as_mesh() {
            //             if !mesh_attachment.has_bones() {
            //                 // Non-skinned mesh.
            //                 // let bone = slot.bone();
            //                 // let bone_index = bone.data().index();

            //                 // for i in 0..renderable.vertices.len() {
            //                 //     out_vertices.push(Vertex {
            //                 //         position: Vec2::new(
            //                 //             renderable.vertices[i][0],
            //                 //             renderable.vertices[i][1],
            //                 //         ),
            //                 //         uv: Vec2::new(renderable.uvs[i][0], renderable.uvs[i][1]),
            //                 //         color: mesh_attachment.color().into(),
            //                 //         weights: [1.0, 0.0, 0.0, 0.0],
            //                 //         indices: [bone_index as f32, 0.0, 0.0, 0.0],
            //                 //     });
            //                 // }

            //                 continue;
            //             }

            //             continue;

            //             // let mesh_bones = mesh_attachment.bones();
            //             // let mesh_vertices = mesh_attachment.vertices();
            //             // let world_vertices_length =
            //             //     mesh_attachment.world_vertices_length() as usize;

            //             // let mut v = 0;
            //             // let mut b = 0;
            //             // let mut vertex_index = 0;

            //             // while vertex_index < world_vertices_length / 2 {
            //             //     if v >= mesh_bones.len() {
            //             //         println!("Warning: Ran out of bone data.");
            //             //         break;
            //             //     }

            //             //     let n = mesh_bones[v] as usize;
            //             //     v += 1;

            //             //     if v + n > mesh_bones.len() {
            //             //         println!("Warning: Not enough bone data.");
            //             //         break;
            //             //     }

            //             //     let mut wx = 0.0;
            //             //     let mut wy = 0.0;

            //             //     let mut vertex_weights = [0.0; 4];
            //             //     let mut vertex_indices = [0.0; 4];

            //             //     for j in 0..n.min(4) {
            //             //         if b + 2 >= mesh_vertices.len() {
            //             //             println!("Warning: Not enough vertex data. Stopping mesh processing.");
            //             //             break;
            //             //         }

            //             //         let bone_index = mesh_bones[v] as usize;
            //             //         let vx = mesh_vertices[b];
            //             //         let vy = mesh_vertices[b + 1];
            //             //         let weight = mesh_vertices[b + 2];

            //             //         vertex_weights[j] = weight;
            //             //         vertex_indices[j] = bone_index as f32;

            //             //         // In a full implementation, we'd use these to compute wx and wy
            //             //         // wx += (vx * bone.a + vy * bone.b + bone.world_x) * weight;
            //             //         // wy += (vx * bone.c + vy * bone.d + bone.world_y) * weight;

            //             //         v += 1;
            //             //         b += 3;
            //             //     }

            //             //     // Skip any remaining bones for this vertex.
            //             //     v += n.saturating_sub(4);
            //             //     b += 3 * n.saturating_sub(4);

            //             //     // Normalize weights
            //             //     let weight_sum: f32 = vertex_weights.iter().sum();
            //             //     if weight_sum > 0.0 {
            //             //         for w in &mut vertex_weights {
            //             //             *w /= weight_sum;
            //             //         }
            //             //     }

            //             //     if vertex_index < renderable.vertices.len() {
            //             //         out_vertices.push(Vertex {
            //             //             position: Vec2::new(
            //             //                 renderable.vertices[vertex_index][0],
            //             //                 renderable.vertices[vertex_index][1],
            //             //             ),
            //             //             uv: Vec2::new(
            //             //                 renderable.uvs[vertex_index][0],
            //             //                 renderable.uvs[vertex_index][1],
            //             //             ),
            //             //             color: mesh_attachment.color().into(),
            //             //             weights: vertex_weights,
            //             //             indices: vertex_indices,
            //             //         });
            //             //     } else {
            //             //         println!("Warning: More vertices in mesh data than in renderable");
            //             //     }

            //             //     vertex_index += 1;
            //             // }
            //         }
            //     }
            //     _ => {
            //         // Not yet implemented.
            //     }
            // }

            // self.vertex_buffer.update(ctx, &out_vertices);
            // self.index_buffer.update(ctx, &renderable.indices);

            // if let Some(SpineTexture::Loaded(texture)) = renderable
            //     .attachment_renderer_object
            //     .map(|obj| unsafe { &*(obj as *const SpineTexture) })
            // {
            //     self.bindings.images[0] = *texture;
            // }

            // ctx.apply_bindings(&self.bindings);

            // ctx.set_cull_face(self.spine.cull_face);

            // // Update bone uniforms
            // let bone_transforms = self.spine.get_bone_transforms();
            // let mut bone_data = [Vec4::ZERO; MAX_BONES * 2];
            // for (i, transform) in bone_transforms.iter().enumerate().take(MAX_BONES) {
            //     bone_data[i * 2] = transform.x_axis;
            //     bone_data[i * 2 + 1] = transform.y_axis;
            // }

            // let view = self.view();

            // ctx.apply_uniforms(&shader::Uniforms {
            //     world: self.spine.world,
            //     view,
            //     bones: bone_data,
            // });

            // ctx.draw(0, renderable.indices.len() as i32, 1);
        }

        ctx.end_render_pass();
        ctx.commit_frame();
    }
}

fn main() {
    rusty_spine::extension::set_create_texture_cb(|atlas_page, path| {
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
    });

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
