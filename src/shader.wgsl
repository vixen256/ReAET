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

struct TextureInfo {
	coords_tl: vec2<f32>,
	coords_tr: vec2<f32>,
	coords_bl: vec2<f32>,
	coords_br: vec2<f32>,
	format: u32,
}

struct VideoInfo {
	matrix: mat4x4<f32>,
	color: vec4<f32>,
	has_matte: u32,
};

@group(0) @binding(0)
var Sampler: sampler;

@group(1) @binding(0)
var Texture: texture_2d<f32>;

@group(2) @binding(0)
var<uniform> tex_info: TextureInfo;

@group(3) @binding(0)
var MatteTexture: texture_2d<f32>;

@group(4) @binding(0)
var<uniform> matte_tex_info: TextureInfo;

@group(5) @binding(0)
var<uniform> video: VideoInfo;

struct VertexInput {
	@location(0) position: vec2<f32>,
	@location(1) tex_index: u32,
}

struct VertexOutput {
	@builtin(position) position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) matte_tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.position = video.matrix * vec4(in.position, 0.0, 1.0);
	var tex_coords = array(
		tex_info.coords_tl,
		tex_info.coords_tr,
		tex_info.coords_bl,
		tex_info.coords_br
	);
	out.tex_coords = tex_coords[in.tex_index];
	if video.has_matte == 1 {
		var matte_tex_coords = array(
			matte_tex_info.coords_tl,
			matte_tex_info.coords_tr,
			matte_tex_info.coords_bl,
			matte_tex_info.coords_br
		);
		out.matte_tex_coords = matte_tex_coords[in.tex_index];
	}
	return out;
}

fn sample(tex: texture_2d<f32>, coords: vec2<f32>, texture_format: u32) -> vec4<f32> {
	if texture_format == 0 {
		return textureSample(tex, Sampler, coords);
	} else if texture_format == 1 {
		var ya = textureSampleLevel(tex, Sampler, coords, 0.0).xy;
		var cbcr = textureSampleLevel(tex, Sampler, coords, 1.0).xy * CBCR_MULT - CBCR_SUB;
		var rgb = vec3(ya.x, cbcr) * YCbCrRgbMatrix;
		return vec4(rgb, ya.y);
	} else if texture_format == 2 {
		var y = textureSampleLevel(tex, Sampler, coords, 0.0).x;
		var cr = textureSampleLevel(tex, Sampler, coords, 1.0).x * CBCR_MULT - CBCR_SUB;
		var cb = textureSampleLevel(tex, Sampler, coords, 2.0).x * CBCR_MULT - CBCR_SUB;
		var rgb = vec3(y, cb, cr) * YCbCrRgbMatrix;
		return vec4(rgb, 1.0);
	} else {
		return vec4(1.0, 1.0, 1.0, 1.0);
	}
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	if video.has_matte == 0 {
		return sample(Texture, in.tex_coords, tex_info.format) * video.color;
	} else {
		var base = sample(Texture, in.tex_coords, tex_info.format);
		var color = sample(MatteTexture, in.matte_tex_coords, matte_tex_info.format);
		color.w *= base.w;
		return color * video.color;
	}
}
