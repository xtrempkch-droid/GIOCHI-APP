#version 100
// Textured + instanced vertex shader for GLES 2.0.
//
// The union of `textured.vert` and `instanced.vert`: the mesh supplies position,
// normal and uv, and the model matrix comes from four per-instance attributes.
// The fragment stage is `textured.frag`, unchanged — so a textured instance is
// lit exactly like a textured mesh, which is what lets a sprite batch sample one
// atlas and still match the opaque scenery around it.
//
// # Why the atlas is a texture and not one mesh per sprite
//
// A particle system wants soft round puffs; a hard quad reads as a square. Giving
// each sprite its own texture would cost one draw per sprite. One atlas plus this
// shader costs one draw per *material*, and each material's quad carries the UV
// rectangle of its cell — the cell selection is geometry, not a uniform.
precision highp float;

uniform mat4 u_view_proj;

attribute vec3 a_position;
attribute vec3 a_normal;
attribute vec2 a_uv;
// The instance matrix, one column per attribute slot.
attribute vec4 a_model_0;
attribute vec4 a_model_1;
attribute vec4 a_model_2;
attribute vec4 a_model_3;

varying vec2 v_uv;
varying vec3 v_normal;

void main() {
    mat4 model = mat4(a_model_0, a_model_1, a_model_2, a_model_3);
    vec4 world = model * vec4(a_position, 1.0);

    // The UV comes from the mesh, not the instance: every copy of a quad samples
    // the same atlas cell, which is why a sprite batch is one mesh per material.
    v_uv = a_uv;
    v_normal = mat3(model) * a_normal;

    gl_Position = u_view_proj * world;
}
