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

struct InstanceInput {
	@location(2) matrix_x: vec4<f32>,
	@location(3) matrix_y: vec4<f32>,
	@location(4) matrix_z: vec4<f32>,
	@location(5) matrix_w: vec4<f32>,
	@location(6) tex_coord_tl: vec2<f32>,
	@location(7) tex_coord_tr: vec2<f32>,
	@location(8) tex_coord_bl: vec2<f32>,
	@location(9) tex_coord_br: vec2<f32>,
	@location(10) color: vec4<f32>,
	@location(11) texture_index: u32,
	@location(12) is_ycbcr: u32,
}

struct VertexOutput {
	@builtin(position) position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) color: vec4<f32>,
	@location(2) texture_index: u32,
	@location(3) is_ycbcr: u32,
}

@vertex
fn vs_main(in: VertexInput, instance: InstanceInput) -> VertexOutput {
	var out: VertexOutput;
	var matrix = mat4x4 (
		instance.matrix_x,
		instance.matrix_y,
		instance.matrix_z,
		instance.matrix_w
	);
	out.position = matrix * vec4(in.position, 0.0, 1.0);

	var tex_coords = array(
		instance.tex_coord_tl,
		instance.tex_coord_tr,
		instance.tex_coord_bl,
		instance.tex_coord_br
	);
	out.tex_coords = tex_coords[in.tex_index];
	out.color = instance.color;
	out.texture_index = instance.texture_index;
	out.is_ycbcr = instance.is_ycbcr;
	return out;
}


@group(0) @binding(0)
var Textures: binding_array<texture_2d<f32>, 256>;
@group(0) @binding(1)
var Sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	if in.is_ycbcr == 1 {
		var ya = textureSampleLevel(Textures[in.texture_index], Sampler, in.tex_coords, 0.0).xy;
		var cbcr = textureSampleLevel(Textures[in.texture_index], Sampler, in.tex_coords, 1.0).xy * CBCR_MULT - CBCR_SUB;
		var rgb = vec3(ya.x, cbcr) * YCbCrRgbMatrix;
		return vec4(rgb, ya.y) * in.color;
	} else {
		var rgba = textureSample(Textures[in.texture_index], Sampler, in.tex_coords);
		return rgba * in.color;
	}
}
