use crate::app::TreeNode;
use crate::txp::*;
use eframe::egui;
use eframe::egui::Widget;
use eframe::egui_wgpu;
use eframe::egui_wgpu::wgpu;
use egui_material_icons::icons::*;
use egui_plot::PlotItem;
use glam::{Mat4, Vec4};
use kkdlib::*;
use regex::Regex;
use std::collections::*;
use std::ops::*;
use std::rc::Rc;
use std::sync::*;
use transform_gizmo_egui::prelude::*;

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

	fn display_opts(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let height = ui.text_style_height(&egui::TextStyle::Body);
		egui_extras::TableBuilder::new(ui)
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
				.map(|scene| {
					let (root, map) = scene.root.to_kkdlib();

					for (_, b) in &map {
						let mut b = b.try_lock().unwrap();
						let parent: Option<Rc<Mutex<AetLayerNode>>> =
							unsafe { std::mem::transmute(b.parent.clone()) };
						let Some(parent) = &parent else { continue };
						b.parent = map
							.iter()
							.find(|(a, _)| Rc::ptr_eq(a, parent))
							.map(|(_, b)| b.clone());
					}

					aet::Scene {
						name: scene.name.clone(),
						start_time: scene.start_time,
						end_time: scene.end_time,
						fps: scene.fps,
						color: scene.color,
						width: scene.width,
						height: scene.height,
						camera: scene.camera.clone(),
						root,
					}
				})
				.collect(),
		};

		set.to_buf()
	}
}

impl AetSetNode {
	pub fn name_pattern() -> Regex {
		Regex::new(r"(^aet_.*\.bin)|(.aec)$").unwrap()
	}

	pub fn read(name: &str, data: &[u8]) -> Self {
		let set = aet::Set::from_buf(data, name.ends_with("aec"));

		let scenes = set
			.scenes
			.into_iter()
			.map(|scene| {
				let (root, map) = AetCompNode::create(&scene.root);

				for (_, b) in &map {
					let mut b = b.try_lock().unwrap();
					let parent: Option<Rc<Mutex<aet::Layer>>> =
						unsafe { std::mem::transmute(b.parent.clone()) };
					let Some(parent) = &parent else { continue };
					b.parent = map
						.iter()
						.find(|(a, _)| Rc::ptr_eq(a, parent))
						.map(|(_, b)| b.clone());
				}

				AetSceneNode {
					name: scene.name,
					start_time: scene.start_time,
					end_time: scene.end_time,
					fps: scene.fps,
					color: scene.color,
					width: scene.width,
					height: scene.height,
					camera: scene.camera,
					root,

					current_time: scene.start_time,
					playing: false,
					display_placeholders: false,
					centered: false,

					selected_curve: None,
					gizmo: Gizmo::default(),
					background_color: [0.0, 0.0, 0.0],
				}
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
	pub background_color: [f32; 3],
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

	fn has_custom_tree(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		for layer in &mut self.root.layers {
			let mut lock = layer.try_lock().unwrap();
			f(&mut *lock);
		}
	}

	fn display_tree(
		&mut self,
		ui: &mut egui::Ui,
		path: &[usize],
		selected: &mut Vec<usize>,
		frame: &mut eframe::Frame,
		undoer: &mut crate::app::LayerUndoer,
		children: &mut Vec<(Vec<usize>, egui::Response)>,
	) -> egui::Response {
		let resp = crate::app::collapsing_selectable_label(
			ui,
			&self.name,
			path,
			path == *selected,
			|ui| {
				self.root
					.display_tree(ui, path, selected, frame, undoer, children);
			},
		)
		.header_response;

		if self.has_context_menu() {
			let menu = egui::Popup::context_menu(&resp).show(|ui| self.display_ctx_menu(ui, frame));
			if menu.is_some() {
				self.selected(frame);
				*selected = path.to_vec();
			}
		}

		if resp.clicked() {
			self.selected(frame);
			*selected = path.to_vec();
		}

		if self.root.layers.iter().any(|layer| {
			let layer = layer.try_lock().unwrap();
			layer.want_deletion || layer.want_duplicate
		}) {
			*selected = path.to_vec();
			undoer.add_undo(
				AetLayerNode::create_with_item(AetItemNode::Comp(self.root.clone())),
				path.to_vec(),
			);
		}

		self.root
			.layers
			.retain(|layer| !layer.try_lock().unwrap().want_deletion);

		for i in self
			.root
			.layers
			.iter()
			.enumerate()
			.filter(|(_, layer)| layer.try_lock().unwrap().want_duplicate)
			.map(|(i, _)| i)
			.collect::<Vec<_>>()
		{
			let mut cloned = self.root.layers[i].try_lock().unwrap().clone();
			if let AetItemNode::Comp(comp) = &cloned.item {
				cloned.item = AetItemNode::Comp(comp.deep_clone());
			}
			self.root.layers.insert(i, Rc::new(Mutex::new(cloned)));
		}

		for layer in &mut self.root.layers {
			layer.try_lock().unwrap().want_duplicate = false;
		}

		resp
	}

	fn display_opts(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let height = ui.text_style_height(&egui::TextStyle::Body);
		egui_extras::TableBuilder::new(ui)
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
						crate::app::num_edit(ui, &mut self.start_time, 2);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("End time");
					});
					row.col(|ui| {
						crate::app::num_edit(ui, &mut self.end_time, 2);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("FPS");
					});
					row.col(|ui| {
						crate::app::num_edit(ui, &mut self.fps, 0);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Width");
					});
					row.col(|ui| {
						crate::app::num_edit(ui, &mut self.width, 0);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Height");
					});
					row.col(|ui| {
						crate::app::num_edit(ui, &mut self.height, 0);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Background color");
					});
					row.col(|ui| {
						ui.color_edit_button_rgb(&mut self.background_color);
					});
				});
			});
	}

	fn has_context_menu(&self) -> bool {
		true
	}

	fn display_ctx_menu(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		if ui.button("Add").clicked() {
			let mut layer = AetLayerNode::create_with_item(AetItemNode::None);
			layer.sprites = self
				.root
				.layers
				.first()
				.unwrap()
				.try_lock()
				.unwrap()
				.sprites
				.clone();
			layer.flags = aet::LayerFlagsBuilder::new()
				.with_video_active(true)
				.with_audio_active(true)
				.build();
			layer.quality = aet::LayerQuality::Best;
			self.root.layers.push(Rc::new(Mutex::new(layer)));
		}

		if ui.button("Hide all").clicked() {
			for layer in &mut self.root.layers {
				layer.try_lock().unwrap().visible = false;
			}
		}
	}
}

impl AetSceneNode {
	pub fn display_visual(
		&mut self,
		ui: &mut egui::Ui,
		rect: egui::Rect,
		selected: &mut Vec<usize>,
	) {
		let mut mat = Mat4::IDENTITY;
		if self.centered {
			mat.w_axis.x = self.width as f32 / 2.0;
			mat.w_axis.y = self.height as f32 / 2.0;
		}

		if let Some(camera) = &self.camera {
			let mut eye = [0.0; 3];

			eye[0] = camera.eye_x.interpolate(self.current_time) - self.width as f32 * 0.5;
			eye[1] = camera.eye_y.interpolate(self.current_time) - self.height as f32 * 0.5;
			eye[2] = camera.eye_z.interpolate(self.current_time);

			mat.w_axis =
				mat.x_axis * -eye[0] + mat.y_axis * -eye[1] + mat.z_axis * -eye[2] + mat.w_axis;
		}

		let mut videos = WgpuAetVideos {
			videos: Vec::new(),
			sprites: BTreeMap::new(),
			matte_sprites: Vec::new(),
			viewport_size: [self.width as f32, self.height as f32],
			background_color: self.background_color,
		};
		videos.sprites.insert(
			0,
			WgpuAetSpriteInfo {
				texture_index: 0,
				texture_coords: [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]],
			},
		);

		self.root.display(
			mat,
			self.current_time,
			1.0,
			self.display_placeholders,
			&mut videos,
			&[selected[0], selected[1]],
			selected,
		);

		ui.painter()
			.add(egui_wgpu::Callback::new_paint_callback(rect, videos));

		if selected.len() >= 3 {
			let mut frame = self.current_time;
			let mut m = Mat4::IDENTITY;
			let mut opacity = 0.0;
			if self.centered {
				m.w_axis.x = self.width as f32 / 2.0;
				m.w_axis.y = self.height as f32 / 2.0;
			}

			if let Some(video) = &self.root.layers[selected[2]].try_lock().unwrap().video {
				calc_mat(&mut m, &mut opacity, video, frame);
			}

			let selected =
				selected
					.iter()
					.skip(3)
					.fold(self.root.layers[selected[2]].clone(), |layer, i| {
						let layer = layer.try_lock().unwrap();
						let AetItemNode::Comp(comp) = &layer.item else {
							panic!()
						};

						let layer = comp.layers[*i].try_lock().unwrap();

						if let Some(parent) = &layer.parent
							&& let Some(video) = &parent.try_lock().unwrap().video
						{
							calc_mat(&mut m, &mut opacity, video, frame);
						}

						if let Some(video) = &layer.video {
							calc_mat(&mut m, &mut opacity, video, frame);
						}

						frame = (frame - layer.start_time) * layer.time_scale + layer.offset_time;
						comp.layers[*i].clone()
					});

			if !selected.try_lock().unwrap().multi_selected
				&& let Some(video) = &mut selected.try_lock().unwrap().video
			{
				m.w_axis = m.x_axis * video.anchor_x.interpolate(frame)
					+ m.y_axis * video.anchor_y.interpolate(frame)
					+ m.w_axis;
				m.w_axis.y = -m.w_axis.y + self.height as f32;
				m.w_axis.z = 0.0;

				self.gizmo.update_config(GizmoConfig {
					projection_matrix: [
						[2.0 / self.width as f64, 0.0, 0.0, -1.0],
						[0.0, 2.0 / self.height as f64, 0.0, -1.0],
						[0.0, 0.0, 1.0, 0.0],
						[0.0, 0.0, 0.0, 1.0],
					]
					.into(),
					viewport: rect,
					modes: GizmoMode::TranslateX
						| GizmoMode::TranslateY
						| GizmoMode::TranslateView
						| GizmoMode::RotateZ,
					snapping: true,
					snap_distance: 5.0,
					..Default::default()
				});

				let (scale, rotation, translation) = m.to_scale_rotation_translation();
				let transform =
					transform_gizmo_egui::math::Transform::from_scale_rotation_translation(
						[scale.x as f64, scale.y as f64, scale.z as f64],
						[
							rotation.x as f64,
							rotation.y as f64,
							rotation.z as f64,
							rotation.w as f64,
						],
						[
							translation.x as f64,
							translation.y as f64,
							translation.z as f64,
						],
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

							if video.pos_y.keys.is_empty() {
								video.pos_y.keys.push(aet::FCurveKey {
									frame: 0.0,
									value: 0.0,
									tangent: 0.0,
								});
							}

							if ui.input(|i| i.modifiers.ctrl) {
								if let Some(key) =
									video
										.pos_x
										.keys
										.iter_mut()
										.rev()
										.reduce(|previous, current| {
											if previous.frame
												- frame - (previous.frame - current.frame) / 2.0
												> 0.0
											{
												current
											} else {
												previous
											}
										}) {
									key.value += delta.x as f32 / m.x_axis.x;
								} else {
									video.pos_x.keys[0].value += delta.x as f32 / m.x_axis.x;
								}

								if let Some(key) =
									video
										.pos_y
										.keys
										.iter_mut()
										.rev()
										.reduce(|previous, current| {
											if previous.frame
												- frame - (previous.frame - current.frame) / 2.0
												> 0.0
											{
												current
											} else {
												previous
											}
										}) {
									key.value += -delta.y as f32 / m.y_axis.y;
								} else {
									video.pos_y.keys[0].value += -delta.y as f32 / m.y_axis.y;
								}
							} else {
								for key in &mut video.pos_x.keys {
									key.value += delta.x as f32 / m.x_axis.x;
								}

								for key in &mut video.pos_y.keys {
									key.value += -delta.y as f32 / m.y_axis.y;
								}
							}
						}
						GizmoResult::Rotation {
							axis: _,
							delta,
							total: _,
							is_view_axis: _,
						} => {
							if video.rot_z.keys.is_empty() {
								video.rot_z.keys.push(aet::FCurveKey {
									frame: 0.0,
									value: 0.0,
									tangent: 0.0,
								});
							}

							if ui.input(|i| i.modifiers.ctrl) {
								if let Some(key) =
									video
										.rot_z
										.keys
										.iter_mut()
										.rev()
										.reduce(|previous, current| {
											if previous.frame
												- frame - (previous.frame - current.frame) / 2.0
												> 0.0
											{
												current
											} else {
												previous
											}
										}) {
									key.value -= delta.to_degrees() as f32;
								} else {
									video.rot_z.keys[0].value -= delta.to_degrees() as f32;
								}
							} else {
								for key in &mut video.rot_z.keys {
									key.value -= delta.to_degrees() as f32;
								}
							}
						}
						_ => {}
					}
				}
			}
		}

		let resp = ui.interact(rect, ui.next_auto_id(), egui::Sense::click());

		if ui.ctx().dragged_id().is_none()
			&& resp.clicked()
			&& let Some(pointer) = resp.interact_pointer_pos()
		{
			let x = (pointer.x - rect.min.x) / rect.width();
			let y = (pointer.y - rect.min.y) / rect.height();
			if let Some(new_path) = self.root.clicked(
				mat,
				self.current_time,
				1.0,
				self.display_placeholders,
				&[x, y],
				&[self.width as f32, self.height as f32],
				&[selected[0], selected[1]],
			) {
				*selected = new_path;
			}
		}
	}
}

pub fn calc_mat(m: &mut Mat4, opacity: &mut f32, video: &aet::LayerVideo, frame: f32) {
	let mut pos = [0.0; 3];
	let mut scale = [1.0; 3];
	let mut dir = [0.0; 3];
	let mut rot = [0.0; 3];
	let mut anchor = [0.0; 3];

	pos[0] = video.pos_x.interpolate(frame);
	pos[1] = video.pos_y.interpolate(frame);
	rot[2] = video.rot_z.interpolate(frame);
	scale[0] = video.scale_x.interpolate(frame);
	scale[1] = video.scale_y.interpolate(frame);
	anchor[0] = video.anchor_x.interpolate(frame);
	anchor[1] = video.anchor_y.interpolate(frame);
	*opacity *= video.opacity.interpolate(frame).clamp(0.0, 1.0);

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

	m.w_axis = m.x_axis * pos[0] + m.y_axis * pos[1] + m.z_axis * -pos[2] + m.w_axis;
	if dir[0] != 0.0 {
		let rad = -dir[0].to_radians();
		let y = m.y_axis;
		let z = m.z_axis;
		m.y_axis = y * rad.cos() + z * rad.sin();
		m.z_axis = y * -rad.sin() + z * rad.cos();
	}
	if dir[1] != 0.0 {
		let rad = -dir[1].to_radians();
		let x = m.x_axis;
		let z = m.z_axis;
		m.x_axis = x * rad.cos() + z * -rad.sin();
		m.z_axis = x * rad.sin() + z * rad.cos();
	}
	if dir[2] != 0.0 {
		let rad = dir[2].to_radians();
		let x = m.x_axis;
		let y = m.y_axis;
		m.x_axis = x * rad.cos() + y * rad.sin();
		m.y_axis = x * -rad.sin() + y * rad.cos();
	}

	if rot[0] != 0.0 {
		let rad = -rot[0].to_radians();
		let y = m.y_axis;
		let z = m.z_axis;
		m.y_axis = y * rad.cos() + z * rad.sin();
		m.z_axis = y * -rad.sin() + z * rad.cos();
	}
	if rot[1] != 0.0 {
		let rad = -rot[1].to_radians();
		let x = m.x_axis;
		let z = m.z_axis;
		m.x_axis = x * rad.cos() + z * -rad.sin();
		m.z_axis = x * rad.sin() + z * rad.cos();
	}
	if rot[2] != 0.0 {
		let rad = rot[2].to_radians();
		let x = m.x_axis;
		let y = m.y_axis;
		m.x_axis = x * rad.cos() + y * rad.sin();
		m.y_axis = x * -rad.sin() + y * rad.cos();
	}

	m.x_axis *= scale[0];
	m.y_axis *= scale[1];
	m.z_axis *= scale[2];
	m.w_axis = m.x_axis * -anchor[0] + m.y_axis * -anchor[1] + m.z_axis * -anchor[2] + m.w_axis;
}

#[derive(Clone)]
pub struct AetCompNode {
	pub layers: Vec<Rc<Mutex<AetLayerNode>>>,
}

impl PartialEq for AetCompNode {
	fn eq(&self, other: &Self) -> bool {
		self.layers.len() == other.layers.len()
			&& self
				.layers
				.iter()
				.zip(other.layers.iter())
				.all(|(a, b)| Rc::ptr_eq(a, b) || *a.try_lock().unwrap() == *b.try_lock().unwrap())
	}
}

impl AetCompNode {
	fn create(
		comp: &aet::Composition,
	) -> (Self, Vec<(Rc<Mutex<aet::Layer>>, Rc<Mutex<AetLayerNode>>)>) {
		let mut map = Vec::new();
		let layers = comp
			.layers
			.iter()
			.map(|layer_rc| {
				let layer = layer_rc.try_lock().unwrap();
				let item = match &layer.item {
					aet::Item::None => AetItemNode::None,
					aet::Item::Video(video) => AetItemNode::Video(AetVideoNode {
						color: video.color,
						width: video.width,
						height: video.height,
						fpf: video.fpf,
						sources: video
							.sources
							.iter()
							.map(|source| AetVideoSourceNode {
								name: source.name.clone(),
								id: source.id,
								sprite: None,
							})
							.collect(),
					}),
					aet::Item::Audio(audio) => AetItemNode::Audio(AetAudioNode {
						sound_index: audio.sound_index,
					}),
					aet::Item::Composition(comp) => {
						let (comp, new_map) = Self::create(comp);
						map.extend(new_map);
						AetItemNode::Comp(comp)
					}
				};

				let rc = Rc::new(Mutex::new(AetLayerNode {
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
					parent: unsafe { std::mem::transmute(layer.parent.clone()) },
					audio: layer.audio.clone(),

					sprites: Rc::new(Mutex::new(Vec::new())),

					selected_key: 0,
					visible: layer.flags.video_active(),
					multi_selected: false,

					want_deletion: false,
					want_duplicate: false,
				}));

				map.push((layer_rc.clone(), rc.clone()));

				rc
			})
			.collect();
		(Self { layers }, map)
	}

	pub fn get_sprite_id(&self) -> Option<u32> {
		for layer in &self.layers {
			let layer = layer.try_lock().unwrap();
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
			let mut layer = layer.try_lock().unwrap();
			layer.sprites = spr_set.sprites_node.children.clone();
			match &mut layer.item {
				AetItemNode::None => {}
				AetItemNode::Video(video) => {
					for source in &mut video.sources {
						let mut index = None;
						for set in &spr_db.sets {
							let set = set.try_lock().unwrap();
							for entry in &set.entries {
								let entry = entry.try_lock().unwrap();
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

						if index.is_none() {
							for set in &spr_db.sets {
								let set = set.try_lock().unwrap();
								for entry in &set.entries {
									let entry = entry.try_lock().unwrap();
									if entry.name.strip_prefix("SPR_").unwrap_or(&entry.name)
										!= source.name || entry.texture
									{
										continue;
									}
									index = Some(entry.index);
									break;
								}
								if index.is_some() {
									break;
								}
							}
						}

						let Some(index) = index else {
							continue;
						};

						let sprs = spr_set.sprites_node.children.try_lock().unwrap();
						let Some(sprite) = sprs.get(index as usize) else {
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
		path: &[usize],
		selected: &[usize],
	) {
		let mut matte = None;
		for (i, layer) in self.layers.iter().enumerate().rev() {
			let layer = layer.try_lock().unwrap();
			if frame < layer.start_time
				|| frame >= layer.end_time
				|| ((!layer.flags.video_active() || !layer.visible) && matte.is_none())
			{
				continue;
			}

			let mut path = path.to_vec();
			path.push(i);

			let mut m = mat;
			// Parent shouldnt affect opacity
			let mut parent_opacity = 0.0;
			let mut opacity = opacity;
			if let Some(parent) = &layer.parent
				&& let Some(video) = &parent.try_lock().unwrap().video
			{
				calc_mat(&mut m, &mut parent_opacity, video, frame);
			}
			if let Some(video) = &layer.video {
				calc_mat(&mut m, &mut opacity, video, frame);
			}

			if opacity <= 0.001 {
				continue;
			}

			match &layer.item {
				AetItemNode::None => {}
				AetItemNode::Video(video) => {
					let Some(source) = video.sources.first() else {
						if display_placeholders {
							videos.videos.push(WgpuAetVideo {
								has_matte: false,
								is_empty: true,
								source_size: [video.width as f32, video.height as f32],
								sprite_id: 0,
								matte_sprite_index: 0,
								mat: m,
								color: [
									video.color[0] as f32 / 255.0,
									video.color[1] as f32 / 255.0,
									video.color[2] as f32 / 255.0,
									opacity,
								],
								blend_mode: aet::BlendMode::Add,
							});
						}
						continue;
					};
					let Some(sprite) = &source.sprite else {
						continue;
					};

					let sprite = sprite.try_lock().unwrap();
					let texture = sprite.texture.try_lock().unwrap();
					let mip = texture.texture.get_mipmap(0, 0).unwrap();
					let db_entry = sprite.db_entry.as_ref().unwrap().try_lock().unwrap();

					if matte.is_none()
						&& let Some(video) = &layer.video
						&& video.transfer_mode.matte != 0
					{
						matte = Some((
							m,
							sprite.info.px(),
							sprite.info.py(),
							mip.width() as f32,
							mip.height() as f32,
							sprite.info.texid() as usize,
						));
						continue;
					}

					let tl = [
						(sprite.info.px()) / mip.width() as f32,
						(mip.height() as f32 - (sprite.info.py())) / mip.height() as f32,
					];
					let bl = [
						(sprite.info.px()) / mip.width() as f32,
						(mip.height() as f32 - (sprite.info.height() + sprite.info.py()))
							/ mip.height() as f32,
					];
					let br = [
						(sprite.info.width() + sprite.info.px()) / mip.width() as f32,
						(mip.height() as f32 - (sprite.info.height() + sprite.info.py()))
							/ mip.height() as f32,
					];
					let tr = [
						(sprite.info.width() + sprite.info.px()) / mip.width() as f32,
						(mip.height() as f32 - (sprite.info.py())) / mip.height() as f32,
					];

					if !videos.sprites.contains_key(&db_entry.id) {
						videos.sprites.insert(
							db_entry.id,
							WgpuAetSpriteInfo {
								texture_index: sprite.info.texid() as usize,
								texture_coords: [tl, tr, bl, br],
							},
						);
					}

					let video = if let Some((m_mat, m_x, m_y, m_w, m_h, m_texid)) = matte {
						let spr_info = WgpuAetSpriteInfo {
							texture_index: sprite.info.texid() as usize,
							texture_coords: [tl, tr, bl, br],
						};

						let vtx0 = glam::vec3(0.0, 0.0, 0.0);
						let vtx1 = glam::vec3(0.0, sprite.info.height(), 0.0);
						let vtx2 = glam::vec3(sprite.info.width(), sprite.info.height(), 0.0);
						let vtx3 = glam::vec3(sprite.info.width(), 0.0, 0.0);

						let vtx0 = m.transform_point3(vtx0);
						let vtx1 = m.transform_point3(vtx1);
						let vtx2 = m.transform_point3(vtx2);
						let vtx3 = m.transform_point3(vtx3);

						let m_mat = m_mat.inverse();

						let vtx0 = m_mat.transform_point3(vtx0);
						let vtx1 = m_mat.transform_point3(vtx1);
						let vtx2 = m_mat.transform_point3(vtx2);
						let vtx3 = m_mat.transform_point3(vtx3);

						let tl = [(vtx0.x + m_x) / m_w, (m_h - (vtx0.y + m_y)) / m_h];
						let bl = [(vtx1.x + m_x) / m_w, (m_h - (vtx1.y + m_y)) / m_h];
						let br = [(vtx2.x + m_x) / m_w, (m_h - (vtx2.y + m_y)) / m_h];
						let tr = [(vtx3.x + m_x) / m_w, (m_h - (vtx3.y + m_y)) / m_h];

						videos.matte_sprites.push((
							spr_info,
							WgpuAetSpriteInfo {
								texture_index: m_texid,
								texture_coords: [tl, tr, bl, br],
							},
						));

						matte = None;
						WgpuAetVideo {
							has_matte: true,
							is_empty: false,
							source_size: [sprite.info.width(), sprite.info.height()],
							sprite_id: db_entry.id,
							matte_sprite_index: videos.matte_sprites.len() - 1,
							mat: m,
							color: [1.0, 1.0, 1.0, opacity],
							blend_mode: layer
								.video
								.as_ref()
								.map_or(aet::BlendMode::Normal, |video| video.transfer_mode.mode),
						}
					} else {
						WgpuAetVideo {
							has_matte: false,
							is_empty: false,
							source_size: [sprite.info.width(), sprite.info.height()],
							sprite_id: db_entry.id,
							matte_sprite_index: 0,
							mat: m,
							color: [1.0, 1.0, 1.0, opacity],
							blend_mode: layer
								.video
								.as_ref()
								.map_or(aet::BlendMode::Normal, |video| video.transfer_mode.mode),
						}
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
					&path,
					selected,
				),
			}

			// Draw rectangle around selected elem
			if path == selected || layer.multi_selected {
				let (width, height) = if let AetItemNode::Video(video) = &layer.item {
					if let Some(source) = video.sources.first()
						&& let Some(sprite) = &source.sprite
					{
						let sprite = sprite.try_lock().unwrap();
						(sprite.info.width(), sprite.info.height())
					} else {
						(video.width as f32, video.height as f32)
					}
				} else if let Some(video) = &layer.video {
					(
						video.anchor_x.interpolate(frame) * 2.0,
						video.anchor_y.interpolate(frame) * 2.0,
					)
				} else {
					return;
				};

				let top = Mat4::from_cols(
					Vec4::new(width, 0.0, 0.0, 0.0),
					Vec4::new(0.0, 5.0, 0.0, 0.0),
					Vec4::new(0.0, 0.0, 1.0, 0.0),
					Vec4::new(0.0, 0.0, 0.0, 1.0),
				);
				videos.videos.push(WgpuAetVideo {
					has_matte: false,
					is_empty: true,
					source_size: [1.0, 1.0],
					sprite_id: 0,
					matte_sprite_index: 0,
					mat: m * top,
					color: [1.0, 1.0, 1.0, 1.0],
					blend_mode: aet::BlendMode::Add,
				});

				let bottom = Mat4::from_cols(
					Vec4::new(width, 0.0, 0.0, 0.0),
					Vec4::new(0.0, 5.0, 0.0, 0.0),
					Vec4::new(0.0, 0.0, 1.0, 0.0),
					Vec4::new(0.0, height - 5.0, 0.0, 1.0),
				);
				videos.videos.push(WgpuAetVideo {
					has_matte: false,
					is_empty: true,
					source_size: [1.0, 1.0],
					sprite_id: 0,
					matte_sprite_index: 0,
					mat: m * bottom,
					color: [1.0, 1.0, 1.0, 1.0],
					blend_mode: aet::BlendMode::Add,
				});

				let left = Mat4::from_cols(
					Vec4::new(5.0, 0.0, 0.0, 0.0),
					Vec4::new(0.0, height, 0.0, 0.0),
					Vec4::new(0.0, 0.0, 1.0, 0.0),
					Vec4::new(0.0, 0.0, 0.0, 1.0),
				);
				videos.videos.push(WgpuAetVideo {
					has_matte: false,
					is_empty: true,
					source_size: [1.0, 1.0],
					sprite_id: 0,
					matte_sprite_index: 0,
					mat: m * left,
					color: [1.0, 1.0, 1.0, 1.0],
					blend_mode: aet::BlendMode::Add,
				});

				let right = Mat4::from_cols(
					Vec4::new(5.0, 0.0, 0.0, 0.0),
					Vec4::new(0.0, height, 0.0, 0.0),
					Vec4::new(0.0, 0.0, 1.0, 0.0),
					Vec4::new(width - 5.0, 0.0, 0.0, 1.0),
				);
				videos.videos.push(WgpuAetVideo {
					has_matte: false,
					is_empty: true,
					source_size: [1.0, 1.0],
					sprite_id: 0,
					matte_sprite_index: 0,
					mat: m * right,
					color: [1.0, 1.0, 1.0, 1.0],
					blend_mode: aet::BlendMode::Add,
				});
			}
		}
	}

	fn clicked(
		&self,
		mat: Mat4,
		frame: f32,
		opacity: f32,
		display_placeholders: bool,
		pos: &[f32; 2],
		viewport_size: &[f32; 2],
		path: &[usize],
	) -> Option<Vec<usize>> {
		for (i, layer) in self.layers.iter().enumerate() {
			let layer = layer.try_lock().unwrap();
			if frame < layer.start_time
				|| frame >= layer.end_time
				|| !layer.flags.video_active()
				|| !layer.visible
			{
				continue;
			}

			let mut path = path.to_vec();
			path.push(i);

			let mut m = mat;
			let mut parent_opacity = 0.0;
			let mut opacity = opacity;
			if let Some(parent) = &layer.parent
				&& let Some(video) = &parent.try_lock().unwrap().video
			{
				calc_mat(&mut m, &mut parent_opacity, video, frame);
			}
			if let Some(video) = &layer.video {
				calc_mat(&mut m, &mut opacity, video, frame);
			}

			if opacity <= 0.001 {
				continue;
			}

			match &layer.item {
				AetItemNode::None => {}
				AetItemNode::Video(video) => {
					let size = if let Some(source) = video.sources.first()
						&& let Some(sprite) = &source.sprite
					{
						let sprite = sprite.try_lock().unwrap();
						[sprite.info.width(), sprite.info.height()]
					} else if display_placeholders {
						[video.width as f32, video.height as f32]
					} else {
						continue;
					};
					m.w_axis = m.x_axis * (size[0] / 2.0)
						+ m.y_axis * (size[1] / 2.0)
						+ m.z_axis + m.w_axis;

					let projection = Mat4::from_cols(
						Vec4::new(2.0 / viewport_size[0], 0.0, 0.0, 0.0),
						Vec4::new(0.0, 2.0 / viewport_size[1], 0.0, 0.0),
						Vec4::new(0.0, 0.0, 1.0, 0.0),
						Vec4::new(-1.0, -1.0, 0.0, 1.0),
					);

					let mut m = projection * m;
					m.x_axis *= size[0] / 2.0;
					m.y_axis *= size[1] / 2.0;

					let tl = Vec4::new(-1.0, -1.0, 0.0, 1.0);
					let tr = Vec4::new(1.0, -1.0, 0.0, 1.0);
					let bl = Vec4::new(-1.0, 1.0, 0.0, 1.0);
					let br = Vec4::new(1.0, 1.0, 0.0, 1.0);

					let projection = Mat4::from_cols(
						Vec4::new(0.5, 0.0, 0.0, 0.0),
						Vec4::new(0.0, 0.5, 0.0, 0.0),
						Vec4::new(0.0, 0.0, 1.0, 0.0),
						Vec4::new(0.5, 0.5, 0.0, 1.0),
					);

					let mut arr = [
						projection * m * tl,
						projection * m * bl,
						projection * m * tr,
						projection * m * br,
					];
					// (tl/bl), (tr/br)
					arr.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
					if arr[0].y > arr[1].y {
						arr.swap(0, 1);
					}
					if arr[2].y > arr[3].y {
						arr.swap(2, 3);
					}

					// Not 100% accurate but good enough for now
					if (pos[0] > arr[0].x || pos[0] > arr[1].x)
						&& (pos[0] < arr[2].x || pos[0] > arr[3].x)
						&& (pos[1] > arr[0].y || pos[1] > arr[1].y)
						&& (pos[1] < arr[2].y || pos[1] < arr[3].y)
					{
						return Some(path);
					}
				}
				AetItemNode::Audio(_) => {}
				AetItemNode::Comp(comp) => {
					if let Some(new_path) = comp.clicked(
						m,
						(frame - layer.start_time) * layer.time_scale + layer.offset_time,
						opacity,
						display_placeholders,
						pos,
						viewport_size,
						&path,
					) {
						return Some(new_path);
					}
				}
			}
		}
		None
	}

	pub fn show_node_curve_editor(
		&mut self,
		ui: &mut egui::Ui,
		selected_curve: &mut Option<CurveType>,
		frame: f32,
		viewport_size: &[f32; 2],
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
		let mut layer = layer.try_lock().unwrap();
		let mut path = path.to_vec();
		path.push(index);

		let adjusted_frame = (frame - layer.start_time) * layer.time_scale + layer.offset_time;
		if depth + 1 == desired_path.len() - 1 {
			layer.display_curve_editor(ui, selected_curve, frame, viewport_size);
		} else if let AetItemNode::Comp(comp) = &mut layer.item {
			comp.show_node_curve_editor(
				ui,
				selected_curve,
				adjusted_frame,
				viewport_size,
				index,
				depth + 1,
				&path,
				desired_path,
			);
		}
	}

	fn to_kkdlib(
		&self,
	) -> (
		aet::Composition,
		Vec<(Rc<Mutex<AetLayerNode>>, Rc<Mutex<aet::Layer>>)>,
	) {
		let mut map = Vec::new();
		let layers = self
			.layers
			.iter()
			.map(|layer_rc| {
				let layer = layer_rc.try_lock().unwrap();
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
									if let Some(db_entry) = &sprite.try_lock().unwrap().db_entry {
										let db_entry = db_entry.try_lock().unwrap();
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
					AetItemNode::Comp(comp) => {
						let (comp, new_map) = comp.to_kkdlib();
						map.extend(new_map);
						aet::Item::Composition(comp)
					}
				};

				let rc = Rc::new(Mutex::new(aet::Layer {
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
					parent: unsafe { std::mem::transmute(layer.parent.clone()) },
					audio: layer.audio.clone(),
				}));

				map.push((layer_rc.clone(), rc.clone()));

				rc
			})
			.collect();

		(aet::Composition { layers }, map)
	}

	fn display_tree(
		&mut self,
		ui: &mut egui::Ui,
		path: &[usize],
		selected: &mut Vec<usize>,
		frame: &mut eframe::Frame,
		undoer: &mut crate::app::LayerUndoer,
		children: &mut Vec<(Vec<usize>, egui::Response)>,
	) -> egui::Response {
		let mut last_resp = None;
		let resp = egui_dnd::dnd(ui, ui.id()).show_custom(|ui, iter| {
			for (i, layer) in self.layers.iter_mut().enumerate() {
				let mut layer = layer.try_lock().unwrap();
				iter.next(
					ui,
					ui.make_persistent_id(egui::Id::new(path).with(i)),
					i,
					true,
					|ui, item_handle| {
						item_handle.ui(ui, |ui, mut handle, state| {
							ui.horizontal(|ui| {
								let resp = crate::app::show_node(
									ui,
									&mut *layer,
									state.index,
									path,
									selected,
									frame,
									undoer,
									children,
								);

								if ui.available_width() < 10.0 {
									ui.allocate_at_least(
										egui::vec2(10.0, 0.0),
										egui::Sense::empty(),
									);
								}

								let rect = egui::Rect {
									min: egui::pos2(
										resp.rect.max.x + ui.spacing().item_spacing.x,
										resp.rect.min.y,
									),
									max: egui::pos2(
										resp.rect.max.x + ui.available_size().x
											- ui.spacing().item_spacing.x,
										resp.rect.min.y
											+ ui.text_style_height(&egui::TextStyle::Body)
											- ui.spacing().item_spacing.y,
									),
								};

								handle.handle_response(
									ui.interact(
										rect,
										ui.make_persistent_id(
											egui::Id::new(&layer.name)
												.with(path)
												.with(state.index)
												.with("dnd"),
										),
										egui::Sense::click_and_drag(),
									),
									ui,
								);

								last_resp = Some(resp);
							});
						})
					},
				);
			}
		});

		if resp.is_dragging() {
			*selected = path.to_vec();
		}

		if let Some(update) = &resp.final_update() {
			let layer = self.layers.remove(update.from);
			let to = if update.to > update.from {
				update.to - 1
			} else {
				update.to
			};

			self.layers.insert(to, layer);
		}

		last_resp.unwrap_or(ui.response())
	}

	pub fn deep_eq(&self, other: &Self) -> bool {
		self.layers.len() == other.layers.len()
			&& self
				.layers
				.iter()
				.zip(other.layers.iter())
				.all(|(a, b)| *a.try_lock().unwrap() == *b.try_lock().unwrap())
	}

	// One layer deep clone
	pub fn mid_clone(&self) -> Self {
		Self {
			layers: self
				.layers
				.iter()
				.map(|layer| Rc::new(Mutex::new(layer.try_lock().unwrap().clone())))
				.collect(),
		}
	}

	pub fn deep_clone(&self) -> Self {
		Self {
			layers: self
				.layers
				.iter()
				.map(|layer| {
					let mut clone = layer.try_lock().unwrap().clone();
					if let AetItemNode::Comp(comp) = &clone.item {
						clone.item = AetItemNode::Comp(comp.deep_clone());
					}
					Rc::new(Mutex::new(clone))
				})
				.collect(),
		}
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
	pub parent: Option<Rc<Mutex<AetLayerNode>>>,
	pub audio: Option<aet::LayerAudio>,

	pub sprites: Rc<Mutex<Vec<Rc<Mutex<crate::spr::SpriteInfoNode>>>>>,

	pub selected_key: usize,
	pub visible: bool,
	pub multi_selected: bool,

	pub want_deletion: bool,
	pub want_duplicate: bool,
}

impl std::hash::Hash for AetLayerNode {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.name.hash(state);
	}
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

	fn has_custom_tree(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		if let AetItemNode::Comp(comp) = &mut self.item {
			for layer in &mut comp.layers {
				let mut lock = layer.try_lock().unwrap();
				f(&mut *lock);
			}
		}
	}

	fn display_tree(
		&mut self,
		ui: &mut egui::Ui,
		path: &[usize],
		selected: &mut Vec<usize>,
		frame: &mut eframe::Frame,
		undoer: &mut crate::app::LayerUndoer,
		children: &mut Vec<(Vec<usize>, egui::Response)>,
	) -> egui::Response {
		let resp = ui
			.horizontal(|ui| {
				self.label_sameline(ui);

				if let AetItemNode::Comp(comp) = &self.item
					&& !comp.layers.is_empty()
				{
					crate::app::collapsing_selectable_label(
						ui,
						self.name.clone(),
						path,
						path == *selected || self.multi_selected,
						|ui| {
							let AetItemNode::Comp(comp) = &mut self.item else {
								panic!();
							};

							comp.display_tree(ui, path, selected, frame, undoer, children);

							if comp.layers.iter().any(|layer| {
								let layer = layer.try_lock().unwrap();
								layer.want_deletion || layer.want_duplicate
							}) {
								*selected = path.to_vec();
								undoer.add_undo(self.clone(), path.to_vec());
							}

							let AetItemNode::Comp(comp) = &mut self.item else {
								panic!();
							};

							comp.layers
								.retain(|layer| !layer.try_lock().unwrap().want_deletion);

							for i in comp
								.layers
								.iter()
								.enumerate()
								.filter(|(_, layer)| layer.try_lock().unwrap().want_duplicate)
								.map(|(i, _)| i)
								.collect::<Vec<_>>()
							{
								let mut cloned = comp.layers[i].try_lock().unwrap().clone();
								if let AetItemNode::Comp(comp) = &cloned.item {
									cloned.item = AetItemNode::Comp(comp.deep_clone());
								}
								comp.layers.insert(i, Rc::new(Mutex::new(cloned)));
							}

							for layer in &mut comp.layers {
								layer.try_lock().unwrap().want_duplicate = false;
							}
						},
					)
					.header_response
				} else {
					ui.selectable_label(path == *selected || self.multi_selected, &self.name)
				}
			})
			.inner;

		if self.has_context_menu() {
			let menu = egui::Popup::context_menu(&resp).show(|ui| self.display_ctx_menu(ui, frame));
			if menu.is_some() {
				self.selected(frame);
				*selected = path.to_vec();
			}
		}

		if resp.clicked() {
			self.selected(frame);
			*selected = path.to_vec();
		}

		resp
	}

	fn display_opts(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let height = ui.text_style_height(&egui::TextStyle::Body);
		egui_extras::TableBuilder::new(ui)
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
						crate::app::num_edit(ui, &mut self.start_time, 2);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("End time");
					});
					row.col(|ui| {
						crate::app::num_edit(ui, &mut self.end_time, 2);
					});
				});

				if let Some(parent) = &self.parent {
					let parent = parent.try_lock().unwrap();
					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("Parent");
						});
						row.col(|ui| {
							ui.label(&parent.name);
						});
					});
				}

				let mut has_audio = self.audio.is_some();
				let mut has_video = self.video.is_some();
				let mut has_3d = self.video.as_ref().is_some_and(|video| video._3d.is_some());

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

				if let Some(video) = &mut self.video {
					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("Blend mode");
						});
						row.col(|ui| {
							egui::ComboBox::from_id_salt("BlendModeComboBox")
								.selected_text(format!("{:?}", video.transfer_mode.mode))
								.show_ui(ui, |ui| {
									ui.selectable_value(
										&mut video.transfer_mode.mode,
										aet::BlendMode::Normal,
										format!("{:?}", aet::BlendMode::Normal),
									);
									ui.selectable_value(
										&mut video.transfer_mode.mode,
										aet::BlendMode::Add,
										format!("{:?}", aet::BlendMode::Add),
									);
									ui.selectable_value(
										&mut video.transfer_mode.mode,
										aet::BlendMode::Multiply,
										format!("{:?}", aet::BlendMode::Multiply),
									);
									ui.selectable_value(
										&mut video.transfer_mode.mode,
										aet::BlendMode::Screen,
										format!("{:?}", aet::BlendMode::Screen),
									);
									ui.selectable_value(
										&mut video.transfer_mode.mode,
										aet::BlendMode::Overlay,
										format!("{:?}", aet::BlendMode::Overlay),
									);
								});
						});
					});

					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("Matte");
						});
						row.col(|ui| {
							let mut is_matte = video.transfer_mode.matte != 0;
							ui.add(egui::Checkbox::without_text(&mut is_matte));
							video.transfer_mode.matte = if is_matte { 1 } else { 0 };
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
								let mut width = video.width;
								if crate::app::num_edit(ui, &mut width, 0).changed() {
									if let Some(lvideo) = &mut self.video
										&& lvideo.anchor_x.keys.len() == 1
										&& lvideo.anchor_x.keys[0].value == video.width as f32 / 2.0
									{
										lvideo.anchor_x.keys[0].value = width as f32 / 2.0;

										for key in &mut lvideo.pos_x.keys {
											key.value -= video.width as f32 / 2.0;
											key.value += width as f32 / 2.0;
										}
									}
									video.width = width;
								}
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Height");
							});
							row.col(|ui| {
								let mut height = video.height;
								if crate::app::num_edit(ui, &mut height, 0).changed() {
									if let Some(lvideo) = &mut self.video
										&& lvideo.anchor_y.keys.len() == 1
										&& lvideo.anchor_y.keys[0].value
											== video.height as f32 / 2.0
									{
										lvideo.anchor_y.keys[0].value = height as f32 / 2.0;

										for key in &mut lvideo.pos_y.keys {
											key.value -= video.height as f32 / 2.0;
											key.value += height as f32 / 2.0;
										}
									}
									video.height = height;
								}
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("FPF");
							});
							row.col(|ui| {
								crate::app::num_edit(ui, &mut video.fpf, 0);
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
										sprite: self.sprites.try_lock().unwrap().first().cloned(),
									});
								}
							});
						});

						for (i, source) in video.sources.iter_mut().enumerate() {
							let Some(sprite) = &source.sprite else {
								continue;
							};
							let sprite = sprite.try_lock().unwrap();
							let Some(db_entry) = &sprite.db_entry else {
								continue;
							};
							let db_entry = db_entry.try_lock().unwrap();
							source.id = db_entry.id;
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
											for sprite in self.sprites.try_lock().unwrap().iter() {
												let sprite = sprite.try_lock().unwrap();
												let Some(db_entry) = &sprite.db_entry else {
													continue;
												};
												let db_entry = db_entry.try_lock().unwrap();
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
									.try_lock()
									.unwrap()
									.iter()
									.find(|spr| {
										spr.try_lock().unwrap().db_entry.is_some()
											&& spr
												.try_lock()
												.unwrap()
												.db_entry
												.as_ref()
												.unwrap()
												.try_lock()
												.unwrap()
												.id == selected_sprite
									})
									.cloned();

								source.id = selected_sprite;
							}
						}
					}
					AetItemNode::Audio(audio) => {
						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Sound index");
							});
							row.col(|ui| {
								crate::app::num_edit(ui, &mut audio.sound_index, 0);
							});
						});
					}
					AetItemNode::Comp(_) => {}
				}

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Video active");
					});
					row.col(|ui| {
						let mut video_active = self.flags.video_active();
						if egui::Checkbox::without_text(&mut video_active)
							.ui(ui)
							.changed()
						{
							self.flags.set_video_active(video_active);
							self.visible = video_active;
						}
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Audio active");
					});
					row.col(|ui| {
						let mut audio_active = self.flags.audio_active();
						if egui::Checkbox::without_text(&mut audio_active)
							.ui(ui)
							.changed()
						{
							self.flags.set_audio_active(audio_active);
						}
					});
				});

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

				self.markers.retain_mut(|(name, value)| {
					let mut want_deletion = false;
					body.row(height, |mut row| {
						row.col(|ui| {
							ui.text_edit_singleline(name);
						});
						row.col(|ui| {
							ui.horizontal(|ui| {
								crate::app::num_edit(ui, value, 2);
								if ui.button(ICON_REMOVE).clicked() {
									want_deletion = true;
								}
							});
						});
					});
					!want_deletion
				});
			});
	}

	fn has_context_menu(&self) -> bool {
		true
	}

	fn display_ctx_menu(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		if let AetItemNode::Comp(comp) = &mut self.item
			&& ui.button("Add").clicked()
		{
			let mut layer = Self::create_with_item(AetItemNode::None);
			layer.sprites = self.sprites.clone();
			layer.flags = aet::LayerFlagsBuilder::new()
				.with_video_active(true)
				.with_audio_active(true)
				.build();
			layer.quality = aet::LayerQuality::Best;
			comp.layers.push(Rc::new(Mutex::new(layer)));
		}

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
		viewport_size: &[f32; 2],
	) {
		let curve = match selected_curve {
			None => None,

			Some(selected_curve) => match selected_curve {
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
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.anchor_z)),
				CurveType::PosZ => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.pos_z)),
				CurveType::DirX => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_x)),
				CurveType::DirY => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_y)),
				CurveType::DirZ => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_z)),
				CurveType::RotX => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.rot_x)),
				CurveType::RotY => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.rot_y)),
				CurveType::ScaleZ => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.scale_z)),
			},
		};

		egui::SidePanel::right("KeyEditor")
			.resizable(true)
			.show_inside(ui, |ui| {
				let Some(curve) = curve else { return };

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

					if ui.button(ICON_ADD).clicked()
						|| ui.input_mut(|i| {
							i.consume_shortcut(&egui::KeyboardShortcut {
								modifiers: egui::Modifiers::COMMAND,
								logical_key: egui::Key::I,
							})
						}) {
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
					if crate::app::num_edit(ui, &mut curve.keys[self.selected_key].frame, 2)
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
					crate::app::num_edit(ui, &mut curve.keys[self.selected_key].value, 2);
				});

				ui.horizontal(|ui| {
					ui.label("Tangent");
					crate::app::num_edit(ui, &mut curve.keys[self.selected_key].tangent, 2);
				});
			});

		let curve_size = OnceLock::new();

		let bottom_panel = egui::TopBottomPanel::bottom("CurveSelector")
			.resizable(true)
			.show_inside(ui, |ui| {
				egui::ScrollArea::vertical().show(ui, |ui| {
					fn draw_keyframes(
						layer: &AetLayerNode,
						ui: &mut egui::Ui,
						resp: &egui::Response,
						curve: &aet::FCurve,
						curve_size: &OnceLock<(f32, f32)>,
					) {
						let (start, end) = curve_size.get_or_init(|| {
							(
								resp.rect.max.x + ui.style().spacing.item_spacing.x,
								ui.available_width() - ui.style().spacing.item_spacing.x,
							)
						});

						ui.painter().hline(
							*start..=*end,
							resp.rect.center().y,
							egui::Stroke::new(1.5, egui::Color32::from_rgb(0x10, 0x60, 0xE0)),
						);

						if curve.keys.len() <= 1 {
							return;
						}

						for key in &curve.keys {
							let frame = (key.frame - layer.start_time)
								/ (layer.end_time - layer.start_time);
							let pos = (end - start) * frame;
							ui.painter().circle_filled(
								egui::pos2(start + pos, resp.rect.center().y),
								5.0,
								egui::Color32::from_rgba_unmultiplied(0xF0, 0xB0, 0x20, 0xA0),
							);
						}
					}

					if let Some(audio) = &self.audio {
						let resp = ui.selectable_label(
							*selected_curve == Some(CurveType::VolumeL),
							"Volume L",
						);
						draw_keyframes(self, ui, &resp, &audio.volume_l, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::VolumeL);
							self.selected_key = 0;
						}

						let resp = ui.selectable_label(
							*selected_curve == Some(CurveType::VolumeR),
							"Volume R",
						);
						draw_keyframes(self, ui, &resp, &audio.volume_r, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::VolumeR);
							self.selected_key = 0;
						}

						let resp =
							ui.selectable_label(*selected_curve == Some(CurveType::PanL), "Pan L");
						draw_keyframes(self, ui, &resp, &audio.pan_l, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::PanL);
							self.selected_key = 0;
						}

						let resp =
							ui.selectable_label(*selected_curve == Some(CurveType::PanR), "Pan R");
						draw_keyframes(self, ui, &resp, &audio.pan_r, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::PanR);
							self.selected_key = 0;
						}
					}

					if let Some(video) = &self.video {
						let resp = ui.selectable_label(
							*selected_curve == Some(CurveType::AnchorX),
							"Anchor X",
						);
						draw_keyframes(self, ui, &resp, &video.anchor_x, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::AnchorX);
							self.selected_key = 0;
						}

						let resp = ui.selectable_label(
							*selected_curve == Some(CurveType::AnchorY),
							"Anchor Y",
						);
						draw_keyframes(self, ui, &resp, &video.anchor_y, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::AnchorY);
							self.selected_key = 0;
						}

						if let Some(_3d) = &video._3d {
							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::AnchorZ),
								"Anchor Z",
							);
							draw_keyframes(self, ui, &resp, &_3d.anchor_z, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::AnchorZ);
								self.selected_key = 0;
							}
						}

						let resp =
							ui.selectable_label(*selected_curve == Some(CurveType::PosX), "Pos X");
						draw_keyframes(self, ui, &resp, &video.pos_x, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::PosX);
							self.selected_key = 0;
						}

						let resp =
							ui.selectable_label(*selected_curve == Some(CurveType::PosY), "Pos Y");
						draw_keyframes(self, ui, &resp, &video.pos_y, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::PosY);
							self.selected_key = 0;
						}

						if let Some(_3d) = &video._3d {
							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::PosZ),
								"Pos Z",
							);
							draw_keyframes(self, ui, &resp, &_3d.pos_z, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::PosZ);
								self.selected_key = 0;
							}

							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::DirX),
								"Dir X",
							);
							draw_keyframes(self, ui, &resp, &_3d.dir_x, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::DirX);
								self.selected_key = 0;
							}

							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::DirY),
								"Dir Y",
							);
							draw_keyframes(self, ui, &resp, &_3d.dir_y, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::DirY);
								self.selected_key = 0;
							}

							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::DirZ),
								"Dir Z",
							);
							draw_keyframes(self, ui, &resp, &_3d.dir_z, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::DirZ);
								self.selected_key = 0;
							}

							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::RotX),
								"Rot X",
							);
							draw_keyframes(self, ui, &resp, &_3d.rot_x, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::RotX);
								self.selected_key = 0;
							}

							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::RotY),
								"Rot Y",
							);
							draw_keyframes(self, ui, &resp, &_3d.rot_y, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::RotY);
								self.selected_key = 0;
							}
						}

						let resp =
							ui.selectable_label(*selected_curve == Some(CurveType::RotZ), "Rot Z");
						draw_keyframes(self, ui, &resp, &video.rot_z, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::RotZ);
							self.selected_key = 0;
						}

						let resp = ui.selectable_label(
							*selected_curve == Some(CurveType::ScaleX),
							"Scale X",
						);
						draw_keyframes(self, ui, &resp, &video.scale_x, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::ScaleX);
							self.selected_key = 0;
						}

						let resp = ui.selectable_label(
							*selected_curve == Some(CurveType::ScaleY),
							"Scale Y",
						);
						draw_keyframes(self, ui, &resp, &video.scale_y, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::ScaleY);
							self.selected_key = 0;
						}

						if let Some(_3d) = &video._3d {
							let resp = ui.selectable_label(
								*selected_curve == Some(CurveType::ScaleZ),
								"Scale Z",
							);
							draw_keyframes(self, ui, &resp, &_3d.scale_z, &curve_size);
							if resp.clicked() {
								*selected_curve = Some(CurveType::ScaleZ);
								self.selected_key = 0;
							}
						}

						let resp = ui.selectable_label(
							*selected_curve == Some(CurveType::Opacity),
							"Opacity",
						);
						draw_keyframes(self, ui, &resp, &video.opacity, &curve_size);
						if resp.clicked() {
							*selected_curve = Some(CurveType::Opacity);
							self.selected_key = 0;
						}
					}

					ui.take_available_space();
				});
			});

		let curve = match selected_curve {
			None => None,

			Some(selected_curve) => match selected_curve {
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
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.anchor_z)),
				CurveType::PosZ => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.pos_z)),
				CurveType::DirX => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_x)),
				CurveType::DirY => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_y)),
				CurveType::DirZ => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.dir_z)),
				CurveType::RotX => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.rot_x)),
				CurveType::RotY => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.rot_y)),
				CurveType::ScaleZ => self
					.video
					.as_mut()
					.and_then(|video| video._3d.as_mut().map(|_3d| &mut _3d.scale_z)),
			},
		};

		let Some(curve) = curve else { return };

		if curve.keys.len() <= 1 {
			return;
		}

		let mut bounds = match selected_curve.unwrap() {
			CurveType::VolumeL => [0.0, 2.0],
			CurveType::VolumeR => [0.0, 2.0],
			CurveType::PanL => [-1.0, 1.0],
			CurveType::PanR => [-1.0, 1.0],

			CurveType::AnchorX => [0.0, viewport_size[0] as f64],
			CurveType::AnchorY => [0.0, viewport_size[1] as f64],
			CurveType::PosX => [0.0, viewport_size[0] as f64],
			CurveType::PosY => [0.0, viewport_size[1] as f64],
			CurveType::RotZ => [0.0, 360.0],
			CurveType::ScaleX => [0.0, 2.0],
			CurveType::ScaleY => [0.0, 2.0],
			CurveType::Opacity => [0.0, 1.0],

			CurveType::AnchorZ => [-1.0, 1.0],
			CurveType::PosZ => [-1.0, 1.0],
			CurveType::DirX => [0.0, 360.0],
			CurveType::DirY => [0.0, 360.0],
			CurveType::DirZ => [0.0, 360.0],
			CurveType::RotX => [0.0, 360.0],
			CurveType::RotY => [0.0, 360.0],
			CurveType::ScaleZ => [0.0, 2.0],
		};

		let min = curve
			.keys
			.iter()
			.map(|key| key.value as f64)
			.reduce(f64::min)
			.unwrap_or(0.0);

		let max = curve
			.keys
			.iter()
			.map(|key| key.value as f64)
			.reduce(f64::max)
			.unwrap_or(0.0);

		let max_decimals = if bounds[1] > 1.0 { 0 } else { 2 };

		if min < bounds[0] {
			bounds[0] = min;
		}

		if max > bounds[1] {
			bounds[1] = max;
		}

		let mut rect = egui::Rect {
			min: ui.cursor().min,
			max: egui::Pos2 {
				x: ui.cursor().max.x,
				y: ui.cursor().min.y + ui.available_height(),
			},
		}
		.shrink(ui.text_style_height(&egui::TextStyle::Body));

		if rect.max.y >= bottom_panel.response.rect.min.y || rect.height().is_sign_negative() {
			rect = ui
				.allocate_space(egui::vec2(
					ui.available_width(),
					100.0 + bottom_panel.response.rect.height(),
				))
				.1
				.shrink(ui.text_style_height(&egui::TextStyle::Body));
			rect.max.y -= bottom_panel.response.rect.height();
		}

		let line_stroke = egui::Stroke::new(1.0, egui::Color32::GRAY);

		ui.painter().text(
			egui::pos2(rect.min.x, rect.max.y),
			egui::Align2::LEFT_CENTER,
			format!("{:.1$}", bounds[0], max_decimals),
			egui::FontSelection::Default.resolve(ui.style()),
			egui::Color32::GRAY,
		);

		let (curve_start, curve_end) = curve_size.get().unwrap();

		ui.painter()
			.hline(*curve_start..=*curve_end, rect.max.y, line_stroke);

		for i in 1..=4 {
			let y = rect.max.y - rect.height() * (i as f32 / 4.0);

			ui.painter().text(
				egui::pos2(rect.min.x, y),
				egui::Align2::LEFT_CENTER,
				format!(
					"{:.1$}",
					(bounds[1] - bounds[0]) * (i as f64 / 4.0) + bounds[0],
					max_decimals
				),
				egui::FontSelection::Default.resolve(ui.style()),
				egui::Color32::GRAY,
			);

			ui.painter()
				.hline(*curve_start..=*curve_end, y, line_stroke);
		}

		rect.min.x = *curve_start;
		rect.max.x = *curve_end;

		{
			let plot = egui_plot::PlotTransform::new(
				rect,
				egui_plot::PlotBounds::from_min_max(
					[self.start_time as f64, bounds[0]],
					[self.end_time as f64, bounds[1]],
				),
				[false, false],
			);

			let mut curve = curve.clone();
			curve.keys.sort_by(|a, b| a.frame.total_cmp(&b.frame));

			let mut line = egui_plot::Line::new(
				String::new(),
				egui_plot::PlotPoints::from_explicit_callback(
					|x| curve.interpolate(x as f32) as f64,
					(self.start_time as f64)..=(self.end_time as f64),
					1000,
				),
			)
			.color(egui::Color32::from_rgb(0x10, 0x60, 0xE0));
			line.initialize((self.start_time as f64)..=(self.end_time as f64));

			let mut shapes = Vec::new();
			line.shapes(ui, &plot, &mut shapes);
			ui.painter().add(shapes);
		}

		let mut want_sort = false;
		for (i, key) in curve.keys.iter_mut().enumerate() {
			let x_pos =
				rect.width() * (key.frame - self.start_time) / (self.end_time - self.start_time);
			let y_pos = rect.height() * (key.value - bounds[0] as f32)
				/ (bounds[1] as f32 - bounds[0] as f32);
			let pos = egui::pos2(rect.min.x + x_pos, rect.max.y - y_pos);
			ui.painter().circle_filled(
				pos,
				5.0,
				egui::Color32::from_rgba_unmultiplied(0xF0, 0xB0, 0x20, 0xA0),
			);

			let resp = ui.interact(
				egui::Rect {
					min: pos - egui::Vec2::splat(2.5),
					max: pos + egui::Vec2::splat(2.5),
				},
				ui.auto_id_with(format!("key{i}")),
				egui::Sense::click_and_drag(),
			);

			if resp.clicked() {
				self.selected_key = i;
			} else if resp.dragged()
				&& let Some(pos) = resp.interact_pointer_pos()
			{
				let pos = pos.clamp(rect.min, rect.max);
				let x_pos = pos.x - rect.min.x;
				let y_pos = -(pos.y - rect.max.y);
				key.frame =
					x_pos * (self.end_time - self.start_time) / rect.width() + self.start_time;
				key.value = y_pos * (bounds[1] as f32 - bounds[0] as f32) / rect.height()
					+ bounds[0] as f32;
			} else if resp.drag_stopped() {
				want_sort = true;
			}
		}

		if want_sort {
			curve.keys.sort_by(|a, b| a.frame.total_cmp(&b.frame));
		}

		if frame >= self.start_time && frame <= self.end_time {
			ui.painter().vline(
				rect.min.x
					+ rect.width() * (frame - self.start_time) / (self.end_time - self.start_time),
				rect.min.y..=rect.max.y,
				egui::Stroke::new(
					1.0,
					egui::Color32::from_rgba_unmultiplied(0xD0, 0x50, 0x60, 0xA0),
				),
			);
		}
	}

	pub fn create_with_item(item: AetItemNode) -> Self {
		Self {
			name: String::from("DUMMY"),
			start_time: 0.0,
			end_time: 0.0,
			offset_time: 0.0,
			time_scale: 1.0,
			flags: kkdlib::aet::LayerFlags::new(),
			quality: kkdlib::aet::LayerQuality::None,
			item,
			markers: Vec::new(),
			video: None,
			parent: None,
			audio: None,
			sprites: Rc::new(Mutex::new(Vec::new())),
			selected_key: 0,
			visible: false,
			multi_selected: false,
			want_deletion: false,
			want_duplicate: false,
		}
	}

	pub fn deep_clone(&self) -> Self {
		Self {
			name: self.name.clone(),
			start_time: self.start_time.clone(),
			end_time: self.end_time.clone(),
			offset_time: self.offset_time.clone(),
			time_scale: self.time_scale.clone(),
			flags: self.flags.clone(),
			quality: self.quality.clone(),
			item: if let AetItemNode::Comp(comp) = &self.item {
				AetItemNode::Comp(comp.deep_clone())
			} else {
				self.item.clone()
			},
			markers: self.markers.clone(),
			video: self.video.clone(),
			parent: self.parent.clone(),
			audio: self.audio.clone(),
			sprites: self.sprites.clone(),
			selected_key: self.selected_key.clone(),
			visible: self.visible.clone(),
			multi_selected: false,
			want_deletion: false,
			want_duplicate: false,
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
	background_color: [f32; 3],
	videos: Vec<WgpuAetVideo>,
	sprites: BTreeMap<u32, WgpuAetSpriteInfo>,
	matte_sprites: Vec<(WgpuAetSpriteInfo, WgpuAetSpriteInfo)>,
}

struct WgpuAetVideo {
	has_matte: bool,
	is_empty: bool,
	blend_mode: aet::BlendMode,
	source_size: [f32; 2],
	sprite_id: u32,
	matte_sprite_index: usize,
	mat: Mat4,
	color: [f32; 4],
}

struct WgpuAetSpriteInfo {
	texture_index: usize,
	texture_coords: [[f32; 2]; 4],
}

struct WgpuSpriteInfos {
	vertex_buffer: wgpu::Buffer,
	uniform_buffer: wgpu::Buffer,
	uniform_buffer_views: Vec<wgpu::BindGroup>,
	sprites: BTreeMap<u32, i32>,
	matte_sprites: Vec<i32>,
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
		let resources: &WgpuRenderResources = callback_resources.get().unwrap();
		let video_bind_group_layout = resources.video_bind_group_layout.clone();
		let video_size = std::mem::size_of::<VideoInfo>()
			.next_multiple_of(device.limits().min_uniform_buffer_offset_alignment as usize);

		let projection = Mat4::from_cols(
			Vec4::new(2.0 / self.viewport_size[0], 0.0, 0.0, 0.0),
			Vec4::new(0.0, -2.0 / self.viewport_size[1], 0.0, 0.0),
			Vec4::new(0.0, 0.0, 1.0, 0.0),
			Vec4::new(-1.0, 1.0, 0.0, 1.0),
		);

		queue.write_buffer(
			&resources.projection_buffer,
			0,
			bytemuck::bytes_of(&projection),
		);

		if callback_resources.get::<WgpuSpriteInfos>().is_none() {
			let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
				label: Some("Uniform buffer"),
				size: video_size as wgpu::BufferAddress * 256,
				usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
				mapped_at_creation: false,
			});
			callback_resources.insert(WgpuSpriteInfos {
				vertex_buffer: device.create_buffer(&wgpu::BufferDescriptor {
					label: Some("Vertex buffer"),
					size: std::mem::size_of::<Vertex>() as wgpu::BufferAddress * 4 * 256,
					usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
					mapped_at_creation: false,
				}),
				uniform_buffer_views: (0..256)
					.map(|i| {
						device.create_bind_group(&wgpu::BindGroupDescriptor {
							layout: &video_bind_group_layout,
							entries: &[wgpu::BindGroupEntry {
								binding: 0,
								resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
									buffer: &uniform_buffer,
									offset: i as wgpu::BufferAddress
										* video_size as wgpu::BufferAddress,
									size: unsafe {
										Some(wgpu::BufferSize::new_unchecked(std::mem::size_of::<
											VideoInfo,
										>()
											as wgpu::BufferAddress))
									},
								}),
							}],
							label: Some(&format!("Uniform bind group {i}")),
						})
					})
					.collect(),
				uniform_buffer,
				sprites: BTreeMap::new(),
				matte_sprites: Vec::new(),
			});
		}

		let wgpu_sprite_infos: &mut WgpuSpriteInfos = callback_resources.get_mut().unwrap();
		wgpu_sprite_infos.sprites.clear();
		wgpu_sprite_infos.matte_sprites.clear();

		let sprites = self.sprites.iter().enumerate().map(|(i, (id, sprite))| {
			let index = i as i32 * 4;
			wgpu_sprite_infos.sprites.insert(*id, index);
			[
				Vertex {
					position: [-1.0, 1.0],
					tex_coords: sprite.texture_coords[0],
					matte_tex_coords: [0.0, 0.0],
				},
				Vertex {
					position: [1.0, 1.0],
					tex_coords: sprite.texture_coords[1],
					matte_tex_coords: [0.0, 0.0],
				},
				Vertex {
					position: [-1.0, -1.0],
					tex_coords: sprite.texture_coords[2],
					matte_tex_coords: [0.0, 0.0],
				},
				Vertex {
					position: [1.0, -1.0],
					tex_coords: sprite.texture_coords[3],
					matte_tex_coords: [0.0, 0.0],
				},
			]
		});

		let matte_sprites = self
			.matte_sprites
			.iter()
			.enumerate()
			.map(|(i, (base, matte))| {
				let index = (self.sprites.len() as i32 + i as i32) * 4;
				wgpu_sprite_infos.matte_sprites.push(index);
				[
					Vertex {
						position: [-1.0, 1.0],
						tex_coords: base.texture_coords[0],
						matte_tex_coords: matte.texture_coords[0],
					},
					Vertex {
						position: [1.0, 1.0],
						tex_coords: base.texture_coords[1],
						matte_tex_coords: matte.texture_coords[1],
					},
					Vertex {
						position: [-1.0, -1.0],
						tex_coords: base.texture_coords[2],
						matte_tex_coords: matte.texture_coords[2],
					},
					Vertex {
						position: [1.0, -1.0],
						tex_coords: base.texture_coords[3],
						matte_tex_coords: matte.texture_coords[3],
					},
				]
			});

		let verticies = sprites.chain(matte_sprites).collect::<Vec<_>>();
		if verticies.len() as wgpu::BufferAddress
			* std::mem::size_of::<Vertex>() as wgpu::BufferAddress
			* 4 > wgpu_sprite_infos.vertex_buffer.size()
		{
			wgpu_sprite_infos.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
				label: Some("Vertex buffer"),
				size: verticies.len().next_power_of_two() as wgpu::BufferAddress
					* std::mem::size_of::<Vertex>() as wgpu::BufferAddress
					* 4,
				usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
				mapped_at_creation: false,
			});
		}

		queue.write_buffer(
			&wgpu_sprite_infos.vertex_buffer,
			0,
			bytemuck::cast_slice(&verticies),
		);

		let video_infos = [VideoInfo {
			matrix: Mat4::from_cols(
				Vec4::new(self.viewport_size[0] / 2.0, 0.0, 0.0, 0.0),
				Vec4::new(0.0, self.viewport_size[1] / 2.0, 0.0, 0.0),
				Vec4::new(0.0, 0.0, 1.0, 0.0),
				Vec4::new(
					self.viewport_size[0] / 2.0,
					self.viewport_size[1] / 2.0,
					0.0,
					1.0,
				),
			)
			.to_cols_array_2d(),
			color: [
				self.background_color[0],
				self.background_color[1],
				self.background_color[2],
				1.0,
			],
			has_matte: 0,
			_padding_0: 0,
			_padding_1: 0,
			_padding_2: 0,
		}]
		.into_iter()
		.chain(self.videos.iter().map(|video| {
			let mut m = video.mat;
			m.w_axis = m.x_axis * (video.source_size[0] / 2.0)
				+ m.y_axis * (video.source_size[1] / 2.0)
				+ m.z_axis + m.w_axis;

			m.x_axis *= video.source_size[0] / 2.0;
			m.y_axis *= -video.source_size[1] / 2.0;

			VideoInfo {
				matrix: m.to_cols_array_2d(),
				color: video.color,
				has_matte: if video.has_matte { 1 } else { 0 },
				_padding_0: 0,
				_padding_1: 0,
				_padding_2: 0,
			}
		}))
		.collect::<Vec<_>>();

		if video_infos.len() as wgpu::BufferAddress * video_size as wgpu::BufferAddress
			> wgpu_sprite_infos.uniform_buffer.size()
		{
			wgpu_sprite_infos.uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
				label: Some("Uniform buffer"),
				size: video_infos.len().next_power_of_two() as wgpu::BufferAddress
					* video_size as wgpu::BufferAddress,
				usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
				mapped_at_creation: false,
			});

			wgpu_sprite_infos.uniform_buffer_views = (0..video_infos.len().next_power_of_two())
				.map(|i| {
					device.create_bind_group(&wgpu::BindGroupDescriptor {
						layout: &video_bind_group_layout,
						entries: &[wgpu::BindGroupEntry {
							binding: 0,
							resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
								buffer: &wgpu_sprite_infos.uniform_buffer,
								offset: i as wgpu::BufferAddress
									* video_size as wgpu::BufferAddress,
								size: unsafe {
									Some(wgpu::BufferSize::new_unchecked(std::mem::size_of::<
										VideoInfo,
									>()
										as wgpu::BufferAddress))
								},
							}),
						}],
						label: Some(&format!("Uniform bind group {i}")),
					})
				})
				.collect();
		}

		let mut bytes = vec![0; video_size * video_infos.len()];
		for (i, video) in video_infos.iter().enumerate() {
			bytes[(i * video_size)..(i * video_size + std::mem::size_of::<VideoInfo>())]
				.copy_from_slice(bytemuck::bytes_of(video));
		}

		queue.write_buffer(&wgpu_sprite_infos.uniform_buffer, 0, &bytes);

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
		let sprites: &WgpuSpriteInfos = callback_resources.get().unwrap();

		// Draw black base
		render_pass.set_pipeline(&resources.pipeline_normal);
		render_pass.set_bind_group(0, &resources.sampler, &[]);
		render_pass.set_bind_group(1, &textures.empty_texture, &[]);
		render_pass.set_bind_group(2, &textures.empty_texture, &[]);
		render_pass.set_bind_group(3, &sprites.uniform_buffer_views[0], &[]);
		render_pass.set_vertex_buffer(0, sprites.vertex_buffer.slice(..));
		render_pass.set_index_buffer(resources.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		render_pass.draw_indexed(0..6, 0, 0..1);

		for (i, video) in self.videos.iter().enumerate() {
			match video.blend_mode {
				aet::BlendMode::Screen => render_pass.set_pipeline(&resources.pipeline_screen),
				aet::BlendMode::Add => render_pass.set_pipeline(&resources.pipeline_add),
				aet::BlendMode::Multiply => render_pass.set_pipeline(&resources.pipeline_multiply),
				_ => render_pass.set_pipeline(&resources.pipeline_normal),
			}

			let vertex_offset = if video.is_empty {
				render_pass.set_bind_group(1, &textures.empty_texture, &[]);
				0
			} else if video.has_matte {
				let (base, matte) = &self.matte_sprites[video.matte_sprite_index];
				render_pass.set_bind_group(
					1,
					&textures.fragment_bind_group[base.texture_index].1,
					&[],
				);
				render_pass.set_bind_group(
					2,
					&textures.fragment_bind_group[matte.texture_index].1,
					&[],
				);
				sprites.matte_sprites[video.matte_sprite_index]
			} else {
				render_pass.set_bind_group(
					1,
					&textures.fragment_bind_group[self.sprites[&video.sprite_id].texture_index].1,
					&[],
				);
				sprites.sprites[&video.sprite_id]
			};

			render_pass.set_bind_group(3, &sprites.uniform_buffer_views[i + 1], &[]);
			render_pass.draw_indexed(0..6, vertex_offset, 0..1);
		}
	}
}
