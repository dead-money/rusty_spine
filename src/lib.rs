#[macro_use]
pub mod c_interface;

pub mod animation_state;
pub mod animation_state_data;
pub mod atlas;
pub mod attachment;
pub mod bone;
pub mod c;
pub mod clipping_attachment;
pub mod color;
pub mod error;
pub mod extension;
pub mod mesh_attachment;
pub mod region_attachment;
pub mod renderer_object;
pub mod skeleton;
pub mod skeleton_clipping;
pub mod skeleton_controller;
pub mod skeleton_data;
pub mod skeleton_json;
pub mod slot;
pub mod sync_ptr;
pub mod texture_region;
pub mod tmp_ref;

#[cfg(test)]
pub mod tests;
