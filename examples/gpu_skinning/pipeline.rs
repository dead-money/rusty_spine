use glam::{Mat4, Vec2};
use miniquad::*;

#[repr(C)]
pub struct Vertex {
    pub positions: [Vec2; 4],
    pub bone_weights: [f32; 4],
    pub bone_indices: [u32; 4],
    pub color: [f32; 4],
    pub uv: Vec2,
    pub attachment_info: [i32; 4],
}

impl Vertex {
    pub fn vertex_attributes() -> Vec<VertexAttribute> {
        [
            VertexAttribute::new("position0", VertexFormat::Float2),
            VertexAttribute::new("position1", VertexFormat::Float2),
            VertexAttribute::new("position2", VertexFormat::Float2),
            VertexAttribute::new("position3", VertexFormat::Float2),
            VertexAttribute::new("bone_weights", VertexFormat::Float4),
            VertexAttribute::new("bone_indices", VertexFormat::Float4),
            VertexAttribute::new("color", VertexFormat::Float4),
            VertexAttribute::new("uv", VertexFormat::Float2),
            VertexAttribute::new("attachment_info", VertexFormat::Float4),
        ]
        .into()
    }
}

pub const BONES: usize = 75;
pub const DEFORM_SIZE: usize = 400;
pub const DEFORM_OFFSETS: usize = 75;
pub const ATTACHMENT_SLOTS: usize = 80;
pub const SLOT_BONES: usize = 80;

#[repr(C)]
pub struct Uniforms {
    pub world: Mat4,
    pub view: Mat4,
    pub bones: [Mat4; BONES],
    pub deform: [f32; DEFORM_SIZE * 2],
    pub deform_offsets: [i32; DEFORM_OFFSETS],
    pub attachment_slots: [i32; ATTACHMENT_SLOTS],
    pub slot_bones: [i32; SLOT_BONES],
}

impl Uniforms {
    pub fn uniform_desc() -> Vec<UniformDesc> {
        vec![
            UniformDesc::new("world", UniformType::Mat4),
            UniformDesc::new("view", UniformType::Mat4),
            UniformDesc::new("bones", UniformType::Mat4).array(BONES),
            UniformDesc::new("deform", UniformType::Float2).array(DEFORM_SIZE),
            UniformDesc::new("deform_offsets", UniformType::Int1).array(DEFORM_OFFSETS),
            UniformDesc::new("attachment_slots", UniformType::Int1).array(ATTACHMENT_SLOTS),
            UniformDesc::new("slot_bones", UniformType::Int1).array(SLOT_BONES),
        ]
        .into()
    }
}

const VERTEX: &str = r#"
        #version 460
        in vec2 position0;
        in vec2 position1;
        in vec2 position2;
        in vec2 position3;
        in vec4 bone_weights;
        in uvec4 bone_indices;
        in vec4 color;
        in vec2 uv;
        in ivec4 attachment_info;

        uniform mat4 world;
        uniform mat4 view;

        // Not enough uniform space for a dark color lookup table?

        // The transform matrices for each bone.
        uniform mat4 bones[75];

        // The per-slot deform vertices.
        uniform vec2 deform[400];

        // A map of the slot index to the offset in the deform array.
        // If the value is -1 then the slot is not deformed.
        uniform int deform_offsets[75];

        // A map of the attachment index to a slot index.
        // This can be used to find an index into the deform_offsets array.
        uniform int attachment_slots[80];

        // A map of the slot index to the bone index.
        uniform int slot_bones[80];

        out vec2 v_uv;
        out vec4 v_color;

        vec3 skinned_position(vec4 local_pos[4], int bone_index) {
            vec3 skinned_pos = vec3(0.0);
            
            skinned_pos += (bones[bone_index] * local_pos[0]).xyz * bone_weights[0];

            for (int i=1; i<4; i++) {
                bone_index = int(bone_indices[i]);
                skinned_pos += (bones[bone_index] * local_pos[i]).xyz * bone_weights[i];
            }

            return skinned_pos;
        }

        vec3 unweighted_deform_position(int deform_offset, int vertex_index) {
            vec2 deformed_pos = deform[deform_offset + vertex_index];
            return vec3(deformed_pos, 0.0);
        }

        void main() {
            vec3 skinned_pos = vec3(0.0, 0.0, 0.0);

            int attachment_index = attachment_info[0];
            int attachment_type = attachment_info[1];
            int vertex_index = attachment_info[2];

            int slot_index = attachment_slots[attachment_index];
            int bone_index = slot_bones[slot_index];
            int deform_offset = deform_offsets[slot_index];

            v_color = color;
            v_uv = uv;

            if (attachment_type == 2) {
                // Skinned meshes have multiple bone influences.
                bone_index = int(bone_indices[0]);
            }

            if (deform_offset > 0) {
                // The slot has deform vertices.
                if (attachment_type == 2) {
                    // For a weighted mesh, these vertices are offsets from the original positions.
                    vec4 local_pos[4];
                    local_pos[0] = vec4(position0 + deform[deform_offset + vertex_index + 1], 0.0, 1.0);
                    local_pos[1] = vec4(position1 + deform[deform_offset + vertex_index + 1], 0.0, 1.0);
                    local_pos[2] = vec4(position2 + deform[deform_offset + vertex_index + 2], 0.0, 1.0);
                    local_pos[3] = vec4(position3 + deform[deform_offset + vertex_index + 3], 0.0, 1.0);

                    skinned_pos = skinned_position(local_pos, bone_index);
                } else {
                    // For an unweighted mesh, these vertices are the final positions.
                    skinned_pos = unweighted_deform_position(deform_offset, vertex_index);
                }
                v_color = vec4(1.0, 0.0, 0.0, 1.0);
            } else {
                vec4 local_pos[4];
                local_pos[0] = vec4(position0, 0.0, 1.0);
                local_pos[1] = vec4(position1, 0.0, 1.0);
                local_pos[2] = vec4(position2, 0.0, 1.0);
                local_pos[3] = vec4(position3, 0.0, 1.0);

                skinned_pos = skinned_position(local_pos, bone_index);
            }

            gl_Position = view * world * vec4(skinned_pos, 1.0);
        }
    "#;

const FRAGMENT: &str = r#"
        #version 460
        precision mediump float;

        in vec2 v_uv;
        in vec4 v_color;

        uniform sampler2D tex;

        out vec4 fragColor;

        void main() {
            vec4 tex_color = texture(tex, v_uv);
            fragColor = v_color * tex_color;
        }
    "#;

pub fn create_pipeline(ctx: &mut Context) -> Pipeline {
    let shader = Shader::new(
        ctx,
        VERTEX,
        FRAGMENT,
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: Uniforms::uniform_desc(),
            },
        },
    )
    .expect("failed to build shader");

    Pipeline::new(
        ctx,
        &[BufferLayout::default()],
        &Vertex::vertex_attributes(),
        shader,
    )
}
