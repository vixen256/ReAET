use crate::*;
use eframe::egui;
use eframe::egui::NumExt;
use eframe::egui::util::undoer::Undoer;
use regex::Regex;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::*;

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
	fn display_opts(&mut self, _ui: &mut egui::Ui) {}
	fn display_ctx_menu(&mut self, _ui: &mut egui::Ui) {}
	fn raw_data(&self) -> Vec<u8> {
		Vec::new()
	}
}

static FARC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.farc$").unwrap());
static SPRSET: LazyLock<Regex> = LazyLock::new(spr::SpriteSetNode::name_pattern);
static TXPSET: LazyLock<Regex> = LazyLock::new(txp::TextureSetNode::name_pattern);
static AETSET: LazyLock<Regex> = LazyLock::new(aet::AetSetNode::name_pattern);
static SPRDB: LazyLock<Regex> = LazyLock::new(spr_db::SprDbNode::name_pattern);

pub fn file_dialog_right_panel(ui: &mut egui::Ui, dia: &mut egui_file_dialog::FileDialog) {
	let Some(entry) = dia.selected_entry() else {
		return;
	};
	if !entry.is_file() {
		return;
	}

	let extension = entry.as_path().extension().unwrap_or_default();
	if image::ImageFormat::from_extension(extension).is_none() {
		return;
	}

	ui.image(format!(
		"file://{}",
		entry.as_path().to_str().unwrap_or_default()
	));
}

pub struct App {
	aet_set: Option<aet::AetSetNode>,
	aet_set_filepath: Option<PathBuf>,
	sprite_set: Option<spr::SpriteSetNode>,
	sprite_set_filepath: Option<PathBuf>,
	spr_db: Option<spr_db::SprDbNode>,
	spr_db_filepath: Option<PathBuf>,
	selected: Vec<usize>,
	file_dialog: egui_file_dialog::FileDialog,

	undoer: Option<Undoer<aet::AetSetNode>>,
}

impl App {
	pub fn new(cc: &eframe::CreationContext) -> Option<Self> {
		cc.egui_ctx.set_zoom_factor(1.2);
		cc.egui_ctx.set_theme(egui::Theme::Light);

		egui_extras::install_image_loaders(&cc.egui_ctx);
		egui_material_icons::initialize(&cc.egui_ctx);
		cc.egui_ctx
			.style_mut(|style| style.spacing.scroll = egui::style::ScrollStyle::solid());

		let wgpu_render_state = cc.wgpu_render_state.as_ref()?;
		txp::setup_wgpu(wgpu_render_state);

		let file_dialog = egui_file_dialog::FileDialog::new()
			.show_new_folder_button(false)
			.add_file_filter(
				"Known diva files",
				Arc::new(|path| {
					let path = path.file_name().unwrap().to_str().unwrap();
					FARC.is_match(path)
						|| SPRSET.is_match(path)
						|| TXPSET.is_match(path)
						|| AETSET.is_match(path)
						|| SPRDB.is_match(path)
				}),
			)
			.default_file_filter("Known diva files");

		Some(Self {
			aet_set: None,
			aet_set_filepath: None,
			sprite_set: None,
			sprite_set_filepath: None,
			spr_db: None,
			spr_db_filepath: None,
			selected: Vec::new(),
			file_dialog,
			undoer: None,
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

		let mut desired_width = text_max_x + button_padding.x - available.left();
		if ui.visuals().collapsing_header_frame {
			desired_width = desired_width.max(available.width()); // fill full width
		}

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
					.clicked()
				{
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

pub fn show_node(
	ui: &mut egui::Ui,
	node: &mut dyn TreeNode,
	index: usize,
	path: &[usize],
	selected: &mut Vec<usize>,
	frame: &mut eframe::Frame,
) -> egui::Response {
	let mut path = path.to_vec();
	path.push(index);

	if node.has_custom_tree() {
		node.display_tree(ui, &path, selected, frame)
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
							show_node(ui, child, index, &path, selected, frame);
							index += 1;
						});
					},
				)
			})
			.inner
			.header_response;

		if node.has_context_menu() {
			let menu = egui::Popup::context_menu(&resp).show(|ui| node.display_ctx_menu(ui));
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
			let menu = egui::Popup::context_menu(&resp).show(|ui| node.display_ctx_menu(ui));

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
	}
}

fn show_node_opts(
	ui: &mut egui::Ui,
	node: &mut dyn TreeNode,
	index: usize,
	depth: usize,
	path: &[usize],
	desired_path: &[usize],
) {
	if depth == desired_path.len() - 1 {
		if desired_path[depth] == index {
			node.display_opts(ui);
		}
		return;
	}

	let desired_index = desired_path[depth + 1];
	let mut new_path = path.to_vec();
	new_path.push(index);

	let mut index = 0;
	node.display_children(&mut |child| {
		if index == desired_index {
			show_node_opts(ui, child, index, depth + 1, &new_path, desired_path);
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

		if AETSET.is_match(name) {
			self.aet_set = Some(aet::AetSetNode::read(&name, data));
			self.aet_set_filepath = Some(path.clone());
			self.spr_db = None;
			self.sprite_set = None;
		} else if SPRSET.is_match(name) {
			let spr_set = spr::SpriteSetNode::read(&name, data);
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
		} else if FARC.is_match(name) {
			let farc = kkdlib::farc::Farc::from_buf(data, true);
			for file in farc.files() {
				if SPRSET.is_match(&file.name()) {
					let spr_set = spr::SpriteSetNode::read(&file.name(), file.data().unwrap());
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
				}
			}
		} else if SPRDB.is_match(name) {
			self.spr_db = Some(spr_db::SprDbNode::read(&data, false));
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
					if SPRDB.is_match(&file.file_name().to_string_lossy().to_string())
						&& let Ok(data) = std::fs::read(file.path())
					{
						self.spr_db = Some(spr_db::SprDbNode::read(&data, false));
						self.spr_db_filepath = Some(file.path());
						break;
					}
				}
			}

			if let Some(aet_set) = &mut self.aet_set
				&& let Some(spr_db) = &self.spr_db
				&& let Some(scene) = aet_set.scenes.first()
				&& let Some(sprite_id) = scene.root.get_sprite_id()
				&& let Some(db_set) = spr_db.sets.iter().find(|set| {
					set.lock()
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
						let mut spr_set = spr::SpriteSetNode::read(&name, &data);
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

								break;
							}
						}
					}
				}
			}
		}

		if let Some(aet_set) = &self.aet_set {
			let mut undoer = Undoer::default();
			self.undoer = Some(undoer);
		}
	}

	fn save_files(&self) {
		if let Some(aet_set) = &self.aet_set
			&& let Some(path) = &self.aet_set_filepath
		{
			let data = aet_set.raw_data();
			_ = std::fs::write(path, &data);
		}

		if let Some(sprite_set) = &self.sprite_set
			&& let Some(path) = &self.sprite_set_filepath
		{
			let data = sprite_set.raw_data();
			if path.extension() == Some(std::ffi::OsString::from("farc").as_os_str()) {
				let mut farc = kkdlib::farc::Farc::new();
				farc.add_file_data(&sprite_set.name, &data);
				let data = farc.to_buf().unwrap_or_default();
				_ = std::fs::write(path, &data);
			} else {
				_ = std::fs::write(path, &data);
			}
		}

		if let Some(spr_db) = &self.spr_db
			&& let Some(path) = &self.spr_db_filepath
		{
			let data = spr_db.raw_data();
			_ = std::fs::write(path, &data);
		}
	}
}

const SAVE_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::S,
};

const UNDO_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::Z,
};

const REDO_SHORTCUT: egui::KeyboardShortcut = egui::KeyboardShortcut {
	modifiers: egui::Modifiers::COMMAND,
	logical_key: egui::Key::Y,
};

impl eframe::App for App {
	fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
		ctx.input_mut(|input| {
			for file in &input.raw.dropped_files {
				if let Some(path) = &file.path
					&& path.is_file()
					&& let Ok(data) = std::fs::read(path)
				{
					self.set_file(frame, path, &data);
				}
			}

			if input.consume_shortcut(&SAVE_SHORTCUT) {
				self.save_files();
			}

			if let Some(undoer) = &mut self.undoer
				&& let Some(aet_set) = &mut self.aet_set
			{
				if input.consume_shortcut(&UNDO_SHORTCUT)
					&& let Some(undone) = undoer.undo(aet_set)
				{
					aet_set.update_from(undone);

					if let Some(spr_db) = &self.spr_db
						&& let Some(spr_set) = &self.sprite_set
					{
						for scene in &mut aet_set.scenes {
							scene.root.update_video_textures(spr_db, spr_set);
						}
					}
				}

				if input.consume_shortcut(&REDO_SHORTCUT)
					&& let Some(redone) = undoer.redo(aet_set)
				{
					aet_set.update_from(redone);

					if let Some(spr_db) = &self.spr_db
						&& let Some(spr_set) = &self.sprite_set
					{
						for scene in &mut aet_set.scenes {
							scene.root.update_video_textures(spr_db, spr_set);
						}
					}
				}
			}
		});

		if let Some(aet_set) = &self.aet_set
			&& let Some(undoer) = &mut self.undoer
		{
			undoer.feed_state(ctx.input(|input| input.time), aet_set);
		}

		self.file_dialog
			.update_with_right_panel_ui(ctx, &mut file_dialog_right_panel);

		if let Some(path) = self.file_dialog.take_picked() {
			if let Ok(data) = std::fs::read(&path) {
				self.set_file(frame, &path, &data);
			}
		}

		egui::TopBottomPanel::new(egui::panel::TopBottomSide::Top, "MenuBar").show(ctx, |ui| {
			egui::MenuBar::new().ui(ui, |ui| {
				ui.menu_button("File", |ui| {
					if ui.button("Open").clicked() {
						self.file_dialog.pick_file();
						self.selected = Vec::new();
						ui.close();
					}

					if ui
						.add(
							egui::Button::new("Save")
								.shortcut_text(ctx.format_shortcut(&SAVE_SHORTCUT)),
						)
						.clicked()
					{
						self.save_files();
					}
				});

				ui.menu_button("Edit", |ui| {
					if let Some(undoer) = &mut self.undoer
						&& let Some(aet_set) = &mut self.aet_set
					{
						if ui
							.add_enabled(
								undoer.has_undo(aet_set),
								egui::Button::new("Undo")
									.shortcut_text(ctx.format_shortcut(&UNDO_SHORTCUT)),
							)
							.clicked() && let Some(undone) = undoer.undo(aet_set)
						{
							aet_set.update_from(undone);

							if let Some(spr_db) = &self.spr_db
								&& let Some(spr_set) = &self.sprite_set
							{
								for scene in &mut aet_set.scenes {
									scene.root.update_video_textures(spr_db, spr_set);
								}
							}
						}

						if ui
							.add_enabled(
								undoer.has_redo(aet_set),
								egui::Button::new("Redo")
									.shortcut_text(ctx.format_shortcut(&REDO_SHORTCUT)),
							)
							.clicked() && let Some(redone) = undoer.redo(aet_set)
						{
							aet_set.update_from(redone);

							if let Some(spr_db) = &self.spr_db
								&& let Some(spr_set) = &self.sprite_set
							{
								for scene in &mut aet_set.scenes {
									scene.root.update_video_textures(spr_db, spr_set);
								}
							}
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
			});
		});

		egui::SidePanel::right("RightSidePanel")
			.resizable(true)
			.show(ctx, |ui| {
				if !self.selected.is_empty() {
					egui::TopBottomPanel::bottom("NodeOptions")
						.resizable(true)
						.show_inside(ui, |ui| {
							if let Some(node) = &mut self.aet_set
								&& self.selected[0] == 0
							{
								show_node_opts(ui, node, 0, 0, &[], &self.selected);
							}
							if let Some(node) = &mut self.sprite_set
								&& self.selected[0] == 1
							{
								show_node_opts(ui, node, 1, 0, &[], &self.selected);
							}
							if let Some(node) = &mut self.spr_db
								&& self.selected[0] == 2
							{
								show_node_opts(ui, node, 2, 0, &[], &self.selected);
							}

							ui.take_available_space();
						});
				}

				egui::ScrollArea::vertical().show(ui, |ui| {
					if let Some(node) = &mut self.aet_set {
						show_node(ui, node, 0, &[], &mut self.selected, frame);
					}
					if let Some(node) = &mut self.sprite_set {
						show_node(ui, node, 1, &[], &mut self.selected, frame);
					}
					if let Some(node) = &mut self.spr_db {
						show_node(ui, node, 2, &[], &mut self.selected, frame);
					}

					ui.take_available_space();
				});

				ui.take_available_space();
			});

		egui::SidePanel::left("LeftSidePanel")
			.resizable(true)
			.show(ctx, |ui| {
				if let Some(scene) = self.get_active_scene() {
					if ui.ctx().memory(|memory| memory.focused().is_none()) {
						if ui.input_mut(|input| {
							input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
						}) {
							scene.playing = !scene.playing;
						}

						if ui.input_mut(|input| {
							input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
						}) {
							scene.current_time -= 1.0;
						}

						if ui.input_mut(|input| {
							input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
						}) {
							scene.current_time += 1.0;
						}
					}

					ui.checkbox(&mut scene.playing, "Playing");
					ui.checkbox(&mut scene.display_placeholders, "Display placeholders");
					ui.checkbox(&mut scene.centered, "Centered");
					ui.add(
						egui::Slider::new(
							&mut scene.current_time,
							scene.start_time..=scene.end_time,
						)
						.text("Time"),
					);

					if scene.playing && scene.current_time < scene.end_time {
						ctx.input(|input| {
							scene.current_time += input.stable_dt * scene.fps;
						});
						ctx.request_repaint_after_secs(1.0 / scene.fps);
					}
				}
				ui.take_available_space();
			});

		egui::TopBottomPanel::bottom("CurveEditor")
			.resizable(true)
			.show(ctx, |ui| {
				if let Some(node) = &mut self.aet_set
					&& self.selected.len() >= 2
					&& self.selected[0] == 0
					&& let Some(scene) = node.scenes.get_mut(self.selected[1])
				{
					scene.root.show_node_curve_editor(
						ui,
						&mut scene.selected_curve,
						scene.current_time,
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
					.lock()
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
			let selected = self.selected.clone();
			if let Some(scene) = self.get_active_scene() {
				let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::empty());
				scene.display_visual(ui, rect, &selected)
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
