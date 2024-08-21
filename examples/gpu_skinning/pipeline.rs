use glam::{Mat4, Vec2};
use miniquad::*;

#[repr(C)]
pub struct Vertex {
    pub positions: [Vec2; 4],
    pub bone_weights: [f32; 4],
    pub bone_indices: [i32; 4],
    pub color: [f32; 4],
    pub uv: Vec2,
    // pub attachment_index: i32,
    // pub attachment_type: i32,
}

impl Vertex {
    pub fn vertex_attributes() -> Vec<VertexAttribute> {
        [
            VertexAttribute::new("position0", VertexFormat::Float2),
            VertexAttribute::new("position1", VertexFormat::Float2),
            VertexAttribute::new("position2", VertexFormat::Float2),
            VertexAttribute::new("position3", VertexFormat::Float2),
            VertexAttribute::new("bone_weights", VertexFormat::Float4),
            VertexAttribute::new("bone_indices", VertexFormat::Int4),
            VertexAttribute::new("color", VertexFormat::Float4),
            VertexAttribute::new("uv", VertexFormat::Float2),
            // VertexAttribute::new("attachment_index", VertexFormat::Int1),
            // VertexAttribute::new("attachment_type", VertexFormat::Int1),
        ]
        .into()
    }
}

#[repr(C)]
pub struct Uniforms {
    pub world: Mat4,
    pub view: Mat4,
    pub bones: [Mat4; 100],
    // pub deform: [f32; 10000],
    // pub deform_offsets: [i32; 100],
    pub attachment_slots: [i32; 100],
    pub slot_bones: [i32; 100],
}

impl Uniforms {
    pub fn uniform_desc() -> Vec<UniformDesc> {
        vec![
            UniformDesc::new("world", UniformType::Mat4),
            UniformDesc::new("view", UniformType::Mat4),
            UniformDesc::new("bones", UniformType::Mat4).array(100),
            // UniformDesc::new("deform", UniformType::Float1).array(10000),
            // UniformDesc::new("deform_offsets", UniformType::Int1).array(100),
            UniformDesc::new("attachment_slots", UniformType::Int1).array(100),
            UniformDesc::new("slot_bones", UniformType::Int1).array(100),
        ]
        .into()
    }
}

const VERTEX: &str = r#"
        #version 300 es
        in vec2 position0;
        in vec2 position1;
        in vec2 position2;
        in vec2 position3;
        in vec4 bone_weights;
        in ivec4 bone_indices;
        in vec4 color;
        in vec2 uv;
        // in int attachment_index;
        // in int attachment_type; // 0 = region, 1 = mesh, 2 = skinned mesh

        uniform mat4 world;
        uniform mat4 view;

        // The transform matrices for each bone.
        uniform mat4 bones[100];

        // The per-slot deform vertices.
        // uniform float deform[10000];

        // A map of the slot index to the offset in the deform array.
        // If the value is -1 then the slot is not deformed.
        // uniform int deform_offsets[100];

        // A map of the attachment index to a slot index.
        // This can be used to find an index into the deform_offsets array.
        uniform int attachment_slots[100];

        // A map of the slot index to the bone index.
        uniform int slot_bones[100];

        out vec2 v_uv;
        out vec4 v_color;

        void main() {
            vec3 skinned_pos = vec3(0.0, 0.0, 0.0);

            // int slot_index = attachment_slots[attachment_index];
            // int bone_index = slot_bones[slot_index];
            // int deform_offset = deform_offsets[slot_index];

            // if (attachment_type == 2) {
            //     // Skinned meshes have multiple bone influences.
            //     bone_index = bone_indices[0];
            //     v_color = color;
                v_color = vec4(1.0, 0.0, 0.0, 0.0);
            // } else if (attachment_type == 1) {
                // v_color = vec4(1.0, 0.0, 0.0, 0.0);
            // } else {
            //     v_color = vec4(1.0, 0.0, 0.0, 0.0);
                v_color = color;
            // }

            // if (deform_offset == -1) {
                // No deform data for this slot.
                // Transform the vertices using the bone data.
                // bone_index = bone_indices[0];
                int bone_index = 20;
                vec4 local_pos = vec4(position0, 0.0, 1.0);
                // skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[0];
                skinned_pos = (bones[bone_index] * local_pos).xyz * bone_weights[0];
                // skinned_pos += local_pos.xyz * 1.0;

                // bone_index = bone_indices[1];
                // local_pos = vec4(position1, 0.0, 1.0);
                // skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[1];

                // bone_index = bone_indices[2];
                // local_pos = vec4(position2, 0.0, 1.0);
                // skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[2];

                // bone_index = bone_indices[3];
                // local_pos = vec4(position3, 0.0, 1.0);
                // skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[3];

                // v_color = color;
            // } else {
                // The slot has deform vertices.
                // For an unweighted mesh, these vertices are the final positions.
                // For a weighted mesh, these vertices are offsets from the original positions.
                // v_color = vec4(1.0, 0.0, 0.0, 1.0);
            // }

            // int vertex_offset = gl_VertexID * 8; 

            // vec2 deformed_pos[4];
            // deformed_pos[0] = position0 + vec2(deform[vertex_offset * 2], deform[vertex_offset * 2 + 1]);
            // deformed_pos[1] = position1 + vec2(deform[vertex_offset * 2 + 2], deform[vertex_offset * 2 + 3]);
            // deformed_pos[2] = position2 + vec2(deform[vertex_offset * 2 + 4], deform[vertex_offset * 2 + 5]);
            // deformed_pos[3] = position3 + vec2(deform[vertex_offset * 2 + 6], deform[vertex_offset * 2 + 7]);

            // uint bone_index;
            // if (current_bone >= 0) {
            //     bone_index = uint(current_bone);
            // } else {
            //     bone_index = bone_indices[0];
            // }

            // if (is_deformed == 1) {
            //     if (is_weighted == 1) {
            //         vec4 local_pos = vec4(deformed_pos[0], 0.0, 1.0);
            //         skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[0];

            //         bone_index = bone_indices[1];
            //         local_pos = vec4(deformed_pos[1], 0.0, 1.0);
            //         skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[1];

            //         bone_index = bone_indices[2];
            //         local_pos = vec4(deformed_pos[2], 0.0, 1.0);
            //         skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[2];

            //         bone_index = bone_indices[3];
            //         local_pos = vec4(deformed_pos[3], 0.0, 1.0);
            //         skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[3];
            //         v_color = color;
            //     } else {
            //         // For unweighted mesh, just use the deformed position
            //         skinned_pos = vec3(deformed_pos[0], 0.0);
            //     v_color = vec4(0.0, 0.0, 0.0, 0.0);
            //     }
            // } else {
            //     vec4 local_pos = vec4(position0, 0.0, 1.0);
            //     skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[0];

            //     bone_index = bone_indices[1];
            //     local_pos = vec4(position1, 0.0, 1.0);
            //     skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[1];

            //     bone_index = bone_indices[2];
            //     local_pos = vec4(position2, 0.0, 1.0);
            //     skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[2];

            //     bone_index = bone_indices[3];
            //     local_pos = vec4(position3, 0.0, 1.0);
            //     skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[3];
            //     v_color = vec4(0.0, 0.0, 0.0, 0.0);
            // }

            gl_Position = view * world * vec4(skinned_pos, 1.0);
            // gl_Position = view * world * vec4(position0, 0.0, 1.0);
            v_uv = uv;
        }
    "#;

const FRAGMENT: &str = r#"
        #version 300 es
        precision mediump float;

        in vec2 v_uv;
        in vec4 v_color;

        uniform sampler2D tex;

        out vec4 fragColor;

        void main() {
            vec4 tex_color = texture(tex, v_uv);
            fragColor = v_color * tex_color;
            // fragColor = vec4(1.0, 0.0, 0.0, 1.0);
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
