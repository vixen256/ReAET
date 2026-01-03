use crate::app::TreeNode;
use eframe::egui;
use eframe::egui::Widget;
use kkdlib::database::sprite::*;
use regex::Regex;
use std::rc::Rc;
use std::sync::Mutex;

pub struct SprDbNode {
	pub filename: String,
	pub modern: bool,
	pub big_endian: bool,
	pub is_x: bool,
	pub sets: Vec<Rc<Mutex<SprDbSetNode>>>,
}

impl TreeNode for SprDbNode {
	fn label(&self) -> &str {
		&self.filename
	}

	fn has_children(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		for set in &mut self.sets {
			let mut set = set.try_lock().unwrap();
			f(&mut *set);
		}
	}

	fn raw_data(&self) -> Vec<u8> {
		let mut spr_db = file::Database::new();
		spr_db.set_ready(true);
		spr_db.set_modern(self.modern);
		spr_db.set_big_endian(self.big_endian);
		spr_db.set_is_x(self.is_x);

		for set in &self.sets {
			let set = set.try_lock().unwrap();
			let mut db_set = file::Set::new();
			db_set.set_id(set.id);
			db_set.set_name(&set.name);
			db_set.set_file_name(&set.file_name);

			for entry in &set.entries {
				let entry = entry.try_lock().unwrap();
				let mut db_entry = file::Entry::new();
				db_entry.set_id(entry.id);
				db_entry.set_name(&entry.name);
				db_entry.set_index(entry.index);
				db_entry.set_texture(entry.texture);

				db_set.add_sprite(&db_entry);
			}

			spr_db.add_set(&db_set);
		}

		spr_db.to_buf().unwrap_or_default()
	}

	fn display_opts(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		let height = ui.text_style_height(&egui::TextStyle::Body);
		egui_extras::TableBuilder::new(ui)
			.column(egui_extras::Column::remainder())
			.column(egui_extras::Column::remainder())
			.body(|mut body| {
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
}

impl SprDbNode {
	pub fn name_pattern() -> Regex {
		Regex::new(r"(spr_db.bin)|(\.spi)$").unwrap()
	}

	pub fn read(filename: &str, data: &[u8]) -> Self {
		let spr_db = file::Database::from_buf(data, filename.ends_with("spi"));

		Self {
			filename: filename.to_string(),
			modern: spr_db.modern(),
			big_endian: spr_db.big_endian(),
			is_x: spr_db.is_x(),
			sets: spr_db
				.sets()
				.map(|set| {
					Rc::new(Mutex::new(SprDbSetNode {
						id: set.id(),
						name: set.name(),
						file_name: set.file_name(),
						entries: set
							.sprites()
							.map(|entry| {
								Rc::new(Mutex::new(SprDbEntryNode {
									id: entry.id(),
									name: entry.name(),
									index: entry.index(),
									texture: entry.texture(),
								}))
							})
							.collect(),
					}))
				})
				.collect(),
		}
	}
}

pub struct SprDbSetNode {
	pub id: u32,
	pub name: String,
	pub file_name: String,
	pub entries: Vec<Rc<Mutex<SprDbEntryNode>>>,
}

impl TreeNode for SprDbSetNode {
	fn label(&self) -> &str {
		&self.name
	}

	fn has_children(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		for entry in &mut self.entries {
			let mut entry = entry.try_lock().unwrap();
			f(&mut *entry);
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
						ui.label("File");
					});
					row.col(|ui| {
						ui.text_edit_singleline(&mut self.file_name);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("ID");
					});
					row.col(|ui| {
						ui.horizontal(|ui| {
							crate::app::num_edit(ui, &mut self.id, 0);

							if ui.button("Murmur").clicked() {
								self.id =
									kkdlib::hash::murmurhash(self.name.bytes().collect::<Vec<_>>());
							}
						});
					});
				});
			});
	}
}

pub struct SprDbEntryNode {
	pub id: u32,
	pub name: String,
	pub index: u16,
	pub texture: bool,
}

impl TreeNode for SprDbEntryNode {
	fn label(&self) -> &str {
		&self.name
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
						ui.label("ID");
					});
					row.col(|ui| {
						ui.horizontal(|ui| {
							crate::app::num_edit(ui, &mut self.id, 0);

							if ui.button("Murmur").clicked() {
								self.id =
									kkdlib::hash::murmurhash(self.name.bytes().collect::<Vec<_>>());
							}
						});
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Index");
					});
					row.col(|ui| {
						crate::app::num_edit(ui, &mut self.index, 0);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Texture");
					});
					row.col(|ui| {
						egui::Checkbox::without_text(&mut self.texture).ui(ui);
					});
				});
			});
	}
}
