use super::*;
use glam::{Mat4, Vec2, Vec4};
use miniquad::*;
use rusty_spine::{
    controller::{SkeletonController, SkeletonControllerSettings},
    draw::{ColorSpace, CullDirection},
    AnimationStateData, Atlas, Skeleton, SkeletonBinary, SkeletonJson,
};
use std::sync::Arc;

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

        let (vertices, indices, slot_meta) = Self::build_skeleton_buffers(&controller.skeleton);

        let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);
        let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &indices);

        Self {
            controller,
            world: Mat4::from_translation(info.position.extend(0.))
                * Mat4::from_scale(Vec2::splat(info.scale).extend(1.)),
            cull_face: match info.backface_culling {
                false => CullFace::Nothing,
                true => CullFace::Front,
            },
            buffers: SkeletonBuffers {
                vertex_buffer,
                index_buffer,
                slot_meta,
            },
        }
    }

    /// For a fully GPU skinned and instanced skeleton, we prepare buffers for
    /// vertex, index, and bone weight data at load time.
    fn build_skeleton_buffers(
        skeleton: &Skeleton,
    ) -> (Vec<Vertex>, Vec<u32>, HashMap<usize, SlotMeta>) {
        let mut vertices = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut slot_meta = HashMap::new();

        for slot in skeleton.slots() {
            let bone = slot.bone();
            let bone_name = bone.data().name().to_string();
            let slot_index = slot.data().index();
            let slot_name = slot.data().name().to_string();

            let Some(attachment) = slot.attachment() else {
                continue;
            };

            let bone_index = slot.bone().data().index();
            let v0 = vertices.len() as u32;
            let i0 = indices.len() as i32;

            if let Some(region_attachment) = attachment.as_region() {
                // Offset contains the local position of the vertices.
                let offsets = region_attachment.offset();
                let mut offset_cursor = 0;

                let uvs = region_attachment.uvs();

                let mut positions = [Vec2::ZERO; 4];

                for vertex_index in 0..4 {
                    // Only one influence, the bone of the slot, so we only
                    // write one position per vertex.
                    positions[0] = Vec2::new(offsets[offset_cursor], offsets[offset_cursor + 1]);

                    vertices.push(Vertex {
                        positions,
                        bone_weights: [1.0, 0.0, 0.0, 0.0],
                        bone_indices: [bone_index as u32; 4],
                        color: region_attachment.color().into(),
                        uv: [uvs[offset_cursor], uvs[offset_cursor + 1]].into(),
                    });

                    offset_cursor += 2;
                }

                // Quad Indices
                // ccw:
                // indices.extend_from_slice(&[v0 + 1, v0 + 3, v0 + 2, v0 + 0, v0 + 3, v0 + 1]);

                // cw:
                indices.extend_from_slice(&[v0 + 0, v0 + 2, v0 + 3, v0 + 1, v0 + 2, v0 + 0]);
            }

            if let Some(mesh_attachment) = attachment.as_mesh() {
                if mesh_attachment.has_bones() {
                    let vertices_data = mesh_attachment.vertices();
                    let mut vertices_cursor = 0 as usize;

                    let bones_data = mesh_attachment.bones();
                    let mut bones_cursor = 0 as usize;

                    let vertex_count = (mesh_attachment.world_vertices_length() / 2) as usize;
                    for vertex_index in 0..vertex_count {
                        let bone_count = bones_data[bones_cursor] as usize;
                        bones_cursor += 1;

                        let mut bone_weights = [0.0; 4];
                        let mut bone_indices = [0; 4];
                        let mut positions = [Vec2::ZERO; 4];

                        for j in 0..bone_count.min(4) {
                            let x = vertices_data[vertices_cursor];
                            let y = vertices_data[vertices_cursor + 1];
                            let w = vertices_data[vertices_cursor + 2];
                            let b = bones_data[bones_cursor] as u32;
                            vertices_cursor += 3;

                            positions[j] = Vec2::new(x, y);
                            bone_weights[j] = w;
                            bone_indices[j] = b;
                            bones_cursor += 1;
                        }

                        let uvs = mesh_attachment.uvs();
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

                        vertices.push(vertex)
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
                            bone_indices: [bone_index as u32; 4],
                            color: mesh_attachment.color().into(),
                            uv: uv.into(),
                        };

                        vertices.push(vertex);
                    }
                }

                let index_count = mesh_attachment.triangles_count() as usize;
                let indices_data = mesh_attachment.triangles();
                let vertex_offset = v0 as u32;

                unsafe {
                    let indices_slice = std::slice::from_raw_parts(indices_data, index_count);
                    indices.extend(indices_slice.iter().map(|&i| vertex_offset + i as u32));
                }
            }

            let metadata = SlotMeta {
                index_start: i0,
                index_count: (indices.len() as i32 - i0),
            };

            slot_meta.insert(slot_index, metadata);
        }

        (vertices, indices, slot_meta)
    }
}