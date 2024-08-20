use glam::{Mat4, Vec2};
use miniquad::*;

#[repr(C)]
pub struct Vertex {
    pub positions: [Vec2; 4],
    pub bone_weights: [f32; 4],
    pub bone_indices: [u32; 4],
    pub color: [f32; 4],
    pub uv: Vec2,
}

#[repr(C)]
pub struct Uniforms {
    pub world: Mat4,
    pub view: Mat4,
    pub bones: [Mat4; 100],
    pub bone_index: i32,
}

pub const VERTEX: &str = r#"
        #version 300 es
        in vec2 position0;
        in vec2 position1;
        in vec2 position2;
        in vec2 position3;
        in vec4 bone_weights;
        in uvec4 bone_indices;
        in vec4 color;
        in vec2 uv;

        uniform mat4 world;
        uniform mat4 view;
        uniform mat4 bones[100];
        uniform int current_bone;

        out vec2 v_uv;
        out vec4 v_color;

        void main() {
            vec3 skinned_pos = vec3(0.0, 0.0, 0.0);

            uint bone_index;

            if (current_bone >= 0) {
                bone_index = uint(current_bone);
            } else {
                bone_index = bone_indices[0];
            }

            vec4 local_pos = vec4(position0, 0.0, 1.0);
            skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[0];

            bone_index = bone_indices[1];
            local_pos = vec4(position1, 0.0, 1.0);
            skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[1];

            bone_index = bone_indices[2];
            local_pos = vec4(position2, 0.0, 1.0);
            skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[2];

            bone_index = bone_indices[3];
            local_pos = vec4(position3, 0.0, 1.0);
            skinned_pos += (bones[bone_index] * local_pos).xyz * bone_weights[3];

            gl_Position = view * world * vec4(skinned_pos, 1.0);
            v_uv = uv;
            v_color = color;
        }
    "#;

pub const FRAGMENT: &str = r#"
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

pub fn shader_meta() -> ShaderMeta {
    ShaderMeta {
        images: vec!["tex".to_string()],
        uniforms: UniformBlockLayout {
            uniforms: vec![
                UniformDesc::new("world", UniformType::Mat4),
                UniformDesc::new("view", UniformType::Mat4),
                UniformDesc::new("bones", UniformType::Mat4).array(100),
                UniformDesc::new("current_bone", UniformType::Int1),
            ],
        },
    }
}
