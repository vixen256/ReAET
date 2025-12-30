use crate::app::TreeNode;
use crate::txp::*;
use eframe::egui;
use eframe::egui::Widget;
use eframe::egui_wgpu;
use eframe::egui_wgpu::wgpu;
use eframe::egui_wgpu::wgpu::util::DeviceExt;
use egui_material_icons::icons::*;
use kkdlib::*;
use regex::Regex;
use std::ops::*;
use std::rc::Rc;
use std::sync::*;
use transform_gizmo_egui::prelude::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec4 {
	pub x: f32,
	pub y: f32,
	pub z: f32,
	pub w: f32,
}

impl Add<Vec4> for Vec4 {
	type Output = Vec4;

	fn add(self, rhs: Vec4) -> Self::Output {
		Vec4 {
			x: self.x + rhs.x,
			y: self.y + rhs.y,
			z: self.z + rhs.z,
			w: self.w + rhs.w,
		}
	}
}

impl Mul<f32> for Vec4 {
	type Output = Vec4;

	fn mul(self, rhs: f32) -> Self::Output {
		Vec4 {
			x: self.x * rhs,
			y: self.y * rhs,
			z: self.z * rhs,
			w: self.w * rhs,
		}
	}
}

impl Mul<Vec4> for Vec4 {
	type Output = Vec4;

	fn mul(self, rhs: Vec4) -> Self::Output {
		Vec4 {
			x: self.x * rhs.x,
			y: self.y * rhs.y,
			z: self.z * rhs.z,
			w: self.w * rhs.w,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
	pub x: Vec4,
	pub y: Vec4,
	pub z: Vec4,
	pub w: Vec4,
}

impl Default for Mat4 {
	fn default() -> Self {
		Self {
			x: Vec4 {
				x: 1.0,
				..Default::default()
			},
			y: Vec4 {
				y: 1.0,
				..Default::default()
			},
			z: Vec4 {
				z: 1.0,
				..Default::default()
			},
			w: Vec4 {
				w: 1.0,
				..Default::default()
			},
		}
	}
}

impl Into<[[f32; 4]; 4]> for Mat4 {
	fn into(self) -> [[f32; 4]; 4] {
		[
			[self.x.x, self.x.y, self.x.z, self.x.w],
			[self.y.x, self.y.y, self.y.z, self.y.w],
			[self.z.x, self.z.y, self.z.z, self.z.w],
			[self.w.x, self.w.y, self.w.z, self.w.w],
		]
	}
}

impl Mul<Vec4> for Mat4 {
	type Output = Vec4;

	fn mul(self, rhs: Vec4) -> Vec4 {
		Vec4 {
			x: self.x.x * rhs.x + self.y.x * rhs.y + self.z.x * rhs.z + self.w.x * rhs.w,
			y: self.x.y * rhs.x + self.y.y * rhs.y + self.z.y * rhs.z + self.w.y * rhs.w,
			z: self.x.z * rhs.x + self.y.z * rhs.y + self.z.z * rhs.z + self.w.z * rhs.w,
			w: self.x.w * rhs.x + self.y.w * rhs.y + self.z.w * rhs.z + self.w.w * rhs.w,
		}
	}
}

impl Mul<Mat4> for Mat4 {
	type Output = Mat4;

	fn mul(self, rhs: Mat4) -> Mat4 {
		Mat4 {
			x: self * rhs.x,
			y: self * rhs.y,
			z: self * rhs.z,
			w: self * rhs.w,
		}
	}
}

#[derive(Clone, PartialEq)]
pub struct AetSetNode {
	pub name: String,
	pub modern: bool,
	pub big_endian: bool,
	pub is_x: bool,
	pub scenes: Vec<AetSceneNode>,
}

impl TreeNode for AetSetNode {
	fn label(&self) -> &str {
		&self.name
	}

	fn has_children(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		for scene in &mut self.scenes {
			f(scene);
		}
	}

	fn display_opts(&mut self, ui: &mut egui::Ui) {
		let height = ui.text_style_height(&egui::TextStyle::Body);
		egui_extras::TableBuilder::new(ui)
			.striped(true)
			.column(egui_extras::Column::remainder())
			.column(egui_extras::Column::remainder())
			.body(|mut body| {
				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Name");
					});
					row.col(|ui| {
						ui.text_edit_singleline(&mut self.name);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Modern");
					});
					row.col(|ui| {
						egui::Checkbox::without_text(&mut self.modern).ui(ui);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Big Endian");
					});
					row.col(|ui| {
						egui::Checkbox::without_text(&mut self.big_endian).ui(ui);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("X");
					});
					row.col(|ui| {
						egui::Checkbox::without_text(&mut self.is_x).ui(ui);
					});
				});
			});
	}

	fn raw_data(&self) -> Vec<u8> {
		let set = aet::Set {
			modern: self.modern,
			big_endian: self.big_endian,
			is_x: self.is_x,
			scenes: self
				.scenes
				.iter()
				.map(|scene| aet::Scene {
					name: scene.name.clone(),
					start_time: scene.start_time,
					end_time: scene.end_time,
					fps: scene.fps,
					color: scene.color,
					width: scene.width,
					height: scene.height,
					camera: scene.camera.clone(),
					root: scene.root.to_kkdlib(),
				})
				.collect(),
		};

		set.to_buf()
	}
}

impl AetSetNode {
	pub fn name_pattern() -> Regex {
		Regex::new(r"^aet_.*\.bin$").unwrap()
	}

	pub fn read(name: &str, data: &[u8]) -> Self {
		let set = aet::Set::from_buf(data, false);

		let scenes = set
			.scenes
			.into_iter()
			.map(|scene| AetSceneNode {
				name: scene.name,
				start_time: scene.start_time,
				end_time: scene.end_time,
				fps: scene.fps,
				color: scene.color,
				width: scene.width,
				height: scene.height,
				camera: scene.camera,
				root: AetCompNode::create(scene.root),

				current_time: scene.start_time,
				playing: false,
				display_placeholders: false,
				centered: false,

				selected_curve: None,
				gizmo: Gizmo::default(),
			})
			.collect();

		Self {
			name: name.to_string(),
			modern: set.modern,
			big_endian: set.big_endian,
			is_x: set.is_x,
			scenes,
		}
	}

	pub fn update_from(&mut self, other: &Self) {
		self.name = other.name.clone();
		self.modern = other.modern;
		self.big_endian = other.big_endian;
		self.is_x = other.is_x;

		if self.scenes.len() == other.scenes.len() {
			for (a, b) in self.scenes.iter_mut().zip(other.scenes.iter()) {
				a.update_from(b);
			}
		} else {
			self.scenes = other.scenes.clone();
		}
	}
}

#[derive(Clone)]
pub struct AetSceneNode {
	pub name: String,
	pub start_time: f32,
	pub end_time: f32,
	pub fps: f32,
	pub color: [u8; 3],
	pub width: u32,
	pub height: u32,
	pub camera: Option<aet::Camera>,
	pub root: AetCompNode,

	pub current_time: f32,
	pub playing: bool,
	pub display_placeholders: bool,
	pub centered: bool,

	pub selected_curve: Option<CurveType>,
	pub gizmo: Gizmo,
}

impl PartialEq for AetSceneNode {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
			&& self.start_time == other.start_time
			&& self.end_time == other.end_time
			&& self.fps == other.fps
			&& self.color == other.color
			&& self.width == other.width
			&& self.height == other.height
			&& self.camera == other.camera
			&& self.root == other.root
	}
}

impl TreeNode for AetSceneNode {
	fn label(&self) -> &str {
		&self.name
	}

	fn has_children(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		self.root.layers.retain_mut(|layer| {
			f(layer);
			!layer.want_deletion
		});
		for i in self
			.root
			.layers
			.iter()
			.enumerate()
			.filter(|(_, layer)| layer.want_duplicate)
			.map(|(i, _)| i)
			.collect::<Vec<_>>()
		{
			self.root.layers.push(self.root.layers[i].clone());
		}
		for layer in &mut self.root.layers {
			layer.want_duplicate = false;
		}
	}

	fn display_opts(&mut self, ui: &mut egui::Ui) {
		let height = ui.text_style_height(&egui::TextStyle::Body);
		egui_extras::TableBuilder::new(ui)
			.striped(true)
			.column(egui_extras::Column::remainder())
			.column(egui_extras::Column::remainder())
			.body(|mut body| {
				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Name");
					});
					row.col(|ui| {
						ui.text_edit_singleline(&mut self.name);
					});
				});
			});
	}

	fn has_context_menu(&self) -> bool {
		true
	}

	fn display_ctx_menu(&mut self, ui: &mut egui::Ui) {
		if ui.button("Hide all").clicked() {
			for layer in &mut self.root.layers {
				layer.visible = false;
			}
		}
	}
}

impl AetSceneNode {
	pub fn display_visual(&mut self, ui: &mut egui::Ui, rect: egui::Rect, selected: &[usize]) {
		let mut mat = Mat4::default();
		if self.centered {
			mat.w.x = self.width as f32 / 2.0;
			mat.w.y = self.height as f32 / 2.0;
		}
		let mut videos = WgpuAetVideos {
			videos: Vec::new(),
			viewport_size: [self.width as f32, self.height as f32],
		};

		self.root.display(
			mat,
			self.current_time,
			1.0,
			self.display_placeholders,
			&mut videos,
		);

		let w = rect.max.x - rect.min.x;
		let h = rect.max.y - rect.min.y;
		let ar = w / h;
		let rect = if ar > self.width as f32 / self.height as f32 {
			let adjusted_w = h / self.height as f32 * self.width as f32;
			let remaining_w = w - adjusted_w;
			egui::Rect {
				min: egui::Pos2 {
					x: rect.min.x + remaining_w / 2.0,
					y: rect.min.y,
				},
				max: egui::Pos2 {
					x: rect.min.x + adjusted_w + remaining_w / 2.0,
					y: rect.min.y + h,
				},
			}
		} else {
			let adjusted_h = w / self.width as f32 * self.height as f32;
			let remaining_h = h - adjusted_h;
			egui::Rect {
				min: egui::Pos2 {
					x: rect.min.x,
					y: rect.min.y + remaining_h / 2.0,
				},
				max: egui::Pos2 {
					x: rect.min.x + w,
					y: rect.min.y + adjusted_h + remaining_h / 2.0,
				},
			}
		};

		ui.painter()
			.add(egui_wgpu::Callback::new_paint_callback(rect, videos));

		if selected.len() >= 3 {
			let mut frame = self.current_time;
			let mut translation = [0.0; 3];
			let mut rotation = [0.0; 3];
			let mut scale = [1.0; 3];
			if self.centered {
				translation[0] = self.width as f64 / 2.0;
				translation[1] = self.height as f64 / 2.0;
			}

			let layer = &mut self.root.layers[selected[2]];
			if let Some(video) = &layer.video {
				translation[0] += scale[0] * video.pos_x.interpolate(frame) as f64;
				translation[1] += scale[1] * video.pos_y.interpolate(frame) as f64;
				if let Some(_3d) = &video._3d {
					translation[2] -= scale[2] * _3d.pos_z.interpolate(frame) as f64;
				}
				scale[0] *= video.scale_x.interpolate(frame) as f64;
				scale[1] *= video.scale_y.interpolate(frame) as f64;
				if let Some(_3d) = &video._3d {
					scale[2] *= _3d.scale_z.interpolate(frame) as f64;
				}
				translation[0] -= scale[0] * video.anchor_x.interpolate(frame) as f64;
				translation[1] -= scale[1] * video.anchor_y.interpolate(frame) as f64;
				if let Some(_3d) = &video._3d {
					translation[2] -= scale[2] * _3d.anchor_z.interpolate(frame) as f64;
				}

				if let Some(_3d) = &video._3d {
					rotation[0] += _3d.dir_x.interpolate(frame).to_radians() as f64;
					rotation[1] += _3d.dir_y.interpolate(frame).to_radians() as f64;
					rotation[2] += _3d.dir_z.interpolate(frame).to_radians() as f64;

					rotation[0] += _3d.rot_x.interpolate(frame).to_radians() as f64;
					rotation[1] += _3d.rot_y.interpolate(frame).to_radians() as f64;
				}
				rotation[2] += video.rot_z.interpolate(frame).to_radians() as f64;
			}

			let selected =
				selected
					.iter()
					.skip(3)
					.fold(&mut self.root.layers[selected[2]], |layer, i| {
						let AetItemNode::Comp(comp) = &mut layer.item else {
							panic!()
						};
						let layer = &mut comp.layers[*i];
						if let Some(video) = &layer.video {
							translation[0] += scale[0] * video.pos_x.interpolate(frame) as f64;
							translation[1] += scale[1] * video.pos_y.interpolate(frame) as f64;
							if let Some(_3d) = &video._3d {
								translation[2] -= scale[2] * _3d.pos_z.interpolate(frame) as f64;
							}
							scale[0] *= video.scale_x.interpolate(frame) as f64;
							scale[1] *= video.scale_y.interpolate(frame) as f64;
							if let Some(_3d) = &video._3d {
								scale[2] *= _3d.scale_z.interpolate(frame) as f64;
							}
							translation[0] -= scale[0] * video.anchor_x.interpolate(frame) as f64;
							translation[1] -= scale[1] * video.anchor_y.interpolate(frame) as f64;
							if let Some(_3d) = &video._3d {
								translation[2] -= scale[2] * _3d.anchor_z.interpolate(frame) as f64;
							}

							if let Some(_3d) = &video._3d {
								rotation[0] += _3d.dir_x.interpolate(frame).to_radians() as f64;
								rotation[1] += _3d.dir_y.interpolate(frame).to_radians() as f64;
								rotation[2] += _3d.dir_z.interpolate(frame).to_radians() as f64;

								rotation[0] += _3d.rot_x.interpolate(frame).to_radians() as f64;
								rotation[1] += _3d.rot_y.interpolate(frame).to_radians() as f64;
							}
							rotation[2] += video.rot_z.interpolate(frame).to_radians() as f64;
						}

						frame = (frame - layer.start_time) * layer.time_scale + layer.offset_time;
						layer
					});

			if let Some(video) = &mut selected.video {
				translation[0] += video.anchor_x.interpolate(frame) as f64;
				translation[1] += video.anchor_y.interpolate(frame) as f64;
				translation[1] = -translation[1] + self.height as f64;

				self.gizmo.update_config(GizmoConfig {
					projection_matrix: glam::DMat4::from_cols_array_2d(&[
						[2.0 / self.width as f64, 0.0, 0.0, 0.0],
						[0.0, 2.0 / self.height as f64, 0.0, 0.0],
						[0.0, 0.0, 1.0, 0.0],
						[-1.0, -1.0, 0.0, 1.0],
					])
					.into(),
					viewport: rect,
					modes: GizmoMode::TranslateX
						| GizmoMode::TranslateY
						| GizmoMode::TranslateXY
						| GizmoMode::RotateZ,
					snapping: true,
					snap_distance: 5.0,
					..Default::default()
				});

				let transform =
					transform_gizmo_egui::math::Transform::from_scale_rotation_translation(
						scale,
						glam::DQuat::from_euler(
							glam::EulerRot::XYZ,
							rotation[0],
							rotation[1],
							rotation[2],
						),
						translation,
					);

				if let Some((result, _)) = self.gizmo.interact(ui, &[transform]) {
					match result {
						GizmoResult::Translation { delta, total: _ } => {
							if video.pos_x.keys.is_empty() {
								video.pos_x.keys.push(aet::FCurveKey {
									frame: 0.0,
									value: 0.0,
									tangent: 0.0,
								});
							}
							for key in &mut video.pos_x.keys {
								key.value += delta.x as f32;
							}
							if video.pos_y.keys.is_empty() {
								video.pos_y.keys.push(aet::FCurveKey {
									frame: 0.0,
									value: 0.0,
									tangent: 0.0,
								});
							}
							for key in &mut video.pos_y.keys {
								key.value += -delta.y as f32;
							}
						}
						GizmoResult::Rotation {
							axis,
							delta,
							total: _,
							is_view_axis: _,
						} => {
							if axis.z == 1.0 {
								if video.rot_z.keys.is_empty() {
									video.rot_z.keys.push(aet::FCurveKey {
										frame: 0.0,
										value: 0.0,
										tangent: 0.0,
									});
								}

								for key in &mut video.rot_z.keys {
									key.value -= delta.to_degrees() as f32;
									if key.value.is_sign_negative() {
										key.value += 360.0;
									}
								}
							}
						}
						_ => {}
					}
				}
			}
		}
	}
}

impl AetSceneNode {
	pub fn update_from(&mut self, other: &Self) {
		self.name = other.name.clone();
		self.start_time = other.start_time;
		self.end_time = other.end_time;
		self.fps = other.fps;
		self.color = other.color;
		self.width = other.width;
		self.height = other.height;
		self.camera = other.camera.clone();

		if self.root.layers.len() == other.root.layers.len() {
			for (a, b) in self.root.layers.iter_mut().zip(other.root.layers.iter()) {
				a.update_from(b);
			}
		} else {
			self.root = other.root.clone();
		}
	}
}

#[derive(Clone, PartialEq)]
pub struct AetCompNode {
	pub layers: Vec<AetLayerNode>,
}

impl AetCompNode {
	fn create(comp: aet::Composition) -> Self {
		let layers = comp
			.layers
			.into_iter()
			.map(|layer| {
				let item = match layer.item {
					aet::Item::None => AetItemNode::None,
					aet::Item::Video(video) => AetItemNode::Video(AetVideoNode {
						color: video.color,
						width: video.width,
						height: video.height,
						fpf: video.fpf,
						sources: video
							.sources
							.into_iter()
							.map(|source| AetVideoSourceNode {
								name: source.name,
								id: source.id,
								sprite: None,
							})
							.collect(),
					}),
					aet::Item::Audio(audio) => AetItemNode::Audio(AetAudioNode {
						sound_index: audio.sound_index,
					}),
					aet::Item::Composition(comp) => AetItemNode::Comp(Self::create(comp)),
				};
				AetLayerNode {
					name: layer.name,
					start_time: layer.start_time,
					end_time: layer.end_time,
					offset_time: layer.offset_time,
					time_scale: layer.time_scale,
					flags: layer.flags,
					quality: layer.quality,
					item,
					markers: layer.markers,
					video: layer.video,
					audio: layer.audio,

					sprites: Rc::new(Mutex::new(Vec::new())),

					visible: layer.flags.video_active(),
					selected_key: 0,

					want_deletion: false,
					want_duplicate: false,
				}
			})
			.collect();
		Self { layers }
	}

	pub fn get_sprite_id(&self) -> Option<u32> {
		for layer in &self.layers {
			match &layer.item {
				AetItemNode::None => {}
				AetItemNode::Video(video) => return video.sources.first().map(|source| source.id),
				AetItemNode::Audio(_) => {}
				AetItemNode::Comp(comp) => {
					if let Some(sprite_id) = comp.get_sprite_id() {
						return Some(sprite_id);
					}
				}
			}
		}
		None
	}

	pub fn update_video_textures(
		&mut self,
		spr_db: &crate::spr_db::SprDbNode,
		spr_set: &crate::spr::SpriteSetNode,
	) {
		for layer in &mut self.layers {
			layer.sprites = spr_set.sprites_node.children.clone();
			match &mut layer.item {
				AetItemNode::None => {}
				AetItemNode::Video(video) => {
					for source in &mut video.sources {
						let mut index = None;
						for set in &spr_db.sets {
							let set = set.lock().unwrap();
							for entry in &set.entries {
								let entry = entry.lock().unwrap();
								if entry.id != source.id || entry.texture {
									continue;
								}
								index = Some(entry.index);
								break;
							}
							if index.is_some() {
								break;
							}
						}
						let Some(index) = index else {
							continue;
						};
						let sprs = spr_set.sprites_node.children.lock().unwrap();
						let Some(sprite) = sprs.iter().skip(index as usize).next() else {
							continue;
						};

						source.sprite = Some(sprite.clone());
					}
				}
				AetItemNode::Audio(_) => {}
				AetItemNode::Comp(comp) => comp.update_video_textures(spr_db, spr_set),
			}
		}
	}

	fn display(
		&self,
		mat: Mat4,
		frame: f32,
		opacity: f32,
		display_placeholders: bool,
		videos: &mut WgpuAetVideos,
	) {
		for layer in self.layers.iter().rev() {
			if frame < layer.start_time
				|| frame >= layer.end_time
				|| !layer.flags.video_active()
				|| !layer.visible
			{
				continue;
			}

			let mut m = mat;
			let mut pos = [0.0; 3];
			let mut scale = [1.0; 3];
			let mut dir = [0.0; 3];
			let mut rot = [0.0; 3];
			let mut anchor = [0.0; 3];
			let mut opacity = opacity;

			if let Some(video) = &layer.video {
				pos[0] = video.pos_x.interpolate(frame);
				pos[1] = video.pos_y.interpolate(frame);
				rot[2] = video.rot_z.interpolate(frame);
				scale[0] = video.scale_x.interpolate(frame);
				scale[1] = video.scale_y.interpolate(frame);
				anchor[0] = video.anchor_x.interpolate(frame);
				anchor[1] = video.anchor_y.interpolate(frame);
				opacity *= video.opacity.interpolate(frame).clamp(0.0, 1.0);

				if let Some(_3d) = &video._3d {
					pos[2] = -_3d.pos_z.interpolate(frame);
					dir[0] = _3d.dir_x.interpolate(frame);
					dir[1] = _3d.dir_y.interpolate(frame);
					dir[2] = _3d.dir_z.interpolate(frame);
					rot[0] = _3d.rot_x.interpolate(frame);
					rot[1] = _3d.rot_y.interpolate(frame);
					scale[2] = _3d.scale_z.interpolate(frame);
					anchor[2] = _3d.anchor_z.interpolate(frame);
				}
			}

			m.w = m.x * pos[0] + m.y * pos[1] + m.z * -pos[2] + m.w;
			if dir[0] > 0.0 {
				let rad = -dir[0].to_radians();
				let y = m.y;
				let z = m.z;
				m.y = y * rad.cos() + z * rad.sin();
				m.z = y * -rad.sin() + z * rad.cos();
			}
			if dir[1] > 0.0 {
				let rad = -dir[1].to_radians();
				let x = m.x;
				let z = m.z;
				m.x = x * rad.cos() + z * -rad.sin();
				m.z = x * rad.sin() + z * rad.cos();
			}
			if dir[2] > 0.0 {
				let rad = dir[2].to_radians();
				let x = m.x;
				let y = m.y;
				m.x = x * rad.cos() + y * rad.sin();
				m.y = x * -rad.sin() + y * rad.cos();
			}

			if rot[0] > 0.0 {
				let rad = -rot[0].to_radians();
				let y = m.y;
				let z = m.z;
				m.y = y * rad.cos() + z * rad.sin();
				m.z = y * -rad.sin() + z * rad.cos();
			}
			if rot[1] > 0.0 {
				let rad = -rot[1].to_radians();
				let x = m.x;
				let z = m.z;
				m.x = x * rad.cos() + z * -rad.sin();
				m.z = x * rad.sin() + z * rad.cos();
			}
			if rot[2] > 0.0 {
				let rad = rot[2].to_radians();
				let x = m.x;
				let y = m.y;
				m.x = x * rad.cos() + y * rad.sin();
				m.y = x * -rad.sin() + y * rad.cos();
			}

			m.x = m.x * scale[0];
			m.y = m.y * scale[1];
			m.z = m.z * scale[2];
			m.w = m.x * -anchor[0] + m.y * -anchor[1] + m.z * -anchor[2] + m.w;

			match &layer.item {
				AetItemNode::None => {}
				AetItemNode::Video(video) => {
					let Some(source) = video.sources.first() else {
						if display_placeholders {
							videos.videos.push(WgpuAetVideo {
								is_ycbcr: false,
								texture_coords: [0.0, 0.0, 0.0, 0.0],
								source_size: [video.width as f32, video.height as f32],
								texture_index: 255,
								mat: m,
								color: [
									video.color[0] as f32 / 255.0,
									video.color[1] as f32 / 255.0,
									video.color[2] as f32 / 255.0,
									opacity,
								],
							});
						}
						continue;
					};
					let Some(sprite) = &source.sprite else {
						continue;
					};

					let sprite = sprite.lock().unwrap();
					let texture = sprite.texture.lock().unwrap();
					let mip = texture.texture.get_mipmap(0, 0).unwrap();
					let x = sprite.info.px() / mip.width() as f32;
					let y = (mip.height() as f32 - sprite.info.py() - sprite.info.height())
						/ mip.height() as f32;
					let w = (sprite.info.px() + sprite.info.width()) / mip.width() as f32;
					let h = (mip.height() as f32 - sprite.info.py()) / mip.height() as f32;

					let video = WgpuAetVideo {
						is_ycbcr: texture.texture.is_ycbcr(),
						texture_coords: [x, y, w, h],
						source_size: [video.width as f32, video.height as f32],
						texture_index: sprite.info.texid() as usize,
						mat: m,
						color: [1.0, 1.0, 1.0, opacity],
					};

					videos.videos.push(video);
				}
				AetItemNode::Audio(_) => {}
				AetItemNode::Comp(comp) => comp.display(
					m,
					(frame - layer.start_time) * layer.time_scale + layer.offset_time,
					opacity,
					display_placeholders,
					videos,
				),
			}
		}
	}

	pub fn show_node_curve_editor(
		&mut self,
		ui: &mut egui::Ui,
		selected_curve: &mut Option<CurveType>,
		frame: f32,
		index: usize,
		depth: usize,
		path: &[usize],
		desired_path: &[usize],
	) {
		if desired_path.len() <= depth + 1 {
			return;
		}
		let desired_index = desired_path[depth + 1];
		let Some(layer) = self.layers.get_mut(desired_index) else {
			return;
		};
		let mut path = path.to_vec();
		path.push(index);

		if depth + 1 == desired_path.len() - 1 {
			layer.display_curve_editor(ui, selected_curve, frame);
		} else if let AetItemNode::Comp(comp) = &mut layer.item {
			comp.show_node_curve_editor(
				ui,
				selected_curve,
				(frame - layer.start_time) * layer.time_scale + layer.offset_time,
				index,
				depth + 1,
				&path,
				desired_path,
			);
		}
	}

	fn to_kkdlib(&self) -> aet::Composition {
		let layers = self
			.layers
			.iter()
			.map(|layer| {
				let item = match &layer.item {
					AetItemNode::None => aet::Item::None,
					AetItemNode::Video(video) => aet::Item::Video(aet::Video {
						color: video.color,
						width: video.width,
						height: video.height,
						fpf: video.fpf,
						sources: video
							.sources
							.iter()
							.map(|source| {
								let (name, id) = if let Some(sprite) = &source.sprite {
									if let Some(db_entry) = &sprite.lock().unwrap().db_entry {
										let db_entry = db_entry.lock().unwrap();
										(db_entry.name.clone(), db_entry.id)
									} else {
										(source.name.clone(), source.id)
									}
								} else {
									(source.name.clone(), source.id)
								};
								aet::VideoSource { name, id }
							})
							.collect(),
					}),
					AetItemNode::Audio(audio) => aet::Item::Audio(aet::Audio {
						sound_index: audio.sound_index,
					}),
					AetItemNode::Comp(comp) => aet::Item::Composition(comp.to_kkdlib()),
				};
				aet::Layer {
					name: layer.name.clone(),
					start_time: layer.start_time,
					end_time: layer.end_time,
					offset_time: layer.offset_time,
					time_scale: layer.time_scale,
					flags: layer.flags,
					quality: layer.quality,
					item,
					markers: layer.markers.clone(),
					video: layer.video.clone(),
					audio: layer.audio.clone(),
				}
			})
			.collect();

		aet::Composition { layers }
	}
}

#[derive(Clone, Copy, PartialEq)]
pub enum CurveType {
	// Audio
	VolumeL,
	VolumeR,
	PanL,
	PanR,
	// Video
	AnchorX,
	AnchorY,
	PosX,
	PosY,
	RotZ,
	ScaleX,
	ScaleY,
	Opacity,
	// 3D
	AnchorZ,
	PosZ,
	DirX,
	DirY,
	DirZ,
	RotX,
	RotY,
	ScaleZ,
}

#[derive(Clone)]
pub struct AetLayerNode {
	pub name: String,
	pub start_time: f32,
	pub end_time: f32,
	pub offset_time: f32,
	pub time_scale: f32,
	pub flags: aet::LayerFlags,
	pub quality: aet::LayerQuality,
	pub item: AetItemNode,
	pub markers: Vec<(String, f32)>,
	pub video: Option<aet::LayerVideo>,
	pub audio: Option<aet::LayerAudio>,

	pub sprites: Rc<Mutex<Vec<Rc<Mutex<crate::spr::SpriteInfoNode>>>>>,

	pub visible: bool,
	pub selected_key: usize,

	pub want_deletion: bool,
	pub want_duplicate: bool,
}

impl PartialEq for AetLayerNode {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
			&& self.start_time == other.start_time
			&& self.end_time == other.end_time
			&& self.offset_time == other.offset_time
			&& self.time_scale == other.time_scale
			&& self.flags == other.flags
			&& self.quality == other.quality
			&& self.item == other.item
			&& self.markers == other.markers
			&& self.video == other.video
			&& self.audio == other.audio
	}
}

impl TreeNode for AetLayerNode {
	fn label(&self) -> &str {
		&self.name
	}

	fn label_sameline(&mut self, ui: &mut egui::Ui) {
		let icon = if self.visible {
			ICON_VISIBILITY
		} else {
			ICON_VISIBILITY_OFF
		};
		if ui.button(icon).clicked() {
			self.visible = !self.visible;
		}
	}

	fn has_children(&self) -> bool {
		match &self.item {
			AetItemNode::Comp(comp) => !comp.layers.is_empty(),
			_ => false,
		}
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		match &mut self.item {
			AetItemNode::Comp(comp) => {
				comp.layers.retain_mut(|layer| {
					f(layer);
					!layer.want_deletion
				});
				for i in comp
					.layers
					.iter()
					.enumerate()
					.filter(|(_, layer)| layer.want_duplicate)
					.map(|(i, _)| i)
					.collect::<Vec<_>>()
				{
					comp.layers.push(comp.layers[i].clone());
				}
				for layer in &mut comp.layers {
					layer.want_duplicate = false;
				}
			}
			_ => {}
		}
	}

	fn display_opts(&mut self, ui: &mut egui::Ui) {
		let height = ui.text_style_height(&egui::TextStyle::Body);
		egui_extras::TableBuilder::new(ui)
			.striped(true)
			.column(egui_extras::Column::remainder())
			.column(egui_extras::Column::remainder())
			.body(|mut body| {
				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Name");
					});
					row.col(|ui| {
						ui.text_edit_singleline(&mut self.name);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Start time");
					});
					row.col(|ui| {
						egui::DragValue::new(&mut self.start_time)
							.max_decimals(0)
							.speed(0.0)
							.update_while_editing(true)
							.ui(ui);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("End time");
					});
					row.col(|ui| {
						egui::DragValue::new(&mut self.end_time)
							.max_decimals(0)
							.speed(0.0)
							.update_while_editing(true)
							.ui(ui);
					});
				});

				let mut has_audio = self.audio.is_some();
				let mut has_video = self.video.is_some();
				let mut has_3d = self
					.video
					.as_ref()
					.map_or(false, |video| video._3d.is_some());

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Audio");
					});
					row.col(|ui| {
						if egui::Checkbox::without_text(&mut has_audio)
							.ui(ui)
							.changed()
						{
							if self.audio.is_none() {
								self.audio = Some(aet::LayerAudio {
									volume_l: aet::FCurve {
										keys: vec![aet::FCurveKey {
											frame: 0.0,
											value: 1.0,
											tangent: 0.0,
										}],
									},
									volume_r: aet::FCurve {
										keys: vec![aet::FCurveKey {
											frame: 0.0,
											value: 1.0,
											tangent: 0.0,
										}],
									},
									pan_l: aet::FCurve { keys: Vec::new() },
									pan_r: aet::FCurve { keys: Vec::new() },
								});
							} else {
								self.audio = None;
							}
						}
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Video");
					});
					row.col(|ui| {
						if egui::Checkbox::without_text(&mut has_video)
							.ui(ui)
							.changed()
						{
							if self.video.is_none() {
								self.video = Some(aet::LayerVideo {
									transfer_mode: aet::TransferMode {
										mode: aet::BlendMode::Add,
										flag: 0,
										matte: 0,
									},
									anchor_x: aet::FCurve { keys: Vec::new() },
									anchor_y: aet::FCurve { keys: Vec::new() },
									pos_x: aet::FCurve { keys: Vec::new() },
									pos_y: aet::FCurve { keys: Vec::new() },
									rot_z: aet::FCurve { keys: Vec::new() },
									scale_x: aet::FCurve {
										keys: vec![aet::FCurveKey {
											frame: 0.0,
											value: 1.0,
											tangent: 0.0,
										}],
									},
									scale_y: aet::FCurve {
										keys: vec![aet::FCurveKey {
											frame: 0.0,
											value: 1.0,
											tangent: 0.0,
										}],
									},
									opacity: aet::FCurve {
										keys: vec![aet::FCurveKey {
											frame: 0.0,
											value: 1.0,
											tangent: 0.0,
										}],
									},
									_3d: None,
								});
							} else {
								self.video = None;
							}
						}
					});
				});

				if let Some(video) = &mut self.video {
					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("3D");
						});
						row.col(|ui| {
							if egui::Checkbox::without_text(&mut has_3d).ui(ui).changed() {
								if video._3d.is_none() {
									video._3d = Some(aet::LayerVideo3D {
										anchor_z: aet::FCurve { keys: Vec::new() },
										pos_z: aet::FCurve { keys: Vec::new() },
										dir_x: aet::FCurve { keys: Vec::new() },
										dir_y: aet::FCurve { keys: Vec::new() },
										dir_z: aet::FCurve { keys: Vec::new() },
										rot_x: aet::FCurve { keys: Vec::new() },
										rot_y: aet::FCurve { keys: Vec::new() },
										scale_z: aet::FCurve {
											keys: vec![aet::FCurveKey {
												frame: 0.0,
												value: 1.0,
												tangent: 0.0,
											}],
										},
									});
								} else {
									video._3d = None;
								}
							}
						});
					});
				}

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Child");
					});
					row.col(|ui| {
						let (item_type, label) = match &self.item {
							AetItemNode::None => (0, "None"),
							AetItemNode::Video(_) => (1, "Video"),
							AetItemNode::Audio(_) => (2, "Audio"),
							AetItemNode::Comp(_) => (3, "Composition"),
						};
						let mut new_item_type = item_type;
						egui::ComboBox::from_id_salt("ChildComboBox")
							.selected_text(label)
							.show_ui(ui, |ui| {
								ui.selectable_value(&mut new_item_type, 0, "None");
								ui.selectable_value(&mut new_item_type, 1, "Video");
								ui.selectable_value(&mut new_item_type, 2, "Audio");
								ui.selectable_value(&mut new_item_type, 3, "Comp");
							});

						if new_item_type != item_type {
							match new_item_type {
								0 => self.item = AetItemNode::None,
								1 => {
									self.item = AetItemNode::Video(AetVideoNode {
										color: [255, 255, 255],
										width: 0,
										height: 0,
										fpf: 0.0,
										sources: Vec::new(),
									})
								}
								2 => {
									self.item = AetItemNode::Audio(AetAudioNode { sound_index: 0 })
								}
								3 => {
									self.item =
										AetItemNode::Comp(AetCompNode { layers: Vec::new() })
								}
								_ => unreachable!(),
							}
						}
					});
				});

				match &mut self.item {
					AetItemNode::None => {}
					AetItemNode::Video(video) => {
						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Width");
							});
							row.col(|ui| {
								egui::DragValue::new(&mut video.width)
									.max_decimals(0)
									.speed(0.0)
									.update_while_editing(true)
									.ui(ui);
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Height");
							});
							row.col(|ui| {
								egui::DragValue::new(&mut video.height)
									.max_decimals(0)
									.speed(0.0)
									.update_while_editing(true)
									.ui(ui);
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("FPF");
							});
							row.col(|ui| {
								egui::DragValue::new(&mut video.fpf)
									.max_decimals(0)
									.speed(0.0)
									.update_while_editing(true)
									.ui(ui);
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Color");
							});
							row.col(|ui| {
								let mut rgb = [
									video.color[0] as f32 / 255.0,
									video.color[1] as f32 / 255.0,
									video.color[2] as f32 / 255.0,
								];
								ui.color_edit_button_rgb(&mut rgb);
								video.color[0] = (rgb[0] * 255.0) as u8;
								video.color[1] = (rgb[1] * 255.0) as u8;
								video.color[2] = (rgb[2] * 255.0) as u8;
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Sources");
							});
							row.col(|ui| {
								if ui.button(ICON_ADD).clicked() {
									video.sources.push(AetVideoSourceNode {
										name: String::new(),
										id: 0,
										sprite: self.sprites.lock().unwrap().first().cloned(),
									});
								}
							});
						});

						for (i, source) in video.sources.iter_mut().enumerate() {
							let Some(sprite) = &source.sprite else {
								continue;
							};
							let sprite = sprite.lock().unwrap();
							let Some(db_entry) = &sprite.db_entry else {
								continue;
							};
							let db_entry = db_entry.lock().unwrap();
							let sprite_name = sprite.name.clone();
							let old_selected_sprite = db_entry.id;
							let mut selected_sprite = db_entry.id;
							drop(db_entry);
							drop(sprite);

							body.row(height, |mut row| {
								row.col(|_| {});
								row.col(|ui| {
									egui::ComboBox::from_id_salt(format!("Source{i}ComboBox"))
										.selected_text(&sprite_name)
										.show_ui(ui, |ui| {
											for sprite in self.sprites.lock().unwrap().iter() {
												let sprite = sprite.lock().unwrap();
												let Some(db_entry) = &sprite.db_entry else {
													continue;
												};
												let db_entry = db_entry.lock().unwrap();
												ui.selectable_value(
													&mut selected_sprite,
													db_entry.id,
													&sprite.name,
												);
											}
										});
								});
							});

							if selected_sprite != old_selected_sprite {
								source.sprite = self
									.sprites
									.lock()
									.unwrap()
									.iter()
									.find(|spr| {
										spr.lock().unwrap().db_entry.is_some()
											&& spr
												.lock()
												.unwrap()
												.db_entry
												.as_ref()
												.unwrap()
												.lock()
												.unwrap()
												.id == selected_sprite
									})
									.cloned();
							}
						}
					}
					AetItemNode::Audio(audio) => {
						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Sound index");
							});
							row.col(|ui| {
								egui::DragValue::new(&mut audio.sound_index)
									.max_decimals(0)
									.speed(0.0)
									.update_while_editing(true)
									.ui(ui);
							});
						});
					}
					AetItemNode::Comp(_) => {}
				}

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Markers");
					});
					row.col(|ui| {
						if ui.button(ICON_ADD).clicked() {
							self.markers.push((String::from("DUMMY"), 0.0));
						}
					});
				});

				for (name, value) in &mut self.markers {
					body.row(height, |mut row| {
						row.col(|ui| {
							ui.text_edit_singleline(name);
						});
						row.col(|ui| {
							egui::DragValue::new(value)
								.max_decimals(2)
								.speed(0.0)
								.update_while_editing(true)
								.ui(ui);
						});
					});
				}
			});
	}

	fn has_context_menu(&self) -> bool {
		true
	}

	fn display_ctx_menu(&mut self, ui: &mut egui::Ui) {
		if let AetItemNode::Comp(comp) = &mut self.item {
			if ui.button("Add").clicked() {
				comp.layers.push(AetLayerNode {
					name: String::from("DUMMY"),
					start_time: 0.0,
					end_time: self.end_time - self.start_time,
					offset_time: 0.0,
					time_scale: 1.0,
					flags: self.flags,
					quality: aet::LayerQuality::Best,
					item: AetItemNode::None,
					markers: Vec::new(),
					video: None,
					audio: None,
					sprites: self.sprites.clone(),
					visible: self.visible,
					selected_key: 0,
					want_deletion: false,
					want_duplicate: false,
				})
			}
		};

		if ui.button("Duplicate").clicked() {
			self.want_duplicate = true;
		}

		if ui.button("Remove").clicked() {
			self.want_deletion = true;
		}
	}
}

impl AetLayerNode {
	pub fn display_curve_editor(
		&mut self,
		ui: &mut egui::Ui,
		selected_curve: &mut Option<CurveType>,
		frame: f32,
	) {
		egui::SidePanel::left("CurveSelector")
			.resizable(true)
			.show_inside(ui, |ui| {
				egui::ScrollArea::vertical().show(ui, |ui| {
					if self.audio.is_some() {
						if ui
							.selectable_label(
								*selected_curve == Some(CurveType::VolumeL),
								"Volume L",
							)
							.clicked()
						{
							*selected_curve = Some(CurveType::VolumeL);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(
								*selected_curve == Some(CurveType::VolumeR),
								"Volume R",
							)
							.clicked()
						{
							*selected_curve = Some(CurveType::VolumeR);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(*selected_curve == Some(CurveType::PanL), "Pan L")
							.clicked()
						{
							*selected_curve = Some(CurveType::PanL);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(*selected_curve == Some(CurveType::PanR), "Pan R")
							.clicked()
						{
							*selected_curve = Some(CurveType::PanR);
							self.selected_key = 0;
						}
					}

					if self.video.is_some() {
						let has_3d = self.video.as_ref().unwrap()._3d.is_some();
						if ui
							.selectable_label(
								*selected_curve == Some(CurveType::AnchorX),
								"Anchor X",
							)
							.clicked()
						{
							*selected_curve = Some(CurveType::AnchorX);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(
								*selected_curve == Some(CurveType::AnchorY),
								"Anchor Y",
							)
							.clicked()
						{
							*selected_curve = Some(CurveType::AnchorY);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(
									*selected_curve == Some(CurveType::AnchorZ),
									"Anchor Z",
								)
								.clicked()
						{
							*selected_curve = Some(CurveType::AnchorZ);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(*selected_curve == Some(CurveType::PosX), "Pos X")
							.clicked()
						{
							*selected_curve = Some(CurveType::PosX);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(*selected_curve == Some(CurveType::PosY), "Pos Y")
							.clicked()
						{
							*selected_curve = Some(CurveType::PosY);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(*selected_curve == Some(CurveType::PosZ), "Pos Z")
								.clicked()
						{
							*selected_curve = Some(CurveType::PosZ);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(*selected_curve == Some(CurveType::DirX), "Dir X")
								.clicked()
						{
							*selected_curve = Some(CurveType::DirX);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(*selected_curve == Some(CurveType::DirY), "Dir Y")
								.clicked()
						{
							*selected_curve = Some(CurveType::DirY);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(*selected_curve == Some(CurveType::DirZ), "Dir Z")
								.clicked()
						{
							*selected_curve = Some(CurveType::DirZ);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(*selected_curve == Some(CurveType::RotX), "Rot X")
								.clicked()
						{
							*selected_curve = Some(CurveType::RotX);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(*selected_curve == Some(CurveType::RotY), "Rot Y")
								.clicked()
						{
							*selected_curve = Some(CurveType::RotY);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(*selected_curve == Some(CurveType::RotZ), "Rot Z")
							.clicked()
						{
							*selected_curve = Some(CurveType::RotZ);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(*selected_curve == Some(CurveType::ScaleX), "Scale X")
							.clicked()
						{
							*selected_curve = Some(CurveType::ScaleX);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(*selected_curve == Some(CurveType::ScaleY), "Scale Y")
							.clicked()
						{
							*selected_curve = Some(CurveType::ScaleY);
							self.selected_key = 0;
						}
						if has_3d
							&& ui
								.selectable_label(
									*selected_curve == Some(CurveType::ScaleZ),
									"Scale Z",
								)
								.clicked()
						{
							*selected_curve = Some(CurveType::ScaleZ);
							self.selected_key = 0;
						}
						if ui
							.selectable_label(
								*selected_curve == Some(CurveType::Opacity),
								"Opacity",
							)
							.clicked()
						{
							*selected_curve = Some(CurveType::Opacity);
							self.selected_key = 0;
						}
					}

					ui.take_available_space();
				});
			});

		let Some(selected_curve) = &selected_curve else {
			return;
		};

		let curve = match selected_curve {
			CurveType::VolumeL => self.audio.as_mut().map(|audio| &mut audio.volume_l),
			CurveType::VolumeR => self.audio.as_mut().map(|audio| &mut audio.volume_r),
			CurveType::PanL => self.audio.as_mut().map(|audio| &mut audio.pan_l),
			CurveType::PanR => self.audio.as_mut().map(|audio| &mut audio.pan_r),

			CurveType::AnchorX => self.video.as_mut().map(|video| &mut video.anchor_x),
			CurveType::AnchorY => self.video.as_mut().map(|video| &mut video.anchor_y),
			CurveType::PosX => self.video.as_mut().map(|video| &mut video.pos_x),
			CurveType::PosY => self.video.as_mut().map(|video| &mut video.pos_y),
			CurveType::RotZ => self.video.as_mut().map(|video| &mut video.rot_z),
			CurveType::ScaleX => self.video.as_mut().map(|video| &mut video.scale_x),
			CurveType::ScaleY => self.video.as_mut().map(|video| &mut video.scale_y),
			CurveType::Opacity => self.video.as_mut().map(|video| &mut video.opacity),

			CurveType::AnchorZ => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.anchor_z))
				.flatten(),
			CurveType::PosZ => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.pos_z))
				.flatten(),
			CurveType::DirX => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_x))
				.flatten(),
			CurveType::DirY => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_y))
				.flatten(),
			CurveType::DirZ => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_z))
				.flatten(),
			CurveType::RotX => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.rot_x))
				.flatten(),
			CurveType::RotY => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.rot_y))
				.flatten(),
			CurveType::ScaleZ => self
				.video
				.as_mut()
				.map(|video| video._3d.as_mut().map(|_3d| &mut _3d.scale_z))
				.flatten(),
		};

		let Some(curve) = curve else {
			return;
		};

		if curve.keys.is_empty() {
			curve.keys.push(aet::FCurveKey {
				frame: 0.0,
				value: 0.0,
				tangent: 0.0,
			});
		}

		if self.selected_key >= curve.keys.len() {
			self.selected_key = curve.keys.len() - 1;
		}

		egui::SidePanel::right("KeyEditor")
			.resizable(true)
			.show_inside(ui, |ui| {
				ui.horizontal(|ui| {
					ui.label(format!("{}/{}", self.selected_key + 1, curve.keys.len()));
					if ui
						.add_enabled(self.selected_key != 0, egui::Button::new(ICON_ARROW_LEFT))
						.clicked()
					{
						self.selected_key -= 1;
					}

					if ui
						.add_enabled(
							self.selected_key != curve.keys.len() - 1,
							egui::Button::new(ICON_ARROW_RIGHT),
						)
						.clicked()
					{
						self.selected_key += 1;
					}

					if ui.button(ICON_ADD).clicked() {
						let f = frame.clamp(self.start_time, self.end_time);
						curve.keys.push(aet::FCurveKey {
							frame: f,
							value: curve.interpolate(f),
							tangent: 0.0,
						});
						curve.keys.sort_by(|a, b| a.frame.total_cmp(&b.frame));
						self.selected_key = curve
							.keys
							.iter()
							.position(|key| key.frame == f)
							.unwrap_or(0);
					}

					if ui
						.add_enabled(curve.keys.len() != 1, egui::Button::new(ICON_REMOVE))
						.clicked()
					{
						curve.keys.remove(self.selected_key);
						if self.selected_key == curve.keys.len() {
							self.selected_key -= 1;
						}
					}
				});

				ui.horizontal(|ui| {
					ui.label("Frame");
					if egui::DragValue::new(&mut curve.keys[self.selected_key].frame)
						.max_decimals(2)
						.speed(0.0)
						.update_while_editing(true)
						.ui(ui)
						.changed()
					{
						curve.keys[self.selected_key].frame = curve.keys[self.selected_key]
							.frame
							.clamp(self.start_time, self.end_time);

						curve.keys.sort_by(|a, b| a.frame.total_cmp(&b.frame));
					}
				});

				ui.horizontal(|ui| {
					ui.label("Value");
					egui::DragValue::new(&mut curve.keys[self.selected_key].value)
						.max_decimals(2)
						.speed(0.0)
						.update_while_editing(true)
						.ui(ui);
				});

				ui.horizontal(|ui| {
					ui.label("Tangent");
					egui::DragValue::new(&mut curve.keys[self.selected_key].tangent)
						.max_decimals(2)
						.speed(0.0)
						.update_while_editing(true)
						.ui(ui);
				});

				ui.take_available_space();
			});

		if curve.keys.len() <= 1 {
			return;
		}

		let ids = (0..curve.keys.len())
			.map(|i| egui::Id::new(format!("Key {}", i + 1)))
			.collect::<Vec<_>>();

		let resp = egui_plot::Plot::new("CurveViewer")
			.allow_drag(false)
			.show(ui, |plot| {
				plot.line(
					egui_plot::Line::new(
						"Curve",
						egui_plot::PlotPoints::from_explicit_callback(
							|x| curve.interpolate(x as f32) as f64,
							(self.start_time as f64)..(self.end_time as f64 + 1.0),
							1000,
						),
					)
					.color(egui::Color32::from_rgb(0xD0, 0x50, 0x60))
					.allow_hover(false),
				);

				if frame >= self.start_time && frame <= self.end_time {
					plot.vline(egui_plot::VLine::new("CurrentTime", frame).allow_hover(false));
				}

				for (name, value) in &self.markers {
					plot.vline(egui_plot::VLine::new(name, *value));
				}

				for (i, key) in curve.keys.iter().enumerate() {
					plot.points(
						egui_plot::Points::new(
							format!("Key {}", i + 1),
							vec![[key.frame as f64, key.value as f64]],
						)
						.id(ids[i])
						.color(egui::Color32::from_rgba_unmultiplied(
							0x50, 0x60, 0xD0, 0xA0,
						))
						.radius(5.0),
					);
				}
			});

		if resp.response.clicked()
			&& let Some(hovered) = resp.hovered_plot_item
			&& let Some(index) = ids.iter().position(|id| *id == hovered)
		{
			self.selected_key = index;
		}
	}

	pub fn update_from(&mut self, other: &Self) {
		self.name = other.name.clone();
		self.start_time = other.start_time;
		self.end_time = other.end_time;
		self.offset_time = other.offset_time;
		self.time_scale = other.time_scale;
		self.flags = other.flags;
		self.quality = other.quality;
		self.markers = other.markers.clone();
		self.video = other.video.clone();
		self.audio = other.audio.clone();
		self.audio = other.audio.clone();

		if let AetItemNode::Comp(a) = &mut self.item
			&& let AetItemNode::Comp(b) = &other.item
			&& a.layers.len() == b.layers.len()
		{
			for (a, b) in a.layers.iter_mut().zip(b.layers.iter()) {
				a.update_from(b);
			}
		} else {
			self.item = other.item.clone();
		}
	}
}

#[derive(Clone, PartialEq)]
pub enum AetItemNode {
	None,
	Video(AetVideoNode),
	Audio(AetAudioNode),
	Comp(AetCompNode),
}

#[derive(Clone, PartialEq)]
pub struct AetVideoNode {
	pub color: [u8; 3],
	pub width: u16,
	pub height: u16,
	pub fpf: f32,
	pub sources: Vec<AetVideoSourceNode>,
}

#[derive(Clone)]
pub struct AetVideoSourceNode {
	pub name: String,
	pub id: u32,
	pub sprite: Option<Rc<Mutex<crate::spr::SpriteInfoNode>>>,
}

impl PartialEq for AetVideoSourceNode {
	fn eq(&self, other: &Self) -> bool {
		if let Some(a) = &self.sprite
			&& let Some(b) = &other.sprite
		{
			Rc::ptr_eq(a, b)
		} else {
			self.name == other.name && self.id == other.id
		}
	}
}

#[derive(Clone, PartialEq)]
pub struct AetAudioNode {
	pub sound_index: u32,
}

struct WgpuAetVideos {
	viewport_size: [f32; 2],
	videos: Vec<WgpuAetVideo>,
}

struct WgpuAetVideo {
	is_ycbcr: bool,
	texture_coords: [f32; 4],
	source_size: [f32; 2],
	texture_index: usize,
	mat: Mat4,
	color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for WgpuAetVideos {
	fn prepare(
		&self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		_screen_descriptor: &egui_wgpu::ScreenDescriptor,
		_egui_encoder: &mut wgpu::CommandEncoder,
		callback_resources: &mut egui_wgpu::CallbackResources,
	) -> Vec<wgpu::CommandBuffer> {
		let resources: &mut WgpuRenderResources = callback_resources.get_mut().unwrap();

		let instances = self
			.videos
			.iter()
			.map(|video| {
				let mut m = video.mat;
				// Offset to match intended position
				m.w = m.x * (video.source_size[0] / 2.0)
					+ m.y * (video.source_size[1] / 2.0)
					+ m.z + m.w;

				let projection = Mat4 {
					x: Vec4 {
						x: 2.0 / self.viewport_size[0],
						y: 0.0,
						z: 0.0,
						w: 0.0,
					},
					y: Vec4 {
						x: 0.0,
						y: -2.0 / self.viewport_size[1],
						z: 0.0,
						w: 0.0,
					},
					z: Vec4 {
						x: 0.0,
						y: 0.0,
						z: 1.0,
						w: 0.0,
					},
					w: Vec4 {
						x: -1.0,
						y: 1.0,
						z: 0.0,
						w: 1.0,
					},
				};

				let mut m = projection * m;
				m.x = m.x * (video.source_size[0] / 2.0);
				m.y = m.y * (-video.source_size[1] / 2.0);

				Instance {
					matrix: m.into(),
					tex_coords: [
						[video.texture_coords[0], video.texture_coords[3]],
						[video.texture_coords[2], video.texture_coords[3]],
						[video.texture_coords[0], video.texture_coords[1]],
						[video.texture_coords[2], video.texture_coords[1]],
					],
					color: video.color,
					texture_index: video.texture_index as u32,
					is_ycbcr: if video.is_ycbcr { 1 } else { 0 },
				}
			})
			.collect::<Vec<_>>();

		if instances.len() * std::mem::size_of::<Instance>()
			> resources.instance_buffer.size() as usize
		{
			resources.instance_buffer =
				device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some("Instance buffer"),
					contents: bytemuck::cast_slice(&instances),
					usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
				});
		} else {
			queue.write_buffer(
				&resources.instance_buffer,
				0,
				bytemuck::cast_slice(&instances),
			);
		}

		Vec::new()
	}

	fn paint(
		&self,
		_info: egui::PaintCallbackInfo,
		render_pass: &mut wgpu::RenderPass<'static>,
		callback_resources: &egui_wgpu::CallbackResources,
	) {
		let resources: &WgpuRenderResources = callback_resources.get().unwrap();
		let textures: &WgpuRenderTextures = callback_resources.get().unwrap();
		render_pass.set_pipeline(&resources.pipeline);
		render_pass.set_bind_group(0, &textures.fragment_bind_group, &[]);
		render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
		render_pass.set_vertex_buffer(1, resources.instance_buffer.slice(..));
		render_pass.draw(0..6, 0..(self.videos.len() as u32));
	}
}
