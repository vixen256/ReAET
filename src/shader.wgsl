const Kb = 0.0722;
const Kr = 0.2126;
const Kg = 1.0 - Kb - Kr;

const YCbCrRgbMatrix = mat3x3 (
	1.0, 0.0, 2.0 - 2.0 * Kr,
	1.0, -(Kb / Kg) * (2.0 - 2.0 * Kb), -(Kr / Kg) * (2.0 - 2.0 * Kr),
	1.0, 2.0 - 2.0 * Kb, 0.0,
);

const CBCR_MULT = 256.0 / 255.0;
const CBCR_SUB = 128.50196 / 255.0;

struct VertexInput {
	@location(0) position: vec2<f32>,
	@location(1) tex_index: u32,
}

struct SpriteInfo {
	matrix: mat4x4<f32>,
	tex_coords: array<vec4<f32>, 4>, // Array stride must be 16
	color: vec4<f32>,
	texture_index: u32,
	is_ycbcr: u32,
};

@group(1) @binding(0)
var<uniform> spr: SpriteInfo;

struct VertexOutput {
	@builtin(position) position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.position = spr.matrix * vec4(in.position, 0.0, 1.0);
	out.tex_coords = spr.tex_coords[in.tex_index].xy;
	return out;
}


@group(0) @binding(0)
var Texture: texture_2d<f32>;
@group(0) @binding(1)
var Sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	if spr.is_ycbcr == 1 {
		var ya = textureSampleLevel(Texture, Sampler, in.tex_coords, 0.0).xy;
		var cbcr = textureSampleLevel(Texture, Sampler, in.tex_coords, 1.0).xy * CBCR_MULT - CBCR_SUB;
		var rgb = vec3(ya.x, cbcr) * YCbCrRgbMatrix;
		return vec4(rgb, ya.y) * spr.color;
	} else {
		var rgba = textureSample(Texture, Sampler, in.tex_coords);
		return rgba * spr.color;
	}
}
