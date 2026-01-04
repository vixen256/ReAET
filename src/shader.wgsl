const Kb = 0.0722;
const Kr = 0.2126;
const Kg = 1.0 - Kb - Kr;

const YCbCrRgbMatrix = mat3x3 (
	1.0, 0.0, 2.0 - 2.0 * Kr,
	1.0, -(Kb / Kg) * (2.0 - 2.0 * Kb), -(Kr / Kg) * (2.0 - 2.0 * Kr),
	1.0, 2.0 - 2.0 * Kb, 0.0,
);

const CBCR_MULT = 256.0 / 255.0;
const CBCR_SUB = 128.0 / 255.0 * CBCR_MULT;

struct VertexInput {
	@location(0) position: vec2<f32>,
	@location(1) tex_index: u32,
}

struct SpriteInfo {
	matrix: mat4x4<f32>,
	tex_coords_tl: vec2<f32>,
	tex_coords_tr: vec2<f32>,
	tex_coords_bl: vec2<f32>,
	tex_coords_br: vec2<f32>,
	color: vec4<f32>,
	matte_tex_coords_tl: vec2<f32>,
	matte_tex_coords_tr: vec2<f32>,
	matte_tex_coords_bl: vec2<f32>,
	matte_tex_coords_br: vec2<f32>,
	is_ycbcr: u32,
	has_matte: u32,
	matte_is_ycbcr: u32,
};

@group(1) @binding(0)
var<uniform> spr: SpriteInfo;

struct VertexOutput {
	@builtin(position) position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) matte_tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.position = spr.matrix * vec4(in.position, 0.0, 1.0);
	var tex_coords = array(
		spr.tex_coords_tl,
		spr.tex_coords_tr,
		spr.tex_coords_bl,
		spr.tex_coords_br
	);
	out.tex_coords = tex_coords[in.tex_index];
	if spr.has_matte == 1 {
		var matte_tex_coords = array(
			spr.matte_tex_coords_tl,
			spr.matte_tex_coords_tr,
			spr.matte_tex_coords_bl,
			spr.matte_tex_coords_br
		);
		out.matte_tex_coords = matte_tex_coords[in.tex_index];
	}
	return out;
}


@group(0) @binding(0)
var Texture: texture_2d<f32>;
@group(0) @binding(1)
var MatteTexture: texture_2d<f32>;
@group(0) @binding(2)
var Sampler: sampler;

fn sample(tex: texture_2d<f32>, coords: vec2<f32>, is_ycbcr: u32) -> vec4<f32> {
	if is_ycbcr == 1 {
		var ya = textureSampleLevel(tex, Sampler, coords, 0.0).xy;
		var cbcr = textureSampleLevel(tex, Sampler, coords, 1.0).xy * CBCR_MULT - CBCR_SUB;
		var rgb = vec3(ya.x, cbcr) * YCbCrRgbMatrix;
		return vec4(rgb, ya.y);
	} else {
		var rgba = textureSample(tex, Sampler, coords);
		return rgba;
	}
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	if spr.has_matte == 0 {
		return sample(Texture, in.tex_coords, spr.is_ycbcr) * spr.color;
	} else {
		var base = sample(Texture, in.tex_coords, spr.is_ycbcr);
		var color = sample(MatteTexture, in.matte_tex_coords, spr.matte_is_ycbcr);
		color.w *= base.w;
		return color * spr.color;
	}
}
