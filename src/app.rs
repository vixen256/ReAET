use crate::*;
use eframe::egui;
use eframe::egui::NumExt;
use egui_material_icons::icons::*;
use parking_lot::*;
use pollster::FutureExt;
use std::collections::*;
use std::path::PathBuf;
use std::rc::Rc;
use transform_gizmo_egui::prelude::*;

pub trait TreeNode {
	fn label(&self) -> &str;
	fn label_sameline(&mut self, _ui: &mut egui::Ui) {}
	fn has_children(&self) -> bool {
		false
	}
	fn has_custom_tree(&self) -> bool {
		false
	}
	fn has_context_menu(&self) -> bool {
		false
	}
	fn display_children(&mut self, _f: &mut dyn FnMut(&mut dyn TreeNode)) {}
	fn display_tree(
		&mut self,
		ui: &mut egui::Ui,
		_path: &[usize],
		_selected: &mut Vec<usize>,
		_frame: &mut eframe::Frame,
		_undoer: &mut LayerUndoer,
		_children: &mut Vec<(Vec<usize>, egui::Response)>,
	) -> egui::Response {
		ui.response()
	}
	fn selected(&mut self, _frame: &mut eframe::Frame) {}
	fn display_visual(
		&mut self,
		_ui: &mut egui::Ui,
		_rect: egui::Rect,
	) -> Option<egui::epaint::PaintCallback> {
		None
	}
	fn display_opts(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
	fn display_ctx_menu(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
	fn raw_data(&self) -> Vec<u8> {
		Vec::new()
	}
}

// Based on egui::util::Undoer
pub struct LayerUndoer {
	undos: VecDeque<(aet::AetLayerNode, Vec<usize>)>,
	redos: Vec<(aet::AetLayerNode, Vec<usize>)>,
	original_layer: aet::AetLayerNode,
	current_path: Vec<usize>,
	flux: Option<(f64, aet::AetLayerNode)>,
}

impl LayerUndoer {
	pub fn new() -> Self {
		Self {
			undos: VecDeque::new(),
			redos: Vec::new(),
			original_layer: aet::AetLayerNode::create_with_item(aet::AetItemNode::None),
			current_path: Vec::new(),
			flux: None,
		}
	}

	pub fn has_undo(&self) -> bool {
		match self.undos.len() {
			0 => self.flux.is_some(),
			_ => true,
		}
	}

	pub fn has_redo(&self) -> bool {
		!self.redos.is_empty() && self.flux.is_none()
	}

	pub fn undo(&mut self) -> Option<(aet::AetLayerNode, Vec<usize>)> {
		if self.flux.is_some() {
			self.flux = None;
			let res = (self.original_layer.clone(), self.current_path.clone());
			self.current_path = Vec::new();
			Some(res)
		} else {
			self.current_path = Vec::new();
			self.undos.pop_back()
		}
	}

	pub fn redo(&mut self) -> Option<(aet::AetLayerNode, Vec<usize>)> {
		self.current_path = Vec::new();
		self.redos.pop()
	}

	// Adds a state *before* changes
	pub fn add_undo(&mut self, layer: aet::AetLayerNode, path: Vec<usize>) {
		self.undos.push_back((layer, path));
		if self.undos.len() > 100 {
			self.undos.pop_front();
		}
		self.redos.clear();
		self.flux = None;
	}

	pub fn add_redo(&mut self, layer: aet::AetLayerNode, path: Vec<usize>) {
		self.redos.push((layer, path));
		self.flux = None;
	}

	pub fn feed_state(&mut self, current_time: f64, selected: &[usize], set: &aet::AetSetNode) {
		if selected.len() < 3 || selected[0] != 0 {
			return;
		}
		let scene = &set.scenes[selected[1]];

		let layer =
			selected
				.iter()
				.skip(3)
				.fold(scene.root.layers[selected[2]].clone(), |layer, i| {
					let layer = layer.try_lock().unwrap();
					let aet::AetItemNode::Comp(comp) = &layer.item else {
						panic!();
					};

					comp.layers[*i].clone()
				});
		let layer = layer.try_lock().unwrap();

		if selected == self.current_path {
			if let Some((time, last_update)) = &mut self.flux {
				if *last_update != *layer {
					*time = current_time;
					*last_update = layer.clone();
				} else if current_time >= *time + 1.0 {
					self.add_undo(self.original_layer.clone(), self.current_path.clone());
					self.original_layer = layer.clone();
				}
			} else if self.original_layer != *layer {
				self.flux = Some((current_time, layer.clone()));
			}
		} else {
			if self.flux.is_some() {
				self.add_undo(self.original_layer.clone(), self.current_path.clone());
			}
			self.current_path = selected.to_vec();
			self.original_layer = layer.clone();
		}
	}

	pub fn feed_multi_select_state(
		&mut self,
		current_time: f64,
		selected: &[usize],
		set: &aet::AetSetNode,
	) {
		let scene = &set.scenes[selected[1]];
		let comp = selected.iter().skip(3).fold(scene.root.clone(), |comp, i| {
			let layer = comp.layers[*i].try_lock().unwrap();
			let aet::AetItemNode::Comp(comp) = &layer.item else {
				panic!();
			};
			comp.clone()
		});

		if selected == self.current_path {
			if let Some((time, last_update)) = &mut self.flux {
				let aet::AetItemNode::Comp(last_update) = &mut last_update.item else {
					panic!();
				};

				if !last_update.deep_eq(&comp) {
					*time = current_time;
					*last_update = comp.mid_clone();
				} else if current_time >= *time + 1.0 {
					self.add_undo(self.original_layer.clone(), self.current_path.clone());
					self.original_layer = aet::AetLayerNode::create_with_item(
						aet::AetItemNode::Comp(comp.mid_clone()),
					);
				}
			} else {
				let aet::AetItemNode::Comp(original_layer) = &self.original_layer.item else {
					panic!();
				};

				if !original_layer.deep_eq(&comp) {
					self.flux = Some((
						current_time,
						aet::AetLayerNode::create_with_item(aet::AetItemNode::Comp(
							comp.mid_clone(),
						)),
					));
				}
			}
		} else {
			if self.flux.is_some() {
				self.add_undo(self.original_layer.clone(), self.current_path.clone());
			}
			self.current_path = selected.to_vec();
			self.original_layer =
				aet::AetLayerNode::create_with_item(aet::AetItemNode::Comp(comp.mid_clone()));
		}
	}
}

pub struct App {
	aet_set: Option<aet::AetSetNode>,
	aet_set_filepath: Option<PathBuf>,
	aet_set_farc: Option<kkdlib::farc::Farc>,
	sprite_set: Option<spr::SpriteSetNode>,
	sprite_set_filepath: Option<PathBuf>,
	sprite_set_farc: Option<kkdlib::farc::Farc>,
	spr_db: Option<spr_db::SprDbNode>,
	spr_db_filepath: Option<PathBuf>,
	selected: Vec<usize>,
	multi_select: Vec<Rc<Mutex<aet::AetLayerNode>>>,

	modern_writing_modal: bool,
	help_modal: bool,

	undoer: LayerUndoer,
	copied_layer: Option<aet::AetLayerNode>,
	frametimes: VecDeque<(f64, f32)>,
}

impl App {
	pub fn new(cc: &eframe::CreationContext) -> Option<Self> {
		cc.egui_ctx.set_zoom_factor(1.2);
		cc.egui_ctx.set_theme(egui::Theme::Light);

		egui_material_icons::initialize(&cc.egui_ctx);
		cc.egui_ctx.style_mut(|style| {
			style.spacing.scroll = egui::style::ScrollStyle::solid();
			style.spacing.slider_width *= 2.0;
			style.visuals.striped = true;
			style.visuals.slider_trailing_fill = true;
			style.visuals.handle_shape = egui::style::HandleShape::Circle;
		});

		let wgpu_render_state = cc.wgpu_render_state.as_ref()?;
		txp::setup_wgpu(wgpu_render_state);

		Some(Self {
			aet_set: None,
			aet_set_filepath: None,
			aet_set_farc: None,
			sprite_set: None,
			sprite_set_filepath: None,
			sprite_set_farc: None,
			spr_db: None,
			spr_db_filepath: None,
			selected: Vec::new(),
			multi_select: Vec::new(),
			modern_writing_modal: false,
			help_modal: false,
			undoer: LayerUndoer::new(),
			copied_layer: None,
			frametimes: VecDeque::new(),
		})
	}
}

// Custom Selectable Label type Collapsing Header
pub fn collapsing_selectable_label<R>(
	ui: &mut egui::Ui,
	label: impl Into<egui::WidgetText>,
	id: impl std::hash::Hash,
	selected: bool,
	add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
	ui.vertical(|ui| {
		let id = ui.make_persistent_id(egui::Id::new(id));
		let button_padding = ui.spacing().button_padding;

		let available = ui.available_rect_before_wrap();
		let text_pos = available.min + egui::vec2(ui.spacing().indent, 0.0);
		let wrap_width = available.right() - text_pos.x;
		let galley = label.into().into_galley(
			ui,
			Some(egui::TextWrapMode::Extend),
			wrap_width,
			egui::TextStyle::Button,
		);
		let text_max_x = text_pos.x + galley.size().x;

		let desired_width = text_max_x + button_padding.x - available.left();
		let mut desired_size = egui::vec2(desired_width, galley.size().y + 2.0 * button_padding.y);
		desired_size = desired_size.at_least(ui.spacing().interact_size);
		let (_, rect) = ui.allocate_space(desired_size);

		let mut header_response = ui.interact(rect, id, egui::Sense::click());
		let text_pos = egui::pos2(
			text_pos.x,
			header_response.rect.center().y - galley.size().y / 2.0,
		);

		let mut state =
			egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);

		header_response.widget_info(|| {
			egui::WidgetInfo::labeled(
				egui::WidgetType::CollapsingHeader,
				ui.is_enabled(),
				galley.text(),
			)
		});

		let openness = state.openness(ui.ctx());

		if ui.is_rect_visible(rect) {
			let visuals = ui.style().interact_selectable(&header_response, selected);

			if selected || (header_response.hovered() || header_response.has_focus()) {
				let rect = rect.expand(visuals.expansion);

				ui.painter().rect(
					rect,
					visuals.corner_radius,
					visuals.bg_fill,
					visuals.bg_stroke,
					egui::StrokeKind::Inside,
				);
			}

			{
				let (_, mut icon_rect) = ui.spacing().icon_rectangles(header_response.rect);
				icon_rect.set_center(egui::pos2(
					header_response.rect.left() + ui.spacing().indent / 2.0,
					header_response.rect.center().y,
				));
				let icon_response = header_response.clone().with_new_rect(icon_rect);
				egui::collapsing_header::paint_default_icon(ui, openness, &icon_response);

				if ui
					.interact(icon_rect, id.with("Icon"), egui::Sense::click())
					.clicked() || (selected
					&& ui.memory(|mem| mem.focused().is_none())
					&& ui.input_mut(|i| {
						i.consume_key(
							egui::Modifiers::NONE,
							if state.is_open() {
								egui::Key::ArrowLeft
							} else {
								egui::Key::ArrowRight
							},
						)
					})) {
					state.toggle(ui);
					header_response.mark_changed();
				}
			}

			ui.painter().galley(text_pos, galley, visuals.text_color());
		}

		let ret_response = state.show_body_indented(&header_response, ui, add_body);

		if let Some(ret_response) = ret_response {
			egui::CollapsingResponse {
				header_response,
				body_response: Some(ret_response.response),
				body_returned: Some(ret_response.inner),
				openness,
			}
		} else {
			egui::CollapsingResponse {
				header_response,
				body_response: None,
				body_returned: None,
				openness,
			}
		}
	})
	.inner
}

// Based on DragValue
pub fn num_edit<Num: egui::emath::Numeric + std::str::FromStr + std::fmt::Display>(
	ui: &mut egui::Ui,
	value: &mut Num,
	max_decimals: usize,
) -> egui::Response {
	ui.horizontal(|ui| {
		let id = ui.next_auto_id();
		let is_editing = ui.is_enabled()
			&& ui.memory_mut(|mem| {
				mem.interested_in_focus(id, ui.layer_id());
				mem.has_focus(id)
			});

		if ui.memory_mut(|mem| !mem.had_focus_last_frame(id) && mem.has_focus(id)) {
			ui.data_mut(|data| data.remove::<String>(id));
		}

		if ui.memory(|mem| !mem.has_focus(id) && mem.had_focus_last_frame(id))
			&& !ui.input(|i| i.key_pressed(egui::Key::Escape))
		{
			ui.data_mut(|data| data.remove::<String>(id));
		}

		let value_text = format!("{:.*}", max_decimals, *value);
		if is_editing {
			let mut value_text = ui
				.data_mut(|data| data.remove_temp::<String>(id))
				.unwrap_or(value_text);
			let response = ui.add(
				egui::TextEdit::singleline(&mut value_text)
					.clip_text(false)
					.horizontal_align(ui.layout().horizontal_align())
					.vertical_align(ui.layout().vertical_align())
					.margin(ui.spacing().button_padding)
					.min_size(ui.spacing().interact_size)
					.id(id)
					.desired_width(
						ui.spacing().interact_size.x - 2.0 * ui.spacing().button_padding.x,
					)
					.font(ui.style().drag_value_text_style.clone()),
			);

			if ui.memory_mut(|mem| !mem.had_focus_last_frame(id) && mem.has_focus(id)) {
				let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
				state
					.cursor
					.set_char_range(Some(egui::text::CCursorRange::two(
						egui::text::CCursor::default(),
						egui::text::CCursor::new(value_text.chars().count()),
					)));
				state.store(ui.ctx(), response.id);
			}

			if response.changed()
				&& let Ok(parsed_value) = value_text.parse()
			{
				*value = parsed_value;
			}
			ui.data_mut(|data| data.insert_temp(id, value_text));

			response
		} else {
			let button = egui::Button::new(
				egui::RichText::new(&value_text)
					.text_style(ui.style().drag_value_text_style.clone()),
			)
			.wrap_mode(egui::TextWrapMode::Extend)
			.sense(egui::Sense::click())
			.min_size(ui.spacing().interact_size);

			let response = ui.add(button);

			if response.clicked() {
				ui.data_mut(|data| data.remove::<String>(id));
				ui.memory_mut(|mem| mem.request_focus(id));

				let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
				state
					.cursor
					.set_char_range(Some(egui::text::CCursorRange::two(
						egui::text::CCursor::default(),
						egui::text::CCursor::new(value_text.chars().count()),
					)));
				state.store(ui.ctx(), response.id);
			}

			response
		}
	})
	.inner
}

pub fn show_node(
	ui: &mut egui::Ui,
	node: &mut dyn TreeNode,
	index: usize,
	path: &[usize],
	selected: &mut Vec<usize>,
	frame: &mut eframe::Frame,
	undoer: &mut LayerUndoer,
	children: &mut Vec<(Vec<usize>, egui::Response)>,
) -> egui::Response {
	let mut path = path.to_vec();
	path.push(index);

	let child_index = children.len();
	children.push((path.clone(), ui.response()));

	let resp = if node.has_custom_tree() {
		node.display_tree(ui, &path, selected, frame, undoer, children)
	} else if node.has_children() {
		let resp = ui
			.horizontal(|ui| {
				node.label_sameline(ui);

				collapsing_selectable_label(
					ui,
					node.label().to_string(),
					&path,
					path == *selected,
					|ui| {
						let mut index = 0;
						node.display_children(&mut |child| {
							show_node(ui, child, index, &path, selected, frame, undoer, children);
							index += 1;
						});
					},
				)
			})
			.inner
			.header_response;

		if node.has_context_menu() {
			let menu = egui::Popup::context_menu(&resp).show(|ui| node.display_ctx_menu(ui, frame));
			if menu.is_some() {
				node.selected(frame);
				*selected = path.clone();
			}
		}

		if resp.clicked() {
			node.selected(frame);
			*selected = path;
		}

		resp
	} else {
		let resp = ui
			.horizontal(|ui| {
				node.label_sameline(ui);
				ui.selectable_label(path == *selected, node.label())
			})
			.inner;

		if node.has_context_menu() {
			let menu = egui::Popup::context_menu(&resp).show(|ui| node.display_ctx_menu(ui, frame));

			if menu.is_some() {
				node.selected(frame);
				*selected = path.clone();
			}
		}

		if resp.clicked() {
			node.selected(frame);
			*selected = path;
		}

		resp
	};

	children[child_index].1 = resp.clone();
	resp
}

fn set_node_selected(
	node: &mut dyn TreeNode,
	index: usize,
	depth: usize,
	path: &[usize],
	desired_path: &[usize],
	frame: &mut eframe::Frame,
) {
	if depth == desired_path.len() - 1 {
		if desired_path[depth] == index {
			node.selected(frame);
		}
		return;
	}

	let desired_index = desired_path[depth + 1];
	let mut new_path = path.to_vec();
	new_path.push(index);

	let mut index = 0;
	node.display_children(&mut |child| {
		if index == desired_index {
			set_node_selected(child, index, depth + 1, &new_path, desired_path, frame);
		}
		index += 1;
	});
}

fn show_node_opts(
	ui: &mut egui::Ui,
	node: &mut dyn TreeNode,
	index: usize,
	depth: usize,
	path: &[usize],
	desired_path: &[usize],
	frame: &mut eframe::Frame,
) {
	if depth == desired_path.len() - 1 {
		if desired_path[depth] == index {
			node.display_opts(ui, frame);
		}
		return;
	}

	let desired_index = desired_path[depth + 1];
	let mut new_path = path.to_vec();
	new_path.push(index);

	let mut index = 0;
	node.display_children(&mut |child| {
		if index == desired_index {
			show_node_opts(ui, child, index, depth + 1, &new_path, desired_path, frame);
		}
		index += 1;
	});
}

fn show_node_visual(
	ui: &mut egui::Ui,
	node: &mut dyn TreeNode,
	index: usize,
	depth: usize,
	path: &[usize],
	desired_path: &[usize],
) {
	if desired_path.len() <= depth + 1 {
		return;
	}
	let desired_index = desired_path[depth + 1];
	let mut path = path.to_vec();
	path.push(index);

	let mut index = 0;
	node.display_children(&mut |child| {
		if index == desired_index {
			if depth + 1 == desired_path.len() - 1 {
				let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::empty());
				if let Some(callback) = child.display_visual(ui, rect) {
					ui.painter().add(callback);
				}
			} else {
				show_node_visual(ui, child, index, depth + 1, &path, desired_path);
			}
		}
		index += 1;
	});
}

impl App {
	fn get_active_scene(&mut self) -> Option<&mut aet::AetSceneNode> {
		let node = self.aet_set.as_mut()?;
		if self.selected.len() < 2 || self.selected[0] != 0 {
			return None;
		}
		node.scenes.get_mut(self.selected[1])
	}

	fn set_file(&mut self, frame: &mut eframe::Frame, path: &PathBuf, data: &[u8]) {
		let name = path
			.file_name()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default();
		let ext = path
			.extension()
			.unwrap_or_default()
			.to_str()
			.unwrap_or_default();

		if (name.starts_with("aet_") && ext == "bin") || ext == "aec" {
			let aet_set = aet::AetSetNode::read(name, data);
			if aet_set.modern {
				self.modern_writing_modal = true;
			}
			self.aet_set = Some(aet_set);
			self.aet_set_filepath = Some(path.clone());
			self.spr_db = None;
			self.sprite_set = None;
			self.undoer = LayerUndoer::new();
		} else if (name.starts_with("spr") && ext == "bin") || ext == "spr" {
			let spr_set = spr::SpriteSetNode::read(name, data);
			if spr_set.modern {
				self.modern_writing_modal = true;
			}
			spr_set.init_wgpu(frame);

			if let Some(aet_set) = &mut self.aet_set
				&& let Some(spr_db) = &self.spr_db
			{
				for scene in &mut aet_set.scenes {
					scene.root.update_video_textures(spr_db, &spr_set);
				}
			}

			self.sprite_set = Some(spr_set);
			self.sprite_set_filepath = Some(path.clone());
		} else if ext == "farc" {
			let farc = kkdlib::farc::Farc::from_buf(data, true);
			let mut spr_set_farc = false;
			let mut aet_set_farc = false;
			for file in farc.files() {
				if (name.starts_with("spr") && ext == "bin") || ext == "spr" {
					let spr_set =
						spr::SpriteSetNode::read(&file.name(), file.data().unwrap_or_default());
					if spr_set.modern {
						self.modern_writing_modal = true;
					}
					spr_set.init_wgpu(frame);

					if let Some(aet_set) = &mut self.aet_set
						&& let Some(spr_db) = &self.spr_db
					{
						for scene in &mut aet_set.scenes {
							scene.root.update_video_textures(spr_db, &spr_set);
						}
					}

					self.sprite_set = Some(spr_set);
					self.sprite_set_filepath = Some(path.clone());
					spr_set_farc = true;
				} else if (name.starts_with("aet_") && ext == "bin") || ext == "aec" {
					let aet_set =
						aet::AetSetNode::read(&file.name(), file.data().unwrap_or_default());
					if aet_set.modern {
						self.modern_writing_modal = true;
					}
					self.aet_set = Some(aet_set);
					self.aet_set_filepath = Some(path.clone());
					self.spr_db = None;
					self.sprite_set = None;
					self.undoer = LayerUndoer::new();
					aet_set_farc = true;
				} else if name.ends_with("spr_db.bin") || ext == "spi" {
					self.spr_db = Some(spr_db::SprDbNode::read(
						&file.name(),
						file.data().unwrap_or_default(),
					));
					self.spr_db_filepath = Some(path.clone());
				}
			}

			if spr_set_farc {
				self.sprite_set_farc = Some(farc);
			} else if aet_set_farc {
				self.aet_set_farc = Some(farc);
			}
		} else if name.ends_with("spr_db.bin") || ext == "spi" {
			self.spr_db = Some(spr_db::SprDbNode::read(name, data));
			self.spr_db_filepath = Some(path.clone());
		}

		self.selected = Vec::new();

		if let Some(path) = path.parent()
			&& let Ok(dir) = path.read_dir()
		{
			if self.aet_set.is_some() && self.spr_db.is_none() {
				for file in dir {
					let Ok(file) = file else {
						continue;
					};
					let name = file.file_name().to_string_lossy().to_string();
					if (name.ends_with("spr_db.bin") || ext == "spi")
						&& let Ok(data) = std::fs::read(file.path())
					{
						self.spr_db = Some(spr_db::SprDbNode::read(&name, &data));
						self.spr_db_filepath = Some(file.path());
						break;
					}
				}
			}

			if let Some(aet_set) = &mut self.aet_set
				&& self.spr_db.is_none()
				&& self.sprite_set.is_none()
			{
				// spr db not in folder, modern?
				let desired_name = aet_set
					.name
					.replace("aet_", "spr_")
					.replace(".aec", ".farc");
				for file in path.read_dir().unwrap() {
					let Ok(file) = file else {
						continue;
					};
					let name = file.file_name().to_string_lossy().to_string();
					if name == desired_name
						&& let Ok(data) = std::fs::read(file.path())
					{
						let farc = kkdlib::farc::Farc::from_buf(&data, true);
						let Some(spr_db) = farc.get_file(&desired_name.replace(".farc", ".spi"))
						else {
							continue;
						};

						let Some(spr_set) = farc.get_file(&desired_name.replace(".farc", ".spr"))
						else {
							continue;
						};

						let spr_db = spr_db::SprDbNode::read(
							&spr_db.name(),
							spr_db.data().unwrap_or_default(),
						);
						let mut spr_set = spr::SpriteSetNode::read(
							&spr_set.name(),
							spr_set.data().unwrap_or_default(),
						);

						if spr_set.modern {
							self.modern_writing_modal = true;
						}

						spr_set.init_wgpu(frame);
						spr_set.add_db(spr_db.sets.first().unwrap().clone());

						for scene in &mut aet_set.scenes {
							scene.root.update_video_textures(&spr_db, &spr_set);
						}

						self.sprite_set = Some(spr_set);
						self.sprite_set_filepath = Some(file.path());

						self.spr_db = Some(spr_db);
						self.spr_db_filepath = Some(file.path());

						self.sprite_set_farc = Some(farc);

						break;
					}
				}
			} else if let Some(aet_set) = &mut self.aet_set
				&& let Some(spr_db) = &self.spr_db
				&& let Some(scene) = aet_set.scenes.first()
				&& let Some(sprite_id) = scene.root.get_sprite_id()
				&& let Some(db_set) = spr_db.sets.iter().find(|set| {
					set.try_lock()
						.unwrap()
						.entries
						.iter()
						.any(|entry| entry.try_lock().unwrap().id == sprite_id)
				}) && self.sprite_set.is_none()
			{
				let set = db_set.try_lock().unwrap();
				let set_name = set.file_name.clone();
				drop(set);

				let set_farc_name = set_name.replace(".bin", ".farc");
				for file in path.read_dir().unwrap() {
					let Ok(file) = file else {
						continue;
					};
					let file_name = file.file_name().to_string_lossy().to_string();
					if file_name == set_name
						&& let Ok(data) = std::fs::read(file.path())
					{
						let mut spr_set = spr::SpriteSetNode::read(name, &data);
						spr_set.init_wgpu(frame);
						spr_set.add_db(db_set.clone());

						for scene in &mut aet_set.scenes {
							scene.root.update_video_textures(spr_db, &spr_set);
						}

						self.sprite_set = Some(spr_set);
						self.sprite_set_filepath = Some(file.path());
						break;
					} else if file_name == set_farc_name
						&& let Ok(data) = std::fs::read(file.path())
					{
						let farc = kkdlib::farc::Farc::from_buf(&data, true);
						for farc_file in farc.files() {
							if farc_file.name() == set_name {
								let mut spr_set = spr::SpriteSetNode::read(
									&farc_file.name(),
									farc_file.data().unwrap(),
								);
								spr_set.init_wgpu(frame);
								spr_set.add_db(db_set.clone());

								for scene in &mut aet_set.scenes {
									scene.root.update_video_textures(spr_db, &spr_set);
								}

								self.sprite_set = Some(spr_set);
								self.sprite_set_filepath = Some(file.path());
								self.sprite_set_farc = Some(farc);

								break;
							}
						}
					}
				}
			}
		}
	}

	// Native only
	fn save_files(&self) {
		if let Some(aet_set) = &self.aet_set
			&& let Some(path) = &self.aet_set_filepath
		{
			if let Some(farc) = &self.aet_set_farc {
				let mut new_farc = kkdlib::farc::Farc::new();
				new_farc.set_flags(farc.flags());
				new_farc.set_signature(farc.signature());
				new_farc.set_compression_level(farc.compression_level());
				new_farc.set_alignment(farc.alignment());
				new_farc.set_ft(farc.ft());
				for file in farc.files() {
					if file.name() != aet_set.name {
						new_farc.add_file_data(&file.name(), file.data().unwrap_or_default());
					}
				}

				new_farc.add_file_data(&aet_set.name, &aet_set.raw_data());
				_ = std::fs::write(path, new_farc.to_buf().unwrap_or_default());
			} else {
				let data = aet_set.raw_data();
				_ = std::fs::write(path, &data);
			}
		}

		if let Some(sprite_set) = &self.sprite_set
			&& let Some(path) = &self.sprite_set_filepath
		{
			if let Some(farc) = &self.sprite_set_farc {
				let adding_spr_db = self.spr_db.as_ref().is_some_and(|spr_db| spr_db.modern);

				let mut new_farc = kkdlib::farc::Farc::new();
				new_farc.set_flags(farc.flags());
				new_farc.set_signature(farc.signature());
				new_farc.set_compression_level(farc.compression_level());
				new_farc.set_alignment(farc.alignment());
				new_farc.set_ft(farc.ft());
				for file in farc.files() {
					if file.name() != sprite_set.name
						&& (!adding_spr_db
							|| self.spr_db.as_ref().unwrap().filename != sprite_set.name)
					{
						new_farc.add_file_data(&file.name(), file.data().unwrap_or_default());
					}
				}

				new_farc.add_file_data(&sprite_set.name, &sprite_set.raw_data());
				if adding_spr_db && let Some(spr_db) = &self.spr_db {
					new_farc.add_file_data(&spr_db.filename, &spr_db.raw_data());
				}

				_ = std::fs::write(path, new_farc.to_buf().unwrap_or_default());
			} else {
				let data = sprite_set.raw_data();
				_ = std::fs::write(path, &data);
			}
		}

		if let Some(spr_db) = &self.spr_db
			&& let Some(path) = &self.spr_db_filepath
			&& (!spr_db.modern && self.sprite_set_farc.is_some())
		{
			let data = spr_db.raw_data();
			_ = std::fs::write(path, &data);
		}
	}

	// Native only
	fn save_files_to(&mut self) {
		let aet_set = if let Some(aet_set) = &self.aet_set {
			if let Some(farc) = &self.aet_set_farc
				&& let Some(path) = &self.aet_set_filepath
			{
				let mut new_farc = kkdlib::farc::Farc::new();
				new_farc.set_flags(farc.flags());
				new_farc.set_signature(farc.signature());
				new_farc.set_compression_level(farc.compression_level());
				new_farc.set_alignment(farc.alignment());
				new_farc.set_ft(farc.ft());
				for file in farc.files() {
					if file.name() != aet_set.name {
						new_farc.add_file_data(&file.name(), file.data().unwrap_or_default());
					}
				}

				new_farc.add_file_data(&aet_set.name, &aet_set.raw_data());
				let name = path.file_name().unwrap().to_string_lossy().to_string();
				Some((new_farc.to_buf().unwrap_or_default(), name))
			} else {
				Some((aet_set.raw_data(), aet_set.name.clone()))
			}
		} else {
			None
		};

		let sprite_set = if let Some(sprite_set) = &self.sprite_set {
			if let Some(farc) = &self.sprite_set_farc
				&& let Some(path) = &self.sprite_set_filepath
			{
				let adding_spr_db = self.spr_db.as_ref().is_some_and(|spr_db| spr_db.modern);

				let mut new_farc = kkdlib::farc::Farc::new();
				new_farc.set_flags(farc.flags());
				new_farc.set_signature(farc.signature());
				new_farc.set_compression_level(farc.compression_level());
				new_farc.set_alignment(farc.alignment());
				new_farc.set_ft(farc.ft());
				for file in farc.files() {
					if file.name() != sprite_set.name
						&& (!adding_spr_db
							|| self.spr_db.as_ref().unwrap().filename != sprite_set.name)
					{
						new_farc.add_file_data(&file.name(), file.data().unwrap_or_default());
					}
				}

				new_farc.add_file_data(&sprite_set.name, &sprite_set.raw_data());
				if adding_spr_db && let Some(spr_db) = &self.spr_db {
					new_farc.add_file_data(&spr_db.filename, &spr_db.raw_data());
				}

				let name = path.file_name().unwrap().to_string_lossy().to_string();
				Some((new_farc.to_buf().unwrap_or_default(), name))
			} else {
				Some((sprite_set.raw_data(), sprite_set.name.clone()))
			}
		} else {
			None
		};

		let spr_db = if let Some(spr_db) = &self.spr_db
			&& (!spr_db.modern && sprite_set.is_some())
		{
			Some((spr_db.raw_data(), spr_db.filename.clone()))
		} else {
			None
		};

		async {
			let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
				return;
			};

			let path = folder.path();
			if let Some((aet_set, name)) = aet_set {
				self.aet_set_filepath = Some(path.join(&name));
				std::fs::write(path.join(name), aet_set).unwrap();
			}
			if let Some((sprite_set, name)) = sprite_set {
				self.sprite_set_filepath = Some(path.join(&name));
				std::fs::write(path.join(name), sprite_set).unwrap();
			}
			if let Some((spr_db, name)) = spr_db {
				self.spr_db_filepath = Some(path.join(&name));
				std::fs::write(path.join(name), spr_db).unwrap();
			}
		}
		.block_on();
	}
}

fn get_selected_layer(aet_set: &aet::AetSetNode, path: &[usize]) -> Rc<Mutex<aet::AetLayerNode>> {
	path.iter().skip(3).fold(
		aet_set.scenes[path[1]].root.layers[path[2]].clone(),
		|layer, i| {
			let layer = layer.try_lock().unwrap();
			let aet::AetItemNode::Comp(comp) = &layer.item else {
				panic!();
			};

			comp.layers[*i].clone()
		},
	)
}

fn apply_redo(aet_set: &mut aet::AetSetNode, undoer: &mut LayerUndoer) {
	let Some((undone, path)) = undoer.redo() else {
		return;
	};
	if path.len() == 2 {
		let aet::AetItemNode::Comp(comp) = undone.item else {
			panic!()
		};

		for layer in &comp.layers {
			let mut layer = layer.try_lock().unwrap();
			layer.multi_selected = false;
			layer.want_deletion = false;
			layer.want_duplicate = false;
		}

		undoer.add_undo(
			aet::AetLayerNode::create_with_item(aet::AetItemNode::Comp(
				aet_set.scenes[path[1]].root.clone(),
			)),
			path.clone(),
		);

		aet_set.scenes[path[1]].root = comp;
	} else {
		let layer = get_selected_layer(aet_set, &path);

		if let aet::AetItemNode::Comp(comp) = &undone.item {
			for layer in &comp.layers {
				let mut layer = layer.try_lock().unwrap();
				layer.multi_selected = false;
				layer.want_deletion = false;
				layer.want_duplicate = false;
			}
		}

		let mut layer = layer.try_lock().unwrap();
		undoer.add_undo(layer.clone(), path);
		*layer = undone;
	}
}

fn apply_undo(aet_set: &mut aet::AetSetNode, undoer: &mut LayerUndoer) {
	let Some((undone, path)) = undoer.undo() else {
		return;
	};
	if path.len() == 2 {
		let aet::AetItemNode::Comp(comp) = undone.item else {
			panic!()
		};

		for layer in &comp.layers {
			let mut layer = layer.try_lock().unwrap();
			layer.multi_selected = false;
			layer.want_deletion = false;
			layer.want_duplicate = false;
		}

		undoer.add_redo(
			aet::AetLayerNode::create_with_item(aet::AetItemNode::Comp(
				aet_set.scenes[path[1]].root.clone(),
			)),
			path.clone(),
		);

		aet_set.scenes[path[1]].root = comp;
	} else {
		let layer = get_selected_layer(aet_set, &path);

		if let aet::AetItemNode::Comp(comp) = &undone.item {
			for layer in &comp.layers {
				let mut layer = layer.try_lock().unwrap();
				layer.multi_selected = false;
				layer.want_deletion = false;
				layer.want_duplicate = false;
			}
		}

		let mut layer = layer.try_lock().unwrap();
		undoer.add_redo(layer.clone(), path);
		*layer = undone;
	}
}

const OPEN_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::O,
};

const SAVE_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::S,
};

const SAVE_TO_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
	logical_key: egui::Key::S,
};

const CLOSE_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::W,
};

const UNDO_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::Z,
};

const REDO_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::Y,
};

pub const EXPORT_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::E,
};

pub const REPLACE_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::R,
};

impl eframe::App for App {
	fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
		if let Some(cpu_usage) = frame.info().cpu_usage {
			let (time, dt) = ctx.input(|i| (i.time, i.unstable_dt as f64));
			self.frametimes.push_back((time - dt, cpu_usage * 1000.0));
			self.frametimes.retain(|(t, _)| time - t < 1.0);
		}

		if ctx.memory(|mem| mem.focused().is_none()) {
			ctx.input_mut(|input| {
				for file in &input.raw.dropped_files {
					if let Some(path) = &file.path
						&& path.is_file() && let Ok(data) = std::fs::read(path)
					{
						self.set_file(frame, path, &data);
					}
				}

				if input.consume_shortcut(&OPEN_SHORTCUT) {
					async {
						let Some(file) = rfd::AsyncFileDialog::new()
							.add_filter("DIVA", &["farc", "bin"])
							.pick_file()
							.await
						else {
							return;
						};

						self.selected = Vec::new();
						self.set_file(frame, &file.path().to_path_buf(), &file.read().await);
					}
					.block_on();
				}

				if input.consume_shortcut(&SAVE_TO_SHORTCUT) {
					self.save_files_to();
				}

				if input.consume_shortcut(&SAVE_SHORTCUT) {
					self.save_files();
				}

				if input.consume_shortcut(&CLOSE_SHORTCUT) {
					self.aet_set = None;
					self.aet_set_filepath = None;
					self.sprite_set = None;
					self.sprite_set_filepath = None;
					self.spr_db = None;
					self.spr_db_filepath = None;
					self.selected = Vec::new();
				}

				if let Some(aet_set) = &mut self.aet_set {
					if self.undoer.has_undo() && input.consume_shortcut(&UNDO_SHORTCUT) {
						apply_undo(aet_set, &mut self.undoer);

						if let Some(spr_db) = &self.spr_db
							&& let Some(spr_set) = &self.sprite_set
						{
							for scene in &mut aet_set.scenes {
								scene.root.update_video_textures(spr_db, spr_set);
							}
						}

						self.multi_select.clear();
					}

					if self.undoer.has_redo() && input.consume_shortcut(&REDO_SHORTCUT) {
						apply_redo(aet_set, &mut self.undoer);

						if let Some(spr_db) = &self.spr_db
							&& let Some(spr_set) = &self.sprite_set
						{
							for scene in &mut aet_set.scenes {
								scene.root.update_video_textures(spr_db, spr_set);
							}
						}

						self.multi_select.clear();
					}

					if !self.multi_select.is_empty() {
						if input.key_pressed(egui::Key::Delete)
							|| self
								.multi_select
								.iter()
								.any(|layer| layer.try_lock().unwrap().want_deletion)
						{
							for layer in &mut self.multi_select {
								layer.try_lock().unwrap().want_deletion = true;
							}
							self.multi_select.clear();
						}
					} else if self.selected.len() >= 3
						&& self.selected[0] == 0
						&& input.key_pressed(egui::Key::Delete)
					{
						let layer = get_selected_layer(aet_set, &self.selected);
						layer.try_lock().unwrap().want_deletion = true;
					}

					if self.selected.len() >= 3 && self.selected[0] == 0 {
						let selected = get_selected_layer(aet_set, &self.selected);
						if input.events.iter().any(|e| matches!(e, egui::Event::Copy)) {
							self.copied_layer = Some(selected.try_lock().unwrap().clone());
						} else if input.events.iter().any(|e| match e {
							egui::Event::Key {
								key,
								physical_key: _,
								pressed: _,
								repeat: false,
								modifiers,
							} => {
								*key == egui::Key::V
									&& modifiers.matches_exact(egui::Modifiers::COMMAND)
							}
							_ => false,
						}) && let Some(copied_layer) = &self.copied_layer
						{
							*selected.try_lock().unwrap() = copied_layer.deep_clone();
						} else if input.events.iter().any(|e| match e {
							egui::Event::Key {
								key,
								physical_key: _,
								pressed: _,
								repeat: false,
								modifiers,
							} => {
								*key == egui::Key::V
									&& modifiers.matches_exact(
										egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
									)
							}
							_ => false,
						}) && let Some(copied_layer) = &self.copied_layer
							&& let aet::AetItemNode::Comp(comp) =
								&mut selected.try_lock().unwrap().item
						{
							comp.layers
								.push(Rc::new(Mutex::new(copied_layer.deep_clone())));
						}
					}
				}
			});
		}

		if self.modern_writing_modal {
			let modal = egui::Modal::new(egui::Id::new("ModernWritingModal")).show(ctx, |ui| {
				ui.vertical_centered(|ui| {
					ui.label(
						egui::RichText::new(
							"KKdLib currently does not support writing modern files",
						)
						.size(20.0)
						.color(egui::Color32::RED),
					);
					if ui.button("Close").clicked() {
						ui.close();
					}
				});
			});

			if modal.should_close() {
				self.modern_writing_modal = false;
			}
		}

		if self.help_modal {
			let modal = egui::Modal::new(egui::Id::new("HelpModal")).show(ctx, |ui| {
				ui.vertical_centered(|ui| {
					ui.label(egui::RichText::new("Shortcuts").size(20.0));
				});

				let height = ui.text_style_height(&egui::TextStyle::Body);
				egui_extras::TableBuilder::new(ui)
					.column(egui_extras::Column::remainder())
					.column(egui_extras::Column::remainder())
					.body(|mut body| {
						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Open file");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&OPEN_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Save file");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&SAVE_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Save file to");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&SAVE_TO_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Close file");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&CLOSE_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Undo");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&UNDO_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Redo");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&REDO_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Export");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&EXPORT_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Replace");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&REPLACE_SHORTCUT));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Copy");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&egui::KeyboardShortcut {
									modifiers: egui::Modifiers::COMMAND,
									logical_key: egui::Key::C,
								}));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Paste");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&egui::KeyboardShortcut {
									modifiers: egui::Modifiers::COMMAND,
									logical_key: egui::Key::V,
								}));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Paste into");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&egui::KeyboardShortcut {
									modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
									logical_key: egui::Key::V,
								}));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Insert keyframe");
							});
							row.col(|ui| {
								ui.label(ctx.format_shortcut(&egui::KeyboardShortcut {
									modifiers: egui::Modifiers::COMMAND,
									logical_key: egui::Key::I,
								}));
							});
						});

						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Only move current keyframe");
							});
							row.col(|ui| {
								ui.label(ctx.format_modifiers(egui::Modifiers::CTRL));
							});
						});
					});

				ui.vertical_centered(|ui| {
					if ui.button("Close").clicked() {
						ui.close();
					}
				});
			});

			if modal.should_close() {
				self.help_modal = false;
			}
		}

		if let Some(aet_set) = &self.aet_set {
			if self.multi_select.is_empty() {
				self.undoer
					.feed_state(ctx.input(|input| input.time), &self.selected, aet_set);
			} else {
				let mut path = self.selected.clone();
				path.pop();
				self.undoer
					.feed_multi_select_state(ctx.input(|input| input.time), &path, aet_set);
			}
		}

		egui::TopBottomPanel::top("MenuBar").show(ctx, |ui| {
			egui::MenuBar::new().ui(ui, |ui| {
				ui.menu_button("File", |ui| {
					if ui
						.add(
							egui::Button::new("Open")
								.shortcut_text(ctx.format_shortcut(&OPEN_SHORTCUT)),
						)
						.clicked()
					{
						ui.close();
						async {
							let Some(file) = rfd::AsyncFileDialog::new()
								.add_filter("DIVA", &["farc", "bin"])
								.pick_file()
								.await
							else {
								return;
							};

							self.selected = Vec::new();
							self.set_file(frame, &file.path().to_path_buf(), &file.read().await);
						}
						.block_on();
					}

					if ui
						.add_enabled(
							self.aet_set.is_some()
								|| self.sprite_set.is_some()
								|| self.spr_db.is_some(),
							egui::Button::new("Save")
								.shortcut_text(ctx.format_shortcut(&SAVE_SHORTCUT)),
						)
						.clicked()
					{
						self.save_files();
					}

					if ui
						.add_enabled(
							self.aet_set.is_some()
								|| self.sprite_set.is_some()
								|| self.spr_db.is_some(),
							egui::Button::new("Save To")
								.shortcut_text(ctx.format_shortcut(&SAVE_TO_SHORTCUT)),
						)
						.clicked()
					{
						self.save_files_to();
					}

					if ui
						.add_enabled(
							self.aet_set.is_some()
								|| self.sprite_set.is_some()
								|| self.spr_db.is_some(),
							egui::Button::new("Close")
								.shortcut_text(ctx.format_shortcut(&CLOSE_SHORTCUT)),
						)
						.clicked()
					{
						self.aet_set = None;
						self.aet_set_filepath = None;
						self.sprite_set = None;
						self.sprite_set_filepath = None;
						self.spr_db = None;
						self.spr_db_filepath = None;
						self.selected = Vec::new();
					}
				});

				ui.menu_button("Edit", |ui| {
					if let Some(aet_set) = &mut self.aet_set {
						if ui
							.add_enabled(
								self.undoer.has_undo(),
								egui::Button::new("Undo")
									.shortcut_text(ctx.format_shortcut(&UNDO_SHORTCUT)),
							)
							.clicked()
						{
							apply_undo(aet_set, &mut self.undoer);

							if let Some(spr_db) = &self.spr_db
								&& let Some(spr_set) = &self.sprite_set
							{
								for scene in &mut aet_set.scenes {
									scene.root.update_video_textures(spr_db, spr_set);
								}
							}

							self.multi_select.clear();
						}

						if ui
							.add_enabled(
								self.undoer.has_redo(),
								egui::Button::new("Redo")
									.shortcut_text(ctx.format_shortcut(&REDO_SHORTCUT)),
							)
							.clicked()
						{
							apply_redo(aet_set, &mut self.undoer);

							if let Some(spr_db) = &self.spr_db
								&& let Some(spr_set) = &self.sprite_set
							{
								for scene in &mut aet_set.scenes {
									scene.root.update_video_textures(spr_db, spr_set);
								}
							}

							self.multi_select.clear();
						}
					} else {
						ui.add_enabled(
							false,
							egui::Button::new("Undo")
								.shortcut_text(ctx.format_shortcut(&UNDO_SHORTCUT)),
						);
						ui.add_enabled(
							false,
							egui::Button::new("Redo")
								.shortcut_text(ctx.format_shortcut(&REDO_SHORTCUT)),
						);
					}
				});

				if ui.button("Help").clicked() {
					self.help_modal = true;
				}

				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					ui.label(format!(
						"{:.2}ms",
						self.frametimes
							.iter()
							.map(|(_, frametime)| *frametime)
							.fold(0.0, |acc, frametime| acc + frametime)
							/ self.frametimes.len() as f32
					));
				});
			});
		});

		egui::SidePanel::right("RightSidePanel")
			.resizable(true)
			.show(ctx, |ui| {
				if !self.selected.is_empty() && self.multi_select.is_empty() {
					egui::TopBottomPanel::bottom("NodeOptions")
						.resizable(true)
						.show_inside(ui, |ui| {
							if let Some(node) = &mut self.aet_set
								&& self.selected[0] == 0
							{
								show_node_opts(ui, node, 0, 0, &[], &self.selected, frame);
							}
							if let Some(node) = &mut self.sprite_set
								&& self.selected[0] == 1
							{
								show_node_opts(ui, node, 1, 0, &[], &self.selected, frame);
							}
							if let Some(node) = &mut self.spr_db
								&& self.selected[0] == 2
							{
								show_node_opts(ui, node, 2, 0, &[], &self.selected, frame);
							}

							ui.take_available_space();
						});
				}

				egui::ScrollArea::vertical().show(ui, |ui| {
					let mut children = Vec::new();
					if let Some(node) = &mut self.aet_set {
						let old_selected = self.selected.clone();

						show_node(
							ui,
							node,
							0,
							&[],
							&mut self.selected,
							frame,
							&mut self.undoer,
							&mut children,
						);

						if self.selected.len() >= 3
							&& old_selected.len() >= 3
							&& old_selected[0] == 0
							&& self.selected != old_selected
							&& self
								.selected
								.iter()
								.rev()
								.skip(1)
								.zip(old_selected.iter().rev().skip(1))
								.all(|(new, old)| new == old)
							&& ui.ctx().input(|i| {
								(i.modifiers.ctrl
									|| (self.selected[self.selected.len() - 2]
										== old_selected[old_selected.len() - 2]
										&& i.modifiers.shift)) && i.pointer.primary_clicked()
									&& ui.max_rect().contains(i.pointer.interact_pos().unwrap())
							}) {
							if self.multi_select.is_empty() {
								let old_layer = get_selected_layer(node, &old_selected);
								old_layer.try_lock().unwrap().multi_selected = true;
								self.multi_select.push(old_layer);
							}

							if ui.ctx().input(|i| i.modifiers.shift) {
								let diff = self.selected[self.selected.len() - 1] as isize
									- old_selected[old_selected.len() - 1] as isize;
								if diff.is_positive() {
									for i in 1..=diff {
										let mut path = old_selected.clone();
										path[old_selected.len() - 1] += i as usize;
										let new_layer = get_selected_layer(node, &path);
										if !new_layer.try_lock().unwrap().multi_selected {
											new_layer.try_lock().unwrap().multi_selected = true;
											self.multi_select.push(new_layer);
										}
									}
								} else {
									for i in diff..0 {
										let mut path = old_selected.clone();
										path[old_selected.len() - 1] =
											(path[old_selected.len() - 1] as isize + i) as usize;
										let new_layer = get_selected_layer(node, &path);
										if !new_layer.try_lock().unwrap().multi_selected {
											new_layer.try_lock().unwrap().multi_selected = true;
											self.multi_select.push(new_layer);
										}
									}
								}
							} else {
								let new_layer = get_selected_layer(node, &self.selected);
								if !new_layer.try_lock().unwrap().multi_selected {
									new_layer.try_lock().unwrap().multi_selected = true;
									self.multi_select.push(new_layer);
								}
							}
						} else if ui.ctx().interaction_snapshot(|i| i.clicked.is_some())
							&& ui.ctx().input(|i| {
								i.pointer.primary_clicked()
									&& ui.max_rect().contains(i.pointer.interact_pos().unwrap())
							}) && !ui.ctx().is_popup_open()
						{
							for layer in &mut self.multi_select {
								let mut layer = layer.try_lock().unwrap();
								layer.multi_selected = false;
							}
							self.multi_select.clear();
						}
					}

					if let Some(node) = &mut self.sprite_set {
						show_node(
							ui,
							node,
							1,
							&[],
							&mut self.selected,
							frame,
							&mut self.undoer,
							&mut children,
						);
					}
					if let Some(node) = &mut self.spr_db {
						show_node(
							ui,
							node,
							2,
							&[],
							&mut self.selected,
							frame,
							&mut self.undoer,
							&mut children,
						);
					}

					if !self.selected.is_empty()
						&& ui.memory(|mem| mem.focused().is_none())
						&& let Some(index) = children.iter().position(|(c, _)| c == &self.selected)
					{
						if index != 0
							&& ui.input_mut(|i| {
								i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
							}) {
							let (path, resp) = children[index - 1].clone();
							self.selected = path;
							resp.scroll_to_me(None);

							let root = if self.selected[0] == 0 {
								self.aet_set.as_mut().unwrap() as &mut dyn TreeNode
							} else if self.selected[0] == 1 {
								self.sprite_set.as_mut().unwrap() as &mut dyn TreeNode
							} else if self.selected[0] == 2 {
								self.spr_db.as_mut().unwrap() as &mut dyn TreeNode
							} else {
								unreachable!()
							};
							set_node_selected(
								root,
								self.selected[0],
								0,
								&[],
								&self.selected,
								frame,
							);
						} else if index != children.len() - 1
							&& ui.input_mut(|i| {
								i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
							}) {
							let (path, resp) = children[index + 1].clone();
							self.selected = path;
							resp.scroll_to_me(None);

							let root = if self.selected[0] == 0 {
								self.aet_set.as_mut().unwrap() as &mut dyn TreeNode
							} else if self.selected[0] == 1 {
								self.sprite_set.as_mut().unwrap() as &mut dyn TreeNode
							} else if self.selected[0] == 2 {
								self.spr_db.as_mut().unwrap() as &mut dyn TreeNode
							} else {
								unreachable!()
							};
							set_node_selected(
								root,
								self.selected[0],
								0,
								&[],
								&self.selected,
								frame,
							);
						}
					}

					ui.take_available_space();
				});

				ui.take_available_space();
			});

		egui::TopBottomPanel::bottom("CurveEditor")
			.resizable(true)
			.show(ctx, |ui| {
				if let Some(scene) = self.get_active_scene() {
					ui.horizontal(|ui| {
						if ui.ctx().memory(|memory| memory.focused().is_none())
							&& ui.input_mut(|input| {
								input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
							}) {
							scene.playing = !scene.playing;
						}

						static WIDTH: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
						let w = WIDTH.get_or_init(|| {
							ui.scope_builder(
								egui::UiBuilder::new().sizing_pass().invisible(),
								|ui| {
									let start = ui.available_width();

									_ = ui.selectable_label(false, ICON_PLAY_ARROW);
									ui.checkbox(
										&mut scene.display_placeholders,
										"Display placeholders",
									);
									ui.checkbox(&mut scene.centered, "Centered");
									ui.add(
										egui::Slider::new(
											&mut scene.current_time,
											scene.start_time..=scene.end_time,
										)
										.clamping(egui::SliderClamping::Edits)
										.max_decimals(0),
									);

									start - ui.available_width()
								},
							)
							.inner
						});

						let offset = ui.available_width() / 2.0 - w / 2.0;

						if offset > 0.0 {
							ui.allocate_space(egui::vec2(offset, 0.0));
						}

						let playback_icon = if scene.playing {
							ICON_PAUSE
						} else {
							ICON_PLAY_ARROW
						};
						if ui.selectable_label(false, playback_icon).clicked() {
							scene.playing = !scene.playing;
						}

						ui.checkbox(&mut scene.display_placeholders, "Display placeholders");
						ui.checkbox(&mut scene.centered, "Centered");
						ui.add(
							egui::Slider::new(
								&mut scene.current_time,
								scene.start_time..=scene.end_time,
							)
							.clamping(egui::SliderClamping::Edits)
							.max_decimals(0),
						);

						if scene.playing && scene.current_time < scene.end_time {
							ctx.input(|input| {
								scene.current_time += input.stable_dt * scene.fps;
							});
							ctx.request_repaint();
						}
					});

					ui.separator();
				}

				if let Some(node) = &mut self.aet_set
					&& self.selected.len() >= 2
					&& self.selected[0] == 0
					&& self.multi_select.is_empty()
					&& let Some(scene) = node.scenes.get_mut(self.selected[1])
				{
					scene.root.show_node_curve_editor(
						ui,
						&mut scene.selected_curve,
						scene.current_time,
						&[scene.width as f32, scene.height as f32],
						0,
						1,
						&[0, self.selected[1]],
						&self.selected,
					);
				}

				ui.take_available_space();
			});

		if let Some(spr_set) = &mut self.sprite_set {
			if spr_set.textures_node.children_changed
				|| spr_set
					.textures_node
					.children
					.iter()
					.any(|tex| tex.try_lock().unwrap().texture_updated)
			{
				spr_set.init_wgpu(frame);

				spr_set.textures_node.children_changed = false;
				for texture in &mut spr_set.textures_node.children {
					texture.try_lock().unwrap().texture_updated = false;
				}
			}

			if let Some(set) = &mut spr_set.db_set {
				let mut set = set.try_lock().unwrap();
				for (i, spr) in spr_set
					.sprites_node
					.children
					.try_lock()
					.unwrap()
					.iter_mut()
					.enumerate()
					.filter(|(_, spr)| spr.try_lock().unwrap().db_entry.is_none())
				{
					let mut spr = spr.try_lock().unwrap();
					let entry = Rc::new(Mutex::new(spr_db::SprDbEntryNode {
						id: 0,
						name: String::from("DUMMY"),
						index: i as u16,
						texture: false,
					}));

					spr.db_entry = Some(entry.clone());
					set.entries.push(entry);
				}

				for (i, tex) in spr_set
					.textures_node
					.children
					.iter_mut()
					.enumerate()
					.filter(|(_, tex)| tex.try_lock().unwrap().db_entry.is_none())
				{
					let mut tex = tex.try_lock().unwrap();
					let entry = Rc::new(Mutex::new(spr_db::SprDbEntryNode {
						id: 0,
						name: String::from("DUMMY"),
						index: i as u16,
						texture: true,
					}));

					tex.db_entry = Some(entry.clone());
					set.entries.push(entry);
				}
			}

			spr_set.update_db_entries();
		}

		egui::CentralPanel::default().show(ctx, |ui| {
			let has_multi_select = !self.multi_select.is_empty();
			if let Some(node) = self.aet_set.as_mut()
				&& self.selected.len() >= 2
				&& self.selected[0] == 0
				&& let Some(scene) = node.scenes.get_mut(self.selected[1])
			{
				let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::empty());
				let ar = rect.width() / rect.height();
				let rect = if ar > scene.width as f32 / scene.height as f32 {
					let adjusted_w = rect.height() / scene.height as f32 * scene.width as f32;
					let remaining_w = rect.width() - adjusted_w;
					egui::Rect {
						min: egui::Pos2 {
							x: rect.min.x + remaining_w / 2.0,
							y: rect.min.y,
						},
						max: egui::Pos2 {
							x: rect.min.x + adjusted_w + remaining_w / 2.0,
							y: rect.min.y + rect.height(),
						},
					}
				} else {
					let adjusted_h = rect.width() / scene.width as f32 * scene.height as f32;
					let remaining_h = rect.height() - adjusted_h;
					egui::Rect {
						min: egui::Pos2 {
							x: rect.min.x,
							y: rect.min.y + remaining_h / 2.0,
						},
						max: egui::Pos2 {
							x: rect.min.x + rect.width(),
							y: rect.min.y + adjusted_h + remaining_h / 2.0,
						},
					}
				};

				scene.display_visual(ui, rect, &mut self.selected);

				if has_multi_select {
					scene.gizmo.update_config(GizmoConfig {
						projection_matrix: [
							[2.0 / scene.width as f64, 0.0, 0.0, -1.0],
							[0.0, 2.0 / scene.height as f64, 0.0, -1.0],
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

					let transform =
						transform_gizmo_egui::math::Transform::from_scale_rotation_translation(
							glam::DVec3::default(),
							glam::DQuat::default(),
							[scene.width as f64 / 2.0, scene.height as f64 / 2.0, 0.0],
						);

					if let Some((result, _)) = scene.gizmo.interact(ui, &[transform]) {
						match result {
							GizmoResult::Translation { delta, total: _ } => {
								for layer in &mut self.multi_select {
									let mut layer = layer.try_lock().unwrap();
									let Some(video) = &mut layer.video else {
										continue;
									};
									if video.pos_x.keys.is_empty() {
										video.pos_x.keys.push(kkdlib::aet::FCurveKey {
											frame: 0.0,
											value: 0.0,
											tangent: 0.0,
										});
									}
									for key in &mut video.pos_x.keys {
										key.value += delta.x as f32;
									}

									if video.pos_y.keys.is_empty() {
										video.pos_y.keys.push(kkdlib::aet::FCurveKey {
											frame: 0.0,
											value: 0.0,
											tangent: 0.0,
										});
									}
									for key in &mut video.pos_y.keys {
										key.value += -delta.y as f32;
									}
								}
							}
							GizmoResult::Rotation {
								axis: _,
								delta,
								total: _,
								is_view_axis: _,
							} => {
								for layer in &mut self.multi_select {
									let mut layer = layer.try_lock().unwrap();
									let Some(video) = &mut layer.video else {
										continue;
									};
									if video.rot_z.keys.is_empty() {
										video.rot_z.keys.push(kkdlib::aet::FCurveKey {
											frame: 0.0,
											value: 0.0,
											tangent: 0.0,
										});
									}

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

			if let Some(node) = &mut self.sprite_set
				&& self.selected.len() >= 2
				&& self.selected[0] == 1
			{
				show_node_visual(ui, node, 1, 0, &[], &self.selected);
			}
		});
	}
}
