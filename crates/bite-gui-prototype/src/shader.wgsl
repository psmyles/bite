struct Screen { size: vec4<f32> }
@group(0) @binding(0) var<uniform> screen: Screen;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;
struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VertexOut {
    var out: VertexOut;
    out.pos = vec4<f32>(pos.x / screen.size.x * 2.0 - 1.0, 1.0 - pos.y / screen.size.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}
@fragment fn fs(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color * textureSample(tex, tex_sampler, input.uv);
}
