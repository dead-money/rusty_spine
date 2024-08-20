use super::*;
use glam::{Mat4, Vec2, Vec4};
use miniquad::*;
use rusty_spine::{
    controller::{SkeletonController, SkeletonControllerSettings},
    draw::{ColorSpace, CullDirection},
    AnimationStateData, Atlas, AttachmentType, Skeleton, SkeletonBinary, SkeletonJson,
};
use std::sync::{Arc, Mutex};

/// Holds all data related to rendering Spine skeletons in this example.
pub struct Spine {
    pub controller: SkeletonController,
    pub world: Mat4,
    pub cull_face: CullFace,
    pub buffers: SkeletonBuffers,
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

        // controller.animation_state.set_timescale(0.1);

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

    /// For a fully GPU skinned and instanced skeleton, we prepare buffers for
    /// vertex, index, and bone weight data at load time.
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

            let bone_index = slot_index;
            // let bone_index = slot.bone().data().index();

            let vertex_start = vertices.len() as u32;
            let index_start = indices.len() as u32;

            if let Some(region_attachment) = attachment.as_region() {
                let mut region_vertices = Vec::with_capacity(4);

                // Offset contains the local position of the vertices.
                let offsets = region_attachment.offset();
                let vertex_size = 2;
                let mut positions = [Vec2::ZERO; 4];

                let uvs = region_attachment.uvs();

                for vertex_index in 0..4 {
                    positions[0] = Vec2::new(
                        offsets[vertex_index * vertex_size],
                        offsets[vertex_index * vertex_size + 1],
                    );

                    region_vertices.push(Vertex {
                        positions,
                        bone_weights: [1.0, 0.0, 0.0, 0.0],
                        bone_indices: [
                            bone_index as f32,
                            bone_index as f32,
                            bone_index as f32,
                            bone_index as f32,
                        ],
                        color: region_attachment.color().into(),
                        uv: [uvs[vertex_index * 2], uvs[vertex_index * 2 + 1]].into(),
                    });
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

            if let Some(mesh_attachment) = attachment.as_mesh() {
                // continue;
                if mesh_attachment.has_bones() {
                    let vertex_size = 3;
                    let vertex_count = mesh_attachment.vertices().len() / vertex_size;
                    let vertices_data = mesh_attachment.vertices();

                    let uvs = mesh_attachment.uvs();
                    let bones = mesh_attachment.bones();

                    // let mut vertex_index = 0 as usize;
                    let mut bone_index = 0 as usize;

                    for vertex_index in 0..vertex_count {
                        let bone_count = bones[bone_index] as usize;
                        bone_index += 1;

                        let mut bone_weights = [0.0; 4];
                        let mut bone_indices = [0.0; 4];
                        let mut positions = [Vec2::ZERO; 4];

                        for j in 0..bone_count.min(4) {
                            let vx = vertices_data[vertex_index * 3];
                            let vy = vertices_data[vertex_index * 3 + 1];
                            positions[j] = Vec2::new(vx, vy);

                            let weight = vertices_data[vertex_index * 3 + 2];
                            bone_weights[j] = weight;

                            bone_indices[j] = bones[bone_index + j] as f32;
                        }

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
                            positions,
                            bone_weights,
                            bone_indices,
                            color: mesh_attachment.color().into(),
                            uv: uv.into(),
                        };

                        vertices.push(vertex);
                    }
                } else {
                    // Not Skinned
                    let vertex_size = 2;
                    let vertex_count = mesh_attachment.vertices().len() / vertex_size;
                    let vertices_data = mesh_attachment.vertices();

                    let uvs = mesh_attachment.uvs();

                    let vertex_offset = vertices.len() as u16;

                    for vertex_index in 0..vertex_count {
                        let mut positions = [Vec2::ZERO; 4];

                        positions[0] = Vec2::new(
                            vertices_data[vertex_index * vertex_size],
                            vertices_data[vertex_index * vertex_size + 1],
                        );

                        // Get UVs
                        let uv = unsafe {
                            [
                                *uvs.offset(vertex_index as isize * 2),
                                *uvs.offset(vertex_index as isize * 2 + 1),
                            ]
                        };

                        let vertex = Vertex {
                            positions,
                            bone_weights: [1.0, 0.0, 0.0, 0.0], // Only influenced by one bone
                            bone_indices: [
                                bone_index as f32,
                                bone_index as f32,
                                bone_index as f32,
                                bone_index as f32,
                            ],
                            color: mesh_attachment.color().into(),
                            uv: uv.into(),
                        };

                        vertices.push(vertex);
                    }
                }

                let index_count = mesh_attachment.triangles_count() as usize;
                let indices_data = mesh_attachment.triangles();

                unsafe {
                    let vertex_offset = vertices.len() as u16;
                    for i in 0..index_count {
                        indices.push(vertex_offset + *indices_data.offset(i as isize) as u16);
                    }
                }
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
