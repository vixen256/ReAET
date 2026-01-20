const Kb_601 = 0.114;
const Kr_601 = 0.299;
const Kg_601 = 1.0 - Kb_601 - Kr_601;

const YCbCrRgbMatrix_601 = mat3x3 (
	1.0, 0.0, 2.0 - 2.0 * Kr_601,
	1.0, -(Kb_601 / Kg_601) * (2.0 - 2.0 * Kb_601), -(Kr_601 / Kg_601) * (2.0 - 2.0 * Kr_601),
	1.0, 2.0 - 2.0 * Kb_601, 0.0,
);

const Kb_709 = 0.0722;
const Kr_709 = 0.2126;
const Kg_709 = 1.0 - Kb_709 - Kr_709;

const YCbCrRgbMatrix_709 = mat3x3 (
	1.0, 0.0, 2.0 - 2.0 * Kr_709,
	1.0, -(Kb_709 / Kg_709) * (2.0 - 2.0 * Kb_709), -(Kr_709 / Kg_709) * (2.0 - 2.0 * Kr_709),
	1.0, 2.0 - 2.0 * Kb_709, 0.0,
);

const Kb_2020 = 0.0593;
const Kr_2020 = 0.2627;
const Kg_2020 = 1.0 - Kb_2020 - Kr_2020;

const YCbCrRgbMatrix_2020 = mat3x3 (
	1.0, 0.0, 2.0 - 2.0 * Kr_2020,
	1.0, -(Kb_2020 / Kg_2020) * (2.0 - 2.0 * Kb_2020), -(Kr_2020 / Kg_2020) * (2.0 - 2.0 * Kr_2020),
	1.0, 2.0 - 2.0 * Kb_2020, 0.0,
);

const BT601: u32 = 0;
const BT709: u32 = 1;
const BT2020: u32 = 2;

struct video_info {
	color_primary: u32,
	full: u32,
	depth: f32,
}

@group(0) @binding(0)
var YTexture: texture_2d<u32>;
@group(0) @binding(1)
var CbTexture: texture_2d<u32>;
@group(0) @binding(2)
var CrTexture: texture_2d<u32>;
@group(0) @binding(3)
var<uniform> VideoInfo: video_info;

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
	var y = f32(textureLoad(YTexture, vec2<u32>(in.tex_coords * vec2<f32>(textureDimensions(YTexture))), 0).x) / VideoInfo.depth;
	var cb = f32(textureLoad(CbTexture, vec2<u32>(in.tex_coords * vec2<f32>(textureDimensions(CbTexture))), 0).x) / VideoInfo.depth;
	var cr = f32(textureLoad(CrTexture, vec2<u32>(in.tex_coords * vec2<f32>(textureDimensions(CrTexture))), 0).x) / VideoInfo.depth;
	if VideoInfo.full == 0 {
		y = y * (255.0 / 219.6) - (16.0 / 219.0);
		cb = cb * (255.0 / 224.0) - (128.0 / 224.0);
		cr = cr * (255.0 / 224.0) - (128.0 / 224.0);
	} else {
		cb = cb - (128.0 / 255.0);
		cr = cr - (128.0 / 255.0);
	}

	if VideoInfo.color_primary == BT601 {
			return vec4(vec3(y, cb, cr) * YCbCrRgbMatrix_601, 1.0);
	} else if VideoInfo.color_primary == BT709 {
			return vec4(vec3(y, cb, cr) * YCbCrRgbMatrix_709, 1.0);
	} else if VideoInfo.color_primary == BT2020 {
			return vec4(vec3(y, cb, cr) * YCbCrRgbMatrix_2020, 1.0);
	} else {
			return vec4(0.5, 0.5, 0.5, 1.0);
	}
}
