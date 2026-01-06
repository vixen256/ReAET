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
struct VideoInfo {
	matrix: mat4x4<f32>,
	color: vec4<f32>,
	has_matte: u32,
};

@group(0) @binding(0)
var Sampler: sampler;

@group(1) @binding(0)
var Texture: texture_2d<f32>;
@group(1) @binding(1)
var<uniform> TextureFormat: u32;

@group(2) @binding(0)
var MatteTexture: texture_2d<f32>;
@group(2) @binding(1)
var<uniform> MatteTextureFormat: u32;

@group(3) @binding(0)
var<uniform> video: VideoInfo;

struct VertexInput {
	@location(0) position: vec2<f32>,
	@location(1) tex_coords: vec2<f32>,
	@location(2) matte_tex_coords: vec2<f32>,
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
	out.tex_coords = in.tex_coords;
	out.matte_tex_coords = in.matte_tex_coords;
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
		return sample(Texture, in.tex_coords, TextureFormat) * video.color;
	} else {
		var base = sample(Texture, in.tex_coords, TextureFormat);
		var color = sample(MatteTexture, in.matte_tex_coords, MatteTextureFormat);
		//var color = vec4(in.matte_tex_coords, 0.0, 1.0);
		color.w *= base.w;
		return color * video.color;
	}
}
