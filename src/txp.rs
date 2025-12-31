use crate::app::TreeNode;
use crate::spr_db::*;
use eframe::egui;
use eframe::egui::Widget;
use eframe::egui_wgpu;
use eframe::egui_wgpu::wgpu;
use eframe::egui_wgpu::wgpu::util::DeviceExt;
use image::EncodableLayout;
use kkdlib::{spr, txp};
use regex::Regex;
use std::rc::Rc;
use std::sync::Mutex;

pub struct TextureSetNode {
	pub big_endian: bool,
	pub modern: bool,
	pub signature: u32,
	pub filename: Option<String>,
	pub children: Vec<Rc<Mutex<TextureNode>>>,
	pub children_changed: bool,
}

impl TreeNode for TextureSetNode {
	fn label(&self) -> &str {
		self.filename
			.as_ref()
			.map_or("Textures", |name| name.as_str())
	}

	fn has_children(&self) -> bool {
		true
	}

	fn has_context_menu(&self) -> bool {
		true
	}

	fn display_children(&mut self, f: &mut dyn FnMut(&mut dyn TreeNode)) {
		let old_len = self.children.len();
		self.children.retain_mut(|tex| {
			let mut tex = tex.try_lock().unwrap();
			f(&mut *tex);
			!tex.want_deletion
		});
		if old_len != self.children.len() {
			for (i, child) in self.children.iter_mut().enumerate() {
				child.try_lock().unwrap().index = i as u32;
			}
			self.children_changed = true;
		}
	}

	fn display_opts(&mut self, ui: &mut egui::Ui) {
		if self.filename.is_some() {
			let height = ui.text_style_height(&egui::TextStyle::Body);
			egui_extras::TableBuilder::new(ui)
				.striped(true)
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

					if self.modern {
						body.row(height, |mut row| {
							row.col(|ui| {
								ui.label("Signature");
							});
							row.col(|ui| {
								egui::DragValue::new(&mut self.signature)
									.hexadecimal(8, false, true)
									.speed(0.0)
									.ui(ui);
							});
						});
					}

					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("Big Endian");
						});
						row.col(|ui| {
							egui::Checkbox::without_text(&mut self.big_endian).ui(ui);
						});
					});
				});
		}
	}

	fn display_ctx_menu(&mut self, ui: &mut egui::Ui) {
		if ui.button("Add").clicked() {
			let name = format!("Texture {:03}", self.children.len());

			let mut mip = txp::Mipmap::new();
			mip.set_height(16);
			mip.set_width(16);
			mip.set_format(txp::Format::RGBA8);
			mip.set_data(&[0u8; 16 * 16 * 4]);

			let mut texture = txp::Texture::new();
			texture.set_array_size(1);
			texture.set_mipmaps_count(1);
			texture.set_has_cube_map(false);
			texture.add_mipmap(&mip);

			self.children.push(Rc::new(Mutex::new(TextureNode {
				file_dialog: egui_file_dialog::FileDialog::new()
					.show_new_folder_button(false)
					.add_save_extension("JPEG", "jpg")
					.add_save_extension("PNG", "png")
					.add_save_extension("WEBP", "webp")
					.default_save_extension("PNG")
					.add_file_filter_extensions("Images", vec!["dds", "jpg", "png", "webp"])
					.default_file_filter("Images")
					.default_file_name(&name),
				name,
				texture,
				flip: self
					.children
					.first()
					.map_or(true, |tex| tex.try_lock().unwrap().flip),
				index: self.children.len() as u32,
				texture_updated: true,
				db_entry: None,
				exporting: false,
				error: None,
				want_deletion: false,
			})));
		}
	}

	fn raw_data(&self) -> Vec<u8> {
		let mut set = txp::Set::new();
		for child in &self.children {
			let texture = child.try_lock().unwrap();
			set.add_file(&texture.texture);
		}

		let modern = if self.modern {
			Some(self.signature)
		} else {
			None
		};
		set.to_buf(self.big_endian, modern).unwrap_or_default()
	}
}

impl TextureSetNode {
	pub fn name_pattern() -> Regex {
		Regex::new(r"(_tex\.bin$)|(\.txd$)").unwrap()
	}

	pub fn from_sprset(set: &spr::Set) -> Self {
		Self {
			big_endian: set.big_endian(),
			modern: set.modern(),
			signature: 0,
			filename: None,
			children: set
				.textures()
				.enumerate()
				.map(|(i, (name, texture))| {
					Rc::new(Mutex::new(TextureNode {
						file_dialog: egui_file_dialog::FileDialog::new()
							.show_new_folder_button(false)
							.add_save_extension("JPEG", "jpg")
							.add_save_extension("PNG", "png")
							.add_save_extension("WEBP", "webp")
							.default_save_extension("PNG")
							.add_file_filter_extensions("Images", vec!["dds", "jpg", "png", "webp"])
							.default_file_filter("Images")
							.default_file_name(&name),
						name,
						texture: texture.clone(),
						flip: true,
						index: i as u32,
						texture_updated: false,
						db_entry: None,
						exporting: false,
						error: None,
						want_deletion: false,
					}))
				})
				.collect(),
			children_changed: false,
		}
	}

	pub fn read(name: &str, data: &[u8]) -> Self {
		let big_endian = data[0] != b'T';
		let set = txp::Set::from_buf(data, big_endian, None);
		Self {
			big_endian,
			modern: false,
			signature: 0x00,
			filename: Some(name.to_string()),
			children: set
				.textures()
				.enumerate()
				.map(|(i, texture)| {
					Rc::new(Mutex::new(TextureNode {
						file_dialog: egui_file_dialog::FileDialog::new()
							.show_new_folder_button(false)
							.add_save_extension("JPEG", "jpg")
							.add_save_extension("PNG", "png")
							.add_save_extension("WEBP", "webp")
							.default_save_extension("PNG")
							.add_file_filter_extensions("Images", vec!["dds", "jpg", "png", "webp"])
							.default_file_filter("Images"),
						name: format!("Texture {i}"),
						texture: texture.clone(),
						flip: false,
						index: i as u32,
						texture_updated: false,
						db_entry: None,
						exporting: false,
						error: None,
						want_deletion: false,
					}))
				})
				.collect(),
			children_changed: false,
		}
	}
}

pub struct TextureNode {
	pub name: String,
	pub texture: txp::Texture,
	pub flip: bool,
	pub index: u32,
	pub texture_updated: bool,
	pub db_entry: Option<Rc<Mutex<SprDbEntryNode>>>,
	pub file_dialog: egui_file_dialog::FileDialog,
	pub exporting: bool,
	pub error: Option<String>,
	pub want_deletion: bool,
}

impl TextureNode {
	fn pick_file(&mut self, path: std::path::PathBuf) {
		let extension = path.extension().unwrap_or_default();
		let Some(format) = image::ImageFormat::from_extension(extension) else {
			self.error = Some(format!("Could not determine format of {:?}", path));
			return;
		};

		let mip = self.texture.get_mipmap(0, 0).unwrap();
		if self.exporting {
			let rgba = if self.texture.is_ycbcr() {
				self.texture.decode_ycbcr()
			} else {
				mip.rgba()
			};

			let Some(rgba) = rgba else {
				self.error = Some(String::from("Could not convert texture to RGBA"));
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
				.save_with_format(path, format)
			{
				self.error = Some(format!("Could not save image {e}"));
			}
		} else {
			let Ok(data) = std::fs::read(&path) else {
				self.error = Some(format!("Failed to read {:?}", path));
				return;
			};

			let Ok(image) = image::load(std::io::Cursor::new(data), format) else {
				self.error = Some(format!("Could not read {:?} as image", path));
				return;
			};

			if self.texture.is_ycbcr() {
				let Some(texture) = txp::Texture::encode_ycbcr(
					image.width() as i32,
					image.height() as i32,
					image.flipv().to_rgba8().as_bytes(),
				) else {
					self.error = Some(String::from("Could not encode image"));
					return;
				};

				self.texture = texture;
				self.texture_updated = true;
			} else {
				let mut texture = txp::Texture::new();
				texture.set_has_cube_map(false);
				texture.set_array_size(1);
				texture.set_mipmaps_count(self.texture.mipmaps_count());

				for i in 0..self.texture.mipmaps_count() {
					let scale = 2_u32.pow(i as u32);
					let (width, height) = if scale == 0 {
						(image.width(), image.height())
					} else {
						(image.width() / scale, image.height() / scale)
					};

					if width == 0 || height == 0 {
						texture.set_mipmaps_count(i);
						break;
					}

					let Some(mipmap) = txp::Mipmap::from_rgba(
						width as i32,
						height as i32,
						image
							.flipv()
							.resize(width, height, image::imageops::FilterType::Lanczos3)
							.to_rgba8()
							.as_bytes(),
						mip.format(),
					) else {
						self.error = Some(String::from("Could not encode image"));
						return;
					};

					texture.add_mipmap(&mipmap);
				}
				self.texture = texture;
				self.texture_updated = true;
			}
		}
	}
}

impl TreeNode for TextureNode {
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
		let mip = self.texture.get_mipmap(0, 0).unwrap();
		let mut replacement_texture = None;
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
						ui.label("Size");
					});
					row.col(|ui| {
						ui.label(format!("{}x{}", mip.width(), mip.height()));
					});
				});

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Size");
					});
					row.col(|ui| {
						ui.label(format!("{}x{}", mip.width(), mip.height()));
					});
				});
				if (self.texture.array_size() > 1 || self.texture.mipmaps_count() > 1)
					&& !self.texture.is_ycbcr()
				{
					body.row(height, |mut row| {
						row.col(|ui| {
							ui.label("Array size");
						});
						row.col(|ui| {
							ui.label(format!(
								"{}x{}",
								self.texture.array_size(),
								self.texture.mipmaps_count()
							));
						});
					});
				}

				body.row(height, |mut row| {
					row.col(|ui| {
						ui.label("Format");
					});
					row.col(|ui| {
						let (old_format, selected) = if self.texture.is_ycbcr() {
							(0x90, String::from("YCbCr"))
						} else {
							(mip.format() as u32, format!("{:?}", mip.format()))
						};
						let mut format = old_format;

						egui::ComboBox::from_id_salt("FormatComboBox")
							.selected_text(selected)
							.show_ui(ui, |ui| {
								ui.selectable_value(&mut format, txp::Format::A8 as u32, "A8");
								ui.selectable_value(&mut format, txp::Format::RGB8 as u32, "RGB8");
								ui.selectable_value(
									&mut format,
									txp::Format::RGBA8 as u32,
									"RGBA8",
								);
								ui.selectable_value(&mut format, txp::Format::RGB5 as u32, "RGB5");
								ui.selectable_value(
									&mut format,
									txp::Format::RGB5A1 as u32,
									"RGB5A1",
								);
								ui.selectable_value(
									&mut format,
									txp::Format::RGBA4 as u32,
									"RGBA4",
								);
								ui.selectable_value(
									&mut format,
									txp::Format::BC1 as u32,
									"BC1 (RGB)",
								);
								ui.selectable_value(
									&mut format,
									txp::Format::BC1a as u32,
									"BC1 (RGBA)",
								);
								ui.selectable_value(&mut format, txp::Format::BC2 as u32, "BC2");
								ui.selectable_value(&mut format, txp::Format::BC3 as u32, "BC3");
								ui.selectable_value(
									&mut format,
									txp::Format::BC4 as u32,
									"BC4 (R)",
								);
								ui.selectable_value(
									&mut format,
									txp::Format::BC5 as u32,
									"BC5 (RG)",
								);
								ui.selectable_value(&mut format, 0x90, "YCbCr");
								ui.selectable_value(&mut format, txp::Format::L8 as u32, "L8");
								ui.selectable_value(&mut format, txp::Format::L8A8 as u32, "L8A8");
								ui.selectable_value(&mut format, txp::Format::BC7 as u32, "BC7");
								ui.selectable_value(&mut format, txp::Format::BC6H as u32, "BC6H");
							});

						if format != old_format {
							if format == 0x90 {
								let rgba = mip.rgba().unwrap_or_default();
								replacement_texture =
									txp::Texture::encode_ycbcr(mip.width(), mip.height(), &rgba);
							} else if old_format == 0x90 {
								let rgba = self.texture.decode_ycbcr().unwrap_or_default();
								if let Some(mip) = txp::Mipmap::from_rgba(
									mip.width(),
									mip.height(),
									&rgba,
									unsafe { std::mem::transmute(format) },
								) {
									let mut tex = txp::Texture::new();
									tex.set_has_cube_map(false);
									tex.set_array_size(1);
									tex.set_mipmaps_count(1);
									tex.add_mipmap(&mip);
									replacement_texture = Some(tex);
								}
							} else {
								let mut tex = txp::Texture::new();
								tex.set_has_cube_map(self.texture.has_cube_map());
								tex.set_array_size(self.texture.array_size());
								tex.set_mipmaps_count(self.texture.mipmaps_count());
								for mip in self.texture.mipmaps() {
									let rgba = mip.rgba().unwrap_or_default();
									if let Some(mip) = txp::Mipmap::from_rgba(
										mip.width(),
										mip.height(),
										&rgba,
										unsafe { std::mem::transmute(format) },
									) {
										tex.add_mipmap(&mip);
									}
								}
								replacement_texture = Some(tex);
							}
						}
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

		if let Some(tex) = replacement_texture {
			self.texture = tex;
			self.texture_updated = true;
		}
	}

	fn selected(&mut self, frame: &mut eframe::Frame) {
		let render_state = frame.wgpu_render_state().unwrap();

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

		let spr_info = SpriteInfo {
			matrix: crate::aet::Mat4::default().into(),
			tex_coords: [
				[0.0, 1.0, 0.0, 0.0],
				[1.0, 1.0, 0.0, 0.0],
				[0.0, 0.0, 0.0, 0.0],
				[1.0, 0.0, 0.0, 0.0],
			],
			color: [1.0, 1.0, 1.0, 1.0],
			texture_index: self.index,
			is_ycbcr: if self.texture.is_ycbcr() { 1 } else { 0 },
			padding: 0,
		};

		render_state.queue.write_buffer(
			&resources.uniform_buffers[0].0,
			0,
			bytemuck::cast_slice(&[spr_info]),
		);
	}

	fn display_visual(
		&mut self,
		_ui: &mut egui::Ui,
		rect: egui::Rect,
	) -> Option<egui::epaint::PaintCallback> {
		let mip = self.texture.get_mipmap(0, 0).unwrap();

		let w = rect.max.x - rect.min.x;
		let h = rect.max.y - rect.min.y;
		let ar = w / h;
		let mip_ar = mip.width() as f32 / mip.height() as f32;
		let rect = if ar > mip_ar {
			let adjusted_w = h / mip.height() as f32 * mip.width() as f32;
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
			let adjusted_h = w / mip.width() as f32 * mip.height() as f32;
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

		Some(egui_wgpu::Callback::new_paint_callback(
			rect,
			WgpuTextureCallback {},
		))
	}
}

struct WgpuTextureCallback {}

impl egui_wgpu::CallbackTrait for WgpuTextureCallback {
	fn paint(
		&self,
		_info: egui::PaintCallbackInfo,
		render_pass: &mut wgpu::RenderPass<'static>,
		callback_resources: &egui_wgpu::CallbackResources,
	) {
		let resources: &WgpuRenderResources = callback_resources.get().unwrap();
		let texture: &WgpuRenderTextures = callback_resources.get().unwrap();
		render_pass.set_pipeline(&resources.pipeline_normal);
		render_pass.set_bind_group(0, &texture.fragment_bind_group, &[]);
		render_pass.set_bind_group(1, &resources.uniform_buffers[0].1, &[]);
		render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
		render_pass.draw(0..6, 0..1);
	}
}

pub struct WgpuRenderResources {
	pub pipeline_normal: wgpu::RenderPipeline,
	pub pipeline_screen: wgpu::RenderPipeline,
	pub pipeline_add: wgpu::RenderPipeline,
	// Multiply and overlay currently unimplemented
	pub fragment_bind_group_layout: wgpu::BindGroupLayout,
	pub uniform_bind_group_layout: wgpu::BindGroupLayout,
	pub vertex_buffer: wgpu::Buffer,
	pub uniform_buffers: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
	pub sampler: wgpu::Sampler,
}

pub struct WgpuRenderTextures {
	pub fragment_bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
	pub position: [f32; 2],
	pub tex_index: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInfo {
	pub matrix: [[f32; 4]; 4],
	pub tex_coords: [[f32; 4]; 4],
	pub color: [f32; 4],
	pub texture_index: u32,
	pub is_ycbcr: u32,
	pub padding: u64,
}

pub fn setup_wgpu(render_state: &egui_wgpu::RenderState) {
	let device = &render_state.device;

	let fragment_bind_group_layout =
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						multisampled: false,
						view_dimension: wgpu::TextureViewDimension::D2,
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
					},
					count: std::num::NonZeroU32::new(256),
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
			label: Some("Fragment bind group layout"),
		});

	let uniform_bind_group_layout =
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
			label: Some("Uniform bind group layout"),
		});

	let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

	let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Texture Render Pipeline Layout"),
		bind_group_layouts: &[&fragment_bind_group_layout, &uniform_bind_group_layout],
		push_constant_ranges: &[],
	});

	let normal_blend_mode = wgpu::BlendState {
		color: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::SrcAlpha,
			dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
			operation: wgpu::BlendOperation::Add,
		},
		alpha: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::Zero,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		},
	};

	let screen_blend_mode = wgpu::BlendState {
		color: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::SrcAlpha,
			dst_factor: wgpu::BlendFactor::OneMinusSrc,
			operation: wgpu::BlendOperation::Add,
		},
		alpha: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::Zero,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		},
	};

	let add_blend_mode = wgpu::BlendState {
		color: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::SrcAlpha,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		},
		alpha: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::Zero,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		},
	};

	// Combiner 1
	let _multiply_blend_mode = wgpu::BlendState {
		color: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::Dst,
			dst_factor: wgpu::BlendFactor::Zero,
			operation: wgpu::BlendOperation::Add,
		},
		alpha: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::Zero,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		},
	};

	// Combiner 2
	let _overlay_blend_mode = wgpu::BlendState {
		color: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::SrcAlpha,
			dst_factor: wgpu::BlendFactor::OneMinusSrc,
			operation: wgpu::BlendOperation::Add,
		},
		alpha: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::Zero,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		},
	};

	let mut target = wgpu::ColorTargetState {
		format: render_state.target_format,
		blend: Some(normal_blend_mode),
		write_mask: wgpu::ColorWrites::ALL,
	};

	let mut pipeline_desc = wgpu::RenderPipelineDescriptor {
		label: Some("Normal blend mode"),
		layout: Some(&pipeline_layout),
		vertex: wgpu::VertexState {
			module: &shader,
			entry_point: Some("vs_main"),
			buffers: &[wgpu::VertexBufferLayout {
				array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
				step_mode: wgpu::VertexStepMode::Vertex,
				attributes: &wgpu::vertex_attr_array![
					0 => Float32x2,
					1 => Uint32,
				],
			}],
			compilation_options: wgpu::PipelineCompilationOptions::default(),
		},
		fragment: Some(wgpu::FragmentState {
			module: &shader,
			entry_point: Some("fs_main"),
			targets: &[Some(target.clone())],
			compilation_options: wgpu::PipelineCompilationOptions::default(),
		}),
		primitive: wgpu::PrimitiveState {
			topology: wgpu::PrimitiveTopology::TriangleList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: None,
			polygon_mode: wgpu::PolygonMode::Fill,
			unclipped_depth: true,
			conservative: false,
		},
		depth_stencil: None,
		multisample: wgpu::MultisampleState {
			count: 1,
			mask: !0,
			alpha_to_coverage_enabled: false,
		},
		multiview: None,
		cache: None,
	};

	let pipeline_normal = device.create_render_pipeline(&pipeline_desc);

	target.blend = Some(screen_blend_mode);
	let target_arr = [Some(target.clone())];
	pipeline_desc.fragment.as_mut().unwrap().targets = &target_arr;
	pipeline_desc.label = Some("Screen blend mode");

	let pipeline_screen = device.create_render_pipeline(&pipeline_desc);

	target.blend = Some(add_blend_mode);
	let target_arr = [Some(target.clone())];
	pipeline_desc.fragment.as_mut().unwrap().targets = &target_arr;
	pipeline_desc.label = Some("Add blend mode");

	let pipeline_add = device.create_render_pipeline(&pipeline_desc);

	let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Vertex buffer"),
		contents: bytemuck::cast_slice(&[
			Vertex {
				position: [1.0, 1.0],
				tex_index: 1,
			},
			Vertex {
				position: [-1.0, -1.0],
				tex_index: 2,
			},
			Vertex {
				position: [1.0, -1.0],
				tex_index: 3,
			},
			Vertex {
				position: [-1.0, 1.0],
				tex_index: 0,
			},
			Vertex {
				position: [-1.0, -1.0],
				tex_index: 2,
			},
			Vertex {
				position: [1.0, 1.0],
				tex_index: 3,
			},
		]),
		usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
	});

	let base_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Uniform buffer 0"),
		contents: bytemuck::cast_slice(&[SpriteInfo {
			matrix: crate::aet::Mat4::default().into(),
			tex_coords: [
				[0.0, 0.0, 0.0, 0.0],
				[1.0, 0.0, 0.0, 0.0],
				[0.0, 1.0, 0.0, 0.0],
				[1.0, 1.0, 0.0, 0.0],
			],
			color: [1.0, 1.0, 1.0, 1.0],
			texture_index: 0,
			is_ycbcr: 0,
			padding: 0,
		}]),
		usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
	});

	let uniform_buffer_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		layout: &uniform_bind_group_layout,
		entries: &[wgpu::BindGroupEntry {
			binding: 0,
			resource: base_uniform_buffer.as_entire_binding(),
		}],
		label: Some("Uniform bind group 0"),
	});

	let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Linear,
		min_filter: wgpu::FilterMode::Linear,
		mipmap_filter: wgpu::FilterMode::Nearest,
		..Default::default()
	});

	render_state
		.renderer
		.write()
		.callback_resources
		.insert(WgpuRenderResources {
			pipeline_normal,
			pipeline_screen,
			pipeline_add,
			fragment_bind_group_layout,
			uniform_bind_group_layout,
			vertex_buffer,
			uniform_buffers: vec![(base_uniform_buffer, uniform_buffer_group)],
			sampler,
		});
}
