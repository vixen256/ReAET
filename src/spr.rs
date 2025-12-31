use crate::app::TreeNode;
use crate::spr_db::*;
use crate::txp::*;
use eframe::egui;
use eframe::egui::Widget;
use eframe::egui_wgpu;
use eframe::egui_wgpu::wgpu;
use image::{EncodableLayout, GenericImage};
use kkdlib::spr;
use regex::Regex;
use std::rc::Rc;
use std::sync::Mutex;

pub struct SpriteSetNode {
	pub name: String,
	pub modern: bool,
	pub big_endian: bool,
	pub is_x: bool,
	pub flag: u32,
	pub sprites_node: SpriteInfosNode,
	pub textures_node: TextureSetNode,
	pub texture_names: Rc<Mutex<Vec<String>>>,
	pub db_set: Option<Rc<Mutex<SprDbSetNode>>>,
}

impl TreeNode for SpriteSetNode {
	fn label(&self) -> &str {
		&self.name
	}

	fn has_children(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		f(&mut self.sprites_node);
		for sprite in self.sprites_node.children.try_lock().unwrap().iter_mut() {
			let mut sprite = sprite.try_lock().unwrap();
			if let Some(texid) = sprite.want_new_texture {
				sprite.texture = self.textures_node.children[texid as usize].clone();
			}
			sprite.want_new_texture = None;
		}
		f(&mut self.textures_node);

		self.texture_names.try_lock().unwrap().clone_from(
			&self
				.textures_node
				.children
				.iter()
				.map(|child| child.try_lock().unwrap().name.clone())
				.collect(),
		);
	}

	fn raw_data(&self) -> Vec<u8> {
		let mut txp_set = kkdlib::txp::Set::new();
		let mut tex_names = Vec::new();
		let mut spr_set = spr::Set::new();

		for texture in &self.textures_node.children {
			let texture = &texture.try_lock().unwrap();
			txp_set.add_file(&texture.texture);
			tex_names.push(texture.name.clone());
		}

		for sprite in self.sprites_node.children.try_lock().unwrap().iter() {
			let sprite = sprite.try_lock().unwrap();
			spr_set.add_spr(&sprite.info, &sprite.name);
		}
		spr_set.set_txp(&txp_set, tex_names);
		spr_set.set_ready(true);
		spr_set.set_modern(self.modern);
		spr_set.set_big_endian(self.big_endian);
		spr_set.set_is_x(self.is_x);
		spr_set.set_flag(self.flag);

		spr_set.to_buf().unwrap_or_default()
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

				if let Some(db_set) = &mut self.db_set {
					let mut db_set = db_set.try_lock().unwrap();
					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("ID");
						});
						row.col(|ui| {
							ui.horizontal(|ui| {
								egui::DragValue::new(&mut db_set.id)
									.max_decimals(0)
									.speed(0.0)
									.update_while_editing(true)
									.ui(ui);

								if ui.button("Murmur").clicked() {
									db_set.id = kkdlib::hash::murmurhash(
										db_set.name.bytes().collect::<Vec<_>>(),
									);
								}
							});
						});
					});

					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("Name");
						});
						row.col(|ui| {
							ui.text_edit_singleline(&mut db_set.name);
						});
					});
				}
			});
	}
}

impl SpriteSetNode {
	pub fn name_pattern() -> Regex {
		Regex::new(r"^spr_.*\.bin$").unwrap()
	}

	pub fn read(name: &str, data: &[u8]) -> Self {
		let set = spr::Set::from_buf(data, false);
		let textures_node = TextureSetNode::from_sprset(&set);
		let texture_names = Rc::new(Mutex::new(
			textures_node
				.children
				.iter()
				.map(|child| child.try_lock().unwrap().name.clone())
				.collect(),
		));
		Self {
			name: String::from(name),
			modern: set.modern(),
			big_endian: set.big_endian(),
			is_x: set.is_x(),
			flag: set.flag(),
			sprites_node: SpriteInfosNode::new(&set, &textures_node, texture_names.clone()),
			textures_node,
			texture_names,
			db_set: None,
		}
	}

	pub fn add_db(&mut self, db_set: Rc<Mutex<SprDbSetNode>>) {
		let set = db_set.try_lock().unwrap();
		for (i, sprite) in self
			.sprites_node
			.children
			.lock()
			.unwrap()
			.iter_mut()
			.enumerate()
		{
			let mut sprite = sprite.try_lock().unwrap();
			sprite.db_entry = set
				.entries
				.iter()
				.find(|entry| {
					let entry = entry.try_lock().unwrap();
					entry.index == i as u16 && entry.texture == false
				})
				.cloned();
		}

		for (i, texture) in self.textures_node.children.iter_mut().enumerate() {
			let mut texture = texture.try_lock().unwrap();
			texture.db_entry = set
				.entries
				.iter()
				.find(|entry| {
					let entry = entry.try_lock().unwrap();
					entry.index == i as u16 && entry.texture == true
				})
				.cloned();
		}
		drop(set);

		self.db_set = Some(db_set);
	}

	pub fn update_db_entries(&mut self) {
		let Some(set) = &self.db_set else { return };
		let mut set = set.try_lock().unwrap();
		set.entries.clear();

		for sprite in self.sprites_node.children.try_lock().unwrap().iter() {
			let sprite = sprite.try_lock().unwrap();
			let Some(db_entry) = &sprite.db_entry else {
				return;
			};
			let mut entry = db_entry.try_lock().unwrap();
			entry.name = format!("{}_{}", set.name, sprite.name);

			set.entries.push(db_entry.clone());
		}

		for tex in &self.textures_node.children {
			let tex = tex.try_lock().unwrap();
			let Some(db_entry) = &tex.db_entry else {
				return;
			};
			let mut entry = db_entry.try_lock().unwrap();
			entry.name = format!("{}_{}", set.name.replace("SPR_", "SPRTEX_"), tex.name);

			set.entries.push(db_entry.clone());
		}
	}

	pub fn init_wgpu(&self, frame: &mut eframe::Frame) {
		let render_state = frame.wgpu_render_state().unwrap();
		let device = &render_state.device;

		let mut textures = Vec::new();

		let callback_resources = render_state.renderer.read();
		let resources: &WgpuRenderResources = callback_resources.callback_resources.get().unwrap();

		let (tl, tr, bl, br) = ([-1.0, 1.0], [1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]);

		let verticies = [
			Vertex {
				position: tr,
				tex_index: 1,
			},
			Vertex {
				position: bl,
				tex_index: 2,
			},
			Vertex {
				position: br,
				tex_index: 3,
			},
			Vertex {
				position: tl,
				tex_index: 0,
			},
			Vertex {
				position: bl,
				tex_index: 2,
			},
			Vertex {
				position: tr,
				tex_index: 1,
			},
		];

		render_state.queue.write_buffer(
			&resources.vertex_buffer,
			0,
			bytemuck::cast_slice(&verticies),
		);

		for texture in &self.textures_node.children {
			let tex = texture.try_lock().unwrap();

			let Some(mip) = tex.texture.get_mipmap(0, 0) else {
				continue;
			};

			let mut data = mip.data().unwrap().to_vec();

			let format = match mip.format() {
				kkdlib::txp::Format::A8
				| kkdlib::txp::Format::RGB8
				| kkdlib::txp::Format::RGB5
				| kkdlib::txp::Format::RGB5A1
				| kkdlib::txp::Format::RGBA4
				| kkdlib::txp::Format::L8
				| kkdlib::txp::Format::L8A8 => {
					data = mip.rgba().unwrap();
					wgpu::TextureFormat::Rgba8Unorm
				}
				kkdlib::txp::Format::RGBA8 => wgpu::TextureFormat::Rgba8Unorm,
				kkdlib::txp::Format::BC1 | kkdlib::txp::Format::BC1a => {
					wgpu::TextureFormat::Bc1RgbaUnorm
				}
				kkdlib::txp::Format::BC2 => wgpu::TextureFormat::Bc2RgbaUnorm,
				kkdlib::txp::Format::BC3 => wgpu::TextureFormat::Bc3RgbaUnorm,
				kkdlib::txp::Format::BC4 => wgpu::TextureFormat::Bc4RSnorm,
				kkdlib::txp::Format::BC5 => wgpu::TextureFormat::Bc5RgUnorm,
				kkdlib::txp::Format::BC7 => wgpu::TextureFormat::Bc7RgbaUnorm,
				kkdlib::txp::Format::BC6H => wgpu::TextureFormat::Bc6hRgbUfloat,
			};

			let (width, height) = if format.is_bcn() {
				(
					(mip.width() as u32 + 4 - 1) / 4 * 4,
					(mip.height() as u32 + 4 - 1) / 4 * 4,
				)
			} else {
				(mip.width() as u32, mip.height() as u32)
			};

			let size = wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			};

			let texture = if tex.texture.is_ycbcr() {
				let texture = device.create_texture(&wgpu::TextureDescriptor {
					size,
					mip_level_count: 2,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format,
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
					label: Some(&tex.name),
					view_formats: &[],
				});

				render_state.queue.write_texture(
					wgpu::TexelCopyTextureInfo {
						texture: &texture,
						mip_level: 0,
						origin: wgpu::Origin3d::ZERO,
						aspect: wgpu::TextureAspect::All,
					},
					&data,
					wgpu::TexelCopyBufferLayout {
						offset: 0,
						bytes_per_row: Some(width * 4),
						rows_per_image: Some(height),
					},
					size,
				);

				let mip = tex.texture.get_mipmap(0, 1).unwrap();
				let width = (mip.width() as u32 + 4 - 1) / 4 * 4;
				let height = (mip.height() as u32 + 4 - 1) / 4 * 4;

				render_state.queue.write_texture(
					wgpu::TexelCopyTextureInfo {
						texture: &texture,
						mip_level: 1,
						origin: wgpu::Origin3d::ZERO,
						aspect: wgpu::TextureAspect::All,
					},
					mip.data().unwrap(),
					wgpu::TexelCopyBufferLayout {
						offset: 0,
						bytes_per_row: Some(width * 4),
						rows_per_image: Some(height),
					},
					wgpu::Extent3d {
						width,
						height,
						depth_or_array_layers: 1,
					},
				);

				texture
			} else {
				let texture = device.create_texture(&wgpu::TextureDescriptor {
					size,
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format,
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
					label: Some(&tex.name),
					view_formats: &[],
				});

				let bytes_per_row = match format {
					wgpu::TextureFormat::Rgba8Unorm => width * 4,
					wgpu::TextureFormat::Bc1RgbaUnorm => width * 2,
					wgpu::TextureFormat::Bc2RgbaUnorm => width * 4,
					wgpu::TextureFormat::Bc3RgbaUnorm => width * 4,
					wgpu::TextureFormat::Bc4RSnorm => width * 2,
					wgpu::TextureFormat::Bc5RgUnorm => width * 4,
					wgpu::TextureFormat::Bc7RgbaUnorm => width * 4,
					wgpu::TextureFormat::Bc6hRgbUfloat => width * 4,
					_ => unreachable!(),
				};

				render_state.queue.write_texture(
					wgpu::TexelCopyTextureInfo {
						texture: &texture,
						mip_level: 0,
						origin: wgpu::Origin3d::ZERO,
						aspect: wgpu::TextureAspect::All,
					},
					&data,
					wgpu::TexelCopyBufferLayout {
						offset: 0,
						bytes_per_row: Some(bytes_per_row),
						rows_per_image: Some(height),
					},
					size,
				);

				texture
			};

			let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
				layout: &resources.fragment_bind_group_layout,
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(
							&texture.create_view(&wgpu::TextureViewDescriptor::default()),
						),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(&resources.sampler),
					},
				],
				label: Some("Fragment bind group"),
			});

			textures.push((texture, bind_group));
		}

		let empty_texture = device.create_texture(&wgpu::TextureDescriptor {
			size: wgpu::Extent3d {
				width: 1,
				height: 1,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			label: None,
			view_formats: &[],
		});

		render_state.queue.write_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &empty_texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			&[0xFF, 0xFF, 0xFF, 0xFF],
			wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(4),
				rows_per_image: Some(1),
			},
			wgpu::Extent3d {
				width: 1,
				height: 1,
				depth_or_array_layers: 1,
			},
		);

		let empty_texture = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &resources.fragment_bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(
						&empty_texture.create_view(&wgpu::TextureViewDescriptor::default()),
					),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&resources.sampler),
				},
			],
			label: Some("Fragment bind group"),
		});

		drop(callback_resources);

		render_state
			.renderer
			.write()
			.callback_resources
			.insert(WgpuRenderTextures {
				fragment_bind_group: textures,
				empty_texture,
			});
	}
}

pub struct SpriteInfosNode {
	pub children: Rc<Mutex<Vec<Rc<Mutex<SpriteInfoNode>>>>>,
	pub texture_names: Rc<Mutex<Vec<String>>>,
}

impl TreeNode for SpriteInfosNode {
	fn label(&self) -> &str {
		"Sprites"
	}

	fn has_children(&self) -> bool {
		true
	}

	fn has_context_menu(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		self.children.try_lock().unwrap().retain_mut(|spr| {
			let mut spr = spr.try_lock().unwrap();
			let texid = spr.texture.try_lock().unwrap().index;
			spr.info.set_texid(texid);
			f(&mut *spr);
			!spr.want_deletion
		});
	}

	fn display_ctx_menu(&mut self, ui: &mut egui::Ui) {
		if ui.button("Add").clicked() {
			let mut info = spr::Info::new();
			info.set_texid(0);
			info.set_px(2.0);
			info.set_py(2.0);
			info.set_width(2.0);
			info.set_height(2.0);
			info.set_resolution_mode(spr::ResolutionMode::FHD);
			let len = self.children.try_lock().unwrap().len();

			self.children
				.lock()
				.unwrap()
				.push(Rc::new(Mutex::new(SpriteInfoNode {
					file_dialog: egui_file_dialog::FileDialog::new()
						.show_new_folder_button(false)
						.add_save_extension("JPEG", "jpg")
						.add_save_extension("PNG", "png")
						.add_save_extension("WEBP", "webp")
						.default_save_extension("PNG")
						.add_file_filter_extensions("Images", vec!["dds", "jpg", "png", "webp"])
						.default_file_filter("Images"),
					name: format!("Sprite {}", len),
					info,
					texture: Rc::new(Mutex::new(TextureNode {
						name: String::new(),
						texture: kkdlib::txp::Texture::new(),
						flip: true,
						index: 0,
						texture_updated: false,
						db_entry: None,
						file_dialog: egui_file_dialog::FileDialog::new(),
						exporting: false,
						error: None,
						want_deletion: false,
					})),
					texture_names: self.texture_names.clone(),
					want_new_texture: Some(0),
					db_entry: None,
					exporting: false,
					error: None,
					want_deletion: false,
				})));
		}
	}
}

impl SpriteInfosNode {
	fn new(
		set: &spr::Set,
		textures_node: &TextureSetNode,
		texture_names: Rc<Mutex<Vec<String>>>,
	) -> Self {
		Self {
			children: Rc::new(Mutex::new(
				set.sprites()
					.filter(|(_, info)| (info.texid() as usize) < textures_node.children.len())
					.map(|(name, info)| {
						Rc::new(Mutex::new(SpriteInfoNode {
							file_dialog: egui_file_dialog::FileDialog::new()
								.show_new_folder_button(false)
								.add_save_extension("JPEG", "jpg")
								.add_save_extension("PNG", "png")
								.add_save_extension("WEBP", "webp")
								.default_save_extension("PNG")
								.add_file_filter_extensions(
									"Images",
									vec!["dds", "jpg", "png", "webp"],
								)
								.default_file_filter("Images")
								.default_file_name(&name),
							name,
							info: info.clone(),
							texture: textures_node.children[info.texid() as usize].clone(),
							texture_names: texture_names.clone(),
							want_new_texture: None,
							db_entry: None,
							exporting: false,
							error: None,
							want_deletion: false,
						}))
					})
					.collect(),
			)),
			texture_names,
		}
	}
}

pub struct SpriteInfoNode {
	pub name: String,
	pub info: spr::Info,
	pub texture: Rc<Mutex<TextureNode>>,
	pub texture_names: Rc<Mutex<Vec<String>>>,
	pub want_new_texture: Option<u32>,
	pub db_entry: Option<Rc<Mutex<SprDbEntryNode>>>,
	pub file_dialog: egui_file_dialog::FileDialog,
	pub exporting: bool,
	pub error: Option<String>,
	pub want_deletion: bool,
}

impl SpriteInfoNode {
	fn pick_file(&mut self, path: std::path::PathBuf) {
		let extension = path.extension().unwrap_or_default();
		let Some(format) = image::ImageFormat::from_extension(extension) else {
			self.error = Some(format!("Could not determine format of {:?}", path));
			return;
		};

		let mut texture = self.texture.try_lock().unwrap();
		let mip = texture.texture.get_mipmap(0, 0).unwrap();

		if self.exporting {
			let rgba = if texture.texture.is_ycbcr() {
				texture.texture.decode_ycbcr()
			} else {
				mip.rgba()
			};
			let Some(rgba) = rgba else {
				self.error = Some(String::from("Could not convert texture to rgba"));
				return;
			};
			let Some(image) =
				image::RgbaImage::from_raw(mip.width() as u32, mip.height() as u32, rgba)
			else {
				self.error = Some(String::from("Could not load image"));
				return;
			};
			if let Err(e) = image::DynamicImage::ImageRgba8(image)
				.flipv()
				.crop(
					self.info.px() as u32,
					self.info.py() as u32,
					self.info.width() as u32,
					self.info.height() as u32,
				)
				.save_with_format(path, format)
			{
				self.error = Some(format!("Image failed to save: {e}"));
			}
		} else {
			let Ok(data) = std::fs::read(&path) else {
				self.error = Some(format!("Failed to read {:?}", path));
				return;
			};

			let Ok(new_image) = image::load(std::io::Cursor::new(data), format) else {
				self.error = Some(format!("Failed to parse {:?} as image", path));
				return;
			};

			if new_image.width() != self.info.width() as u32
				|| new_image.height() != self.info.height() as u32
			{
				self.error = Some(String::from(
					"New image did match dimensions of current sprite",
				));
				return;
			}

			let rgba = if texture.texture.is_ycbcr() {
				texture.texture.decode_ycbcr()
			} else {
				mip.rgba()
			};
			let Some(rgba) = rgba else {
				self.error = Some(String::from("Failed to convert current texture to RGBA"));
				return;
			};
			let Some(mut image) =
				image::RgbaImage::from_raw(mip.width() as u32, mip.height() as u32, rgba)
			else {
				self.error = Some(String::from("Could not load image"));
				return;
			};

			if let Err(e) = image.copy_from(
				&new_image.flipv(),
				self.info.px() as u32,
				mip.height() as u32 - self.info.py() as u32 - self.info.height() as u32,
			) {
				self.error = Some(format!("Could not copy sprite into current image {e}"));
				return;
			}

			if texture.texture.is_ycbcr() {
				let Some(tex) = kkdlib::txp::Texture::encode_ycbcr(
					image.width() as i32,
					image.height() as i32,
					image.as_bytes(),
				) else {
					self.error = Some(String::from("Could not encode texture"));
					return;
				};

				texture.texture = tex;
				texture.texture_updated = true;
			} else {
				let Some(mipmap) = kkdlib::txp::Mipmap::from_rgba(
					image.width() as i32,
					image.height() as i32,
					image.as_bytes(),
					mip.format(),
				) else {
					self.error = Some(String::from("Could not encode texture"));
					return;
				};

				let mut tex = kkdlib::txp::Texture::new();
				tex.set_has_cube_map(false);
				tex.set_array_size(1);
				tex.set_mipmaps_count(1);
				tex.add_mipmap(&mipmap);
				texture.texture = tex;
				texture.texture_updated = true;
			}
		}
	}
}

impl TreeNode for SpriteInfoNode {
	fn label(&self) -> &str {
		&self.name
	}

	fn has_context_menu(&self) -> bool {
		true
	}

	fn display_ctx_menu(&mut self, ui: &mut egui::Ui) {
		if ui.button("Export").clicked() {
			self.file_dialog.save_file();
			self.exporting = true;
		}
		if ui.button("Replace").clicked() {
			self.file_dialog.pick_file();
			self.exporting = false;
		}
		if ui.button("Remove").clicked() {
			self.want_deletion = true;
		}
	}

	fn display_opts(&mut self, ui: &mut egui::Ui) {
		if let Some(error) = &self.error {
			let modal = egui::Modal::new(egui::Id::new("SpriteInfoError")).show(ui.ctx(), |ui| {
				ui.heading("An error has occured");
				ui.vertical_centered(|ui| {
					ui.label(error);
					if ui.button("Ok").clicked() {
						ui.close();
					}
				});
			});

			if modal.should_close() {
				self.error = None;
			}
		}

		self.file_dialog
			.update_with_right_panel_ui(ui.ctx(), &mut crate::app::file_dialog_right_panel);

		if let Some(path) = self.file_dialog.take_picked() {
			self.pick_file(path);
		}

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
						ui.label("Texture");
					});
					row.col(|ui| {
						let mut texture = self.info.texid();
						let tex_name = &self.texture.try_lock().unwrap().name;
						egui::ComboBox::from_id_salt("TextureComboBox")
							.selected_text(tex_name)
							.show_ui(ui, |ui| {
								for (id, name) in
									self.texture_names.try_lock().unwrap().iter().enumerate()
								{
									ui.selectable_value(&mut texture, id as u32, name);
								}
							});

						if texture != self.info.texid() {
							self.info.set_texid(texture);
							self.want_new_texture = Some(texture);
						}
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("X");
					});
					row.col(|ui| {
						let mut px = self.info.px();
						egui::DragValue::new(&mut px)
							.max_decimals(0)
							.speed(0.0)
							.update_while_editing(true)
							.ui(ui);
						self.info.set_px(px);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Y");
					});
					row.col(|ui| {
						let mut py = self.info.py();
						egui::DragValue::new(&mut py)
							.max_decimals(0)
							.speed(0.0)
							.update_while_editing(true)
							.ui(ui);
						self.info.set_py(py);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Width");
					});
					row.col(|ui| {
						let mut width = self.info.width();
						egui::DragValue::new(&mut width)
							.max_decimals(0)
							.speed(0.0)
							.update_while_editing(true)
							.ui(ui);
						self.info.set_width(width);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Height");
					});
					row.col(|ui| {
						let mut height = self.info.height();
						egui::DragValue::new(&mut height)
							.max_decimals(0)
							.speed(0.0)
							.update_while_editing(true)
							.ui(ui);
						self.info.set_height(height);
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Resolution Mode");
					});
					row.col(|ui| {
						let mut resolution_mode = self.info.resolution_mode();
						egui::ComboBox::from_id_salt("ResolutionModeComboBox")
							.selected_text(format!("{:?}", resolution_mode))
							.show_ui(ui, |ui| {
								for i in 0..=0x20 {
									let mode: spr::ResolutionMode =
										unsafe { std::mem::transmute(i) };
									ui.selectable_value(
										&mut resolution_mode,
										mode,
										format!("{:?}", mode),
									);
								}
							});
						self.info.set_resolution_mode(resolution_mode);
					});
				});

				if let Some(db_entry) = &mut self.db_entry {
					let mut db_entry = db_entry.try_lock().unwrap();

					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("ID");
						});
						row.col(|ui| {
							ui.horizontal(|ui| {
								egui::DragValue::new(&mut db_entry.id)
									.max_decimals(0)
									.speed(0.0)
									.update_while_editing(true)
									.ui(ui);

								if ui.button("Murmur").clicked() {
									db_entry.id = kkdlib::hash::murmurhash(
										db_entry.name.bytes().collect::<Vec<_>>(),
									);
								}
							});
						});
					});
				}
			});
	}

	fn selected(&mut self, frame: &mut eframe::Frame) {
		self.texture.try_lock().unwrap().selected(frame);
	}

	fn display_visual(
		&mut self,
		_ui: &mut egui::Ui,
		rect: egui::Rect,
	) -> Option<egui::epaint::PaintCallback> {
		let texture = self.texture.try_lock().unwrap();

		let w = rect.max.x - rect.min.x;
		let h = rect.max.y - rect.min.y;
		let ar = w / h;
		let sprite_aet = self.info.width() / self.info.height();
		let rect = if ar > sprite_aet {
			let adjusted_w = h / self.info.height() * self.info.width();
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
			let adjusted_h = w / self.info.width() * self.info.height();
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

		let mip = texture.texture.get_mipmap(0, 0).unwrap();
		let x = self.info.px() / mip.width() as f32;
		let y = (mip.height() as f32 - self.info.py() - self.info.height()) / mip.height() as f32;
		let w = (self.info.px() + self.info.width()) / mip.width() as f32;
		let h = (mip.height() as f32 - self.info.py()) / mip.height() as f32;

		Some(egui_wgpu::Callback::new_paint_callback(
			rect,
			WgpuSpriteCallback {
				is_ycbcr: texture.texture.is_ycbcr(),
				sprite_coords: [x, y, w, h],
				texture_index: texture.index,
			},
		))
	}
}

struct WgpuSpriteCallback {
	is_ycbcr: bool,
	sprite_coords: [f32; 4],
	texture_index: u32,
}

impl egui_wgpu::CallbackTrait for WgpuSpriteCallback {
	fn prepare(
		&self,
		_device: &wgpu::Device,
		queue: &wgpu::Queue,
		_screen_descriptor: &egui_wgpu::ScreenDescriptor,
		_egui_encoder: &mut wgpu::CommandEncoder,
		callback_resources: &mut egui_wgpu::CallbackResources,
	) -> Vec<wgpu::CommandBuffer> {
		let resources: &WgpuRenderResources = callback_resources.get().unwrap();

		let spr_info = SpriteInfo {
			matrix: crate::aet::Mat4::default().into(),
			tex_coords: [
				[self.sprite_coords[0], self.sprite_coords[3]],
				[self.sprite_coords[2], self.sprite_coords[3]],
				[self.sprite_coords[0], self.sprite_coords[1]],
				[self.sprite_coords[2], self.sprite_coords[1]],
			],
			color: [1.0, 1.0, 1.0, 1.0],
			is_ycbcr: if self.is_ycbcr { 1 } else { 0 },
			_padding_0: 0,
			_padding_1: 0,
			_padding_2: 0,
		};

		queue.write_buffer(
			&resources.uniform_buffers[0].0,
			0,
			bytemuck::cast_slice(&[spr_info]),
		);

		Vec::new()
	}

	fn paint(
		&self,
		_info: egui::PaintCallbackInfo,
		render_pass: &mut wgpu::RenderPass<'static>,
		callback_resources: &egui_wgpu::CallbackResources,
	) {
		let resources: &WgpuRenderResources = callback_resources.get().unwrap();
		let texture: &WgpuRenderTextures = callback_resources.get().unwrap();
		render_pass.set_pipeline(&resources.pipeline_normal);
		render_pass.set_bind_group(
			0,
			&texture.fragment_bind_group[self.texture_index as usize].1,
			&[],
		);
		render_pass.set_bind_group(1, &resources.uniform_buffers[0].1, &[]);
		render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
		render_pass.draw(0..6, 0..1);
	}
}
