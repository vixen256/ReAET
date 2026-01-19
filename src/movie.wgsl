const Kb = 0.0722;
const Kr = 0.2126;
const Kg = 1.0 - Kb - Kr;

const YCbCrRgbMatrix = mat3x3 (
	1.0, 0.0, 2.0 - 2.0 * Kr,
	1.0, -(Kb / Kg) * (2.0 - 2.0 * Kb), -(Kr / Kg) * (2.0 - 2.0 * Kr),
	1.0, 2.0 - 2.0 * Kb, 0.0,
);

@group(0) @binding(0)
var Sampler: sampler;
@group(0) @binding(1)
var YTexture: texture_2d<f32>;
@group(0) @binding(2)
var CbTexture: texture_2d<f32>;
@group(0) @binding(3)
var CrTexture: texture_2d<f32>;

struct VertexInput {
	@location(0) position: vec2<f32>,
	@location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
	@builtin(position) position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.position = vec4(in.position, 0.0, 1.0);
	out.tex_coords = in.tex_coords;
	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	var y = textureSample(YTexture, Sampler, in.tex_coords).x;
	var cb = textureSample(CbTexture, Sampler, in.tex_coords).x - (128.0 / 255.0);
	var cr = textureSample(CrTexture, Sampler, in.tex_coords).x - (128.0 / 255.0);
	var rgb = vec3(y, cb, cr) * YCbCrRgbMatrix;
	return vec4(rgb, 1.0);
}
