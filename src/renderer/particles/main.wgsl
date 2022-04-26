struct Uniforms {
    mvp_matrix: mat4x4<f32>;
    m_mat: mat4x4<f32>;
    v_mat: mat4x4<f32>;
    p_mat: mat4x4<f32>;
};

struct Instance {
    position: vec3<f32>;
    color: vec3<f32>;
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;
@group(0) @binding(1)
var<storage, read> instances: array<Instance>;

struct VertexOut {
    @builtin(position) position: vec4<f32>;
    @location(0) color: vec4<f32>;
};

@stage(vertex)
fn vs_main(
    @location(0) vertex_position: vec3<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    let instance = instances[instance_index];

    var position = uniforms.v_mat * uniforms.m_mat * vec4<f32>(instance.position, 1.0);
    position += vec4<f32>(vertex_position, 1.0);

    var out: VertexOut;
    out.position = uniforms.p_mat * position;
    out.color = vec4<f32>(vec3<f32>(vertex_position + 0.5), 1.0);

    return out;
}

@stage(fragment)
fn fs_main(
  @location(0) color: vec4<f32>
) -> @location(0) vec4<f32> {
    return color;
}