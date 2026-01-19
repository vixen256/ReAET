use eframe::egui;
use eframe::egui_wgpu;
use eframe::egui_wgpu::wgpu;
use eframe::egui_wgpu::wgpu::util::DeviceExt;
use ffmpeg_sys_next::*;
use std::collections::VecDeque;
use std::ffi::CString;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone)]
pub struct Movie {
	pub input_ctx: *mut AVFormatContext,
	pub stream_index: i32,
	pub video: *mut AVStream,
	pub decoder_ctx: *mut AVCodecContext,
	pub buffered_frames: VecDeque<MovieFrame>,
	pub dont_seek: bool,
}

impl Drop for Movie {
	fn drop(&mut self) {
		unsafe {
			avcodec_free_context(&mut self.decoder_ctx);
			avformat_free_context(self.input_ctx);
		}
	}
}

fn error_to_string(err: i32) -> String {
	let mut buf = vec![0u8; 256];
	unsafe {
		av_strerror(err, buf.as_mut_ptr() as *mut std::ffi::c_char, 255);
	}
	buf.resize(
		unsafe { strlen(buf.as_ptr() as *const std::ffi::c_char) } as usize,
		0,
	);
	CString::new(buf)
		.unwrap_or_default()
		.to_string_lossy()
		.to_string()
}

impl Movie {
	pub fn open<P: AsRef<Path> + ?Sized>(
		path: &P,
		render_state: &egui_wgpu::RenderState,
	) -> Result<Self, String> {
		unsafe {
			#[cfg(debug_assertions)]
			av_log_set_level(AV_LOG_WARNING);

			let mut input_ctx = avformat_alloc_context();

			let url = CString::from_str(&path.as_ref().to_string_lossy().to_string()).unwrap();
			let res = avformat_open_input(
				&mut input_ctx,
				url.as_ptr(),
				std::ptr::null(),
				std::ptr::null_mut(),
			);
			if res < 0 || input_ctx.is_null() {
				return Err(format!("Failed to open input {}", error_to_string(res)));
			}

			let res = avformat_find_stream_info(input_ctx, std::ptr::null_mut());
			if res < 0 {
				avformat_free_context(input_ctx);
				return Err(format!("Failed to find stream {}", error_to_string(res)));
			}

			let mut decoder = std::ptr::null();
			let stream_index = av_find_best_stream(
				input_ctx,
				AVMediaType::AVMEDIA_TYPE_VIDEO,
				-1,
				-1,
				&mut decoder,
				0,
			);
			if stream_index < 0 || decoder.is_null() {
				avformat_free_context(input_ctx);
				return Err(format!(
					"Failed to find video stream {}",
					error_to_string(stream_index)
				));
			}

			let video = input_ctx.read().streams.add(stream_index as usize).read();
			let mut decoder_ctx = avcodec_alloc_context3(decoder);
			let res = avcodec_parameters_to_context(decoder_ctx, video.read().codecpar);
			if res < 0 {
				avcodec_free_context(&mut decoder_ctx);
				avformat_free_context(input_ctx);
				return Err(format!(
					"Failed to allocate decoder {}",
					error_to_string(res)
				));
			}

			let res = avcodec_open2(decoder_ctx, decoder, std::ptr::null_mut());
			if res < 0 {
				avcodec_free_context(&mut decoder_ctx);
				avformat_free_context(input_ctx);
				return Err(format!("Failed to open decoder {}", error_to_string(res)));
			}

			let device = &render_state.device;
			let y_texture = device.create_texture(&wgpu::TextureDescriptor {
				size: wgpu::Extent3d {
					width: decoder_ctx.read().width as u32,
					height: decoder_ctx.read().height as u32,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::R8Unorm,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
				label: None,
				view_formats: &[],
			});

			let cb_texture = device.create_texture(&wgpu::TextureDescriptor {
				size: wgpu::Extent3d {
					width: decoder_ctx.read().width as u32 / 2,
					height: decoder_ctx.read().height as u32 / 2,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::R8Unorm,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
				label: None,
				view_formats: &[],
			});

			let cr_texture = device.create_texture(&wgpu::TextureDescriptor {
				size: wgpu::Extent3d {
					width: decoder_ctx.read().width as u32 / 2,
					height: decoder_ctx.read().height as u32 / 2,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::R8Unorm,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
				label: None,
				view_formats: &[],
			});

			let mut renderer = render_state.renderer.write();
			let resources: &WgpuResources = renderer.callback_resources.get().unwrap();

			let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
				layout: &resources.bind_group_layout,
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::Sampler(&resources.sampler),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::TextureView(
							&y_texture.create_view(&wgpu::TextureViewDescriptor::default()),
						),
					},
					wgpu::BindGroupEntry {
						binding: 2,
						resource: wgpu::BindingResource::TextureView(
							&cb_texture.create_view(&wgpu::TextureViewDescriptor::default()),
						),
					},
					wgpu::BindGroupEntry {
						binding: 3,
						resource: wgpu::BindingResource::TextureView(
							&cr_texture.create_view(&wgpu::TextureViewDescriptor::default()),
						),
					},
				],
				label: Some("Movie bind group"),
			});

			renderer.callback_resources.insert(WgpuMovie {
				time: -1.0,
				y_texture,
				cb_texture,
				cr_texture,
				bind_group,
			});

			Ok(Self {
				input_ctx,
				stream_index,
				video,
				decoder_ctx,
				buffered_frames: VecDeque::new(),
				dont_seek: true,
			})
		}
	}

	pub fn get_frame(&mut self, ui: &mut egui::Ui, time: f64) -> Option<&MovieFrame> {
		unsafe {
			if time < 0.0
				|| time + 0.1 >= self.input_ctx.read().duration as f64 * av_q2d(AV_TIME_BASE_Q)
			{
				return None;
			}

			if let Some(front) = self.buffered_frames.front()
				&& let Some(back) = self.buffered_frames.back()
			{
				if time < front.time || time > back.time + 0.5 && !self.dont_seek {
					avformat_seek_file(
						self.input_ctx,
						self.stream_index,
						((time - 0.5).max(0.0) / av_q2d(self.video.read().time_base)) as i64,
						(time / av_q2d(self.video.read().time_base)) as i64,
						(time / av_q2d(self.video.read().time_base)) as i64,
						0,
					);
					self.dont_seek = true;
					self.buffered_frames.clear();
				} else if back.time > time && front.time < time {
					return self
						.buffered_frames
						.iter()
						.rev()
						.skip_while(|frame| frame.time > time)
						.next();
				}
			}

			self.buffered_frames
				.retain_mut(|frame| frame.time + 1.0 >= time && frame.time - 1.0 <= time);

			for _ in 0..10 {
				let mut packet = av_packet_alloc();
				let res = av_read_frame(self.input_ctx, packet);
				if res < 0 {
					dbg!(error_to_string(res));
					av_packet_free(&mut packet);
					return None;
				}
				if packet.read().stream_index != self.stream_index {
					av_packet_free(&mut packet);
					continue;
				}

				let res = avcodec_send_packet(self.decoder_ctx, packet);
				if res < 0 && res != AVERROR(EAGAIN) {
					dbg!(error_to_string(res));
					av_packet_free(&mut packet);
					return None;
				}

				let mut frame = av_frame_alloc();
				let res = avcodec_receive_frame(self.decoder_ctx, frame);
				av_packet_free(&mut packet);
				if res < 0 {
					av_frame_free(&mut frame);
					if res == AVERROR(EAGAIN) {
						dbg!(error_to_string(res));
						continue;
					} else {
						dbg!(error_to_string(res));
						return None;
					}
				}

				let frame_time = frame.read().pts as f64 * av_q2d(self.video.read().time_base);

				let frame = MovieFrame {
					time: frame_time,
					frame: Arc::new(RawFrame { frame }),
				};

				self.buffered_frames.push_back(frame);
				if frame_time >= time {
					self.dont_seek = false;
					return self.buffered_frames.back();
				}
			}

			ui.ctx().request_repaint();
			return self.buffered_frames.back();
		}
	}
}

pub struct RawFrame {
	pub frame: *mut AVFrame,
}

unsafe impl Send for RawFrame {}
unsafe impl Sync for RawFrame {}

#[derive(Clone)]
pub struct MovieFrame {
	pub time: f64,
	pub frame: Arc<RawFrame>,
}

impl Drop for RawFrame {
	fn drop(&mut self) {
		unsafe {
			av_frame_free(&mut self.frame);
		}
	}
}

pub struct WgpuMovie {
	pub time: f64,

	pub y_texture: wgpu::Texture,
	pub cb_texture: wgpu::Texture,
	pub cr_texture: wgpu::Texture,
	pub bind_group: wgpu::BindGroup,
}

pub struct WgpuResources {
	pub sampler: wgpu::Sampler,
	pub bind_group_layout: wgpu::BindGroupLayout,
	pub pipeline: wgpu::RenderPipeline,
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
	pub position: [f32; 2],
	pub tex_coords: [f32; 2],
}

pub fn setup_wgpu(render_state: &egui_wgpu::RenderState) {
	let device = &render_state.device;

	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		entries: &[
			wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
				count: None,
			},
			wgpu::BindGroupLayoutEntry {
				binding: 1,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					multisampled: false,
					view_dimension: wgpu::TextureViewDimension::D2,
					sample_type: wgpu::TextureSampleType::Float { filterable: true },
				},
				count: None,
			},
			wgpu::BindGroupLayoutEntry {
				binding: 2,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					multisampled: false,
					view_dimension: wgpu::TextureViewDimension::D2,
					sample_type: wgpu::TextureSampleType::Float { filterable: true },
				},
				count: None,
			},
			wgpu::BindGroupLayoutEntry {
				binding: 3,
				visibility: wgpu::ShaderStages::FRAGMENT,
				ty: wgpu::BindingType::Texture {
					multisampled: false,
					view_dimension: wgpu::TextureViewDimension::D2,
					sample_type: wgpu::TextureSampleType::Float { filterable: true },
				},
				count: None,
			},
		],
		label: Some("Movie bind group layout"),
	});

	let shader = device.create_shader_module(wgpu::include_wgsl!("movie.wgsl"));

	let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Texture Render Pipeline Layout"),
		bind_group_layouts: &[&bind_group_layout],
		push_constant_ranges: &[],
	});

	let pipeline_desc = wgpu::RenderPipelineDescriptor {
		label: Some("Movie"),
		layout: Some(&pipeline_layout),
		vertex: wgpu::VertexState {
			module: &shader,
			entry_point: Some("vs_main"),
			buffers: &[wgpu::VertexBufferLayout {
				array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
				step_mode: wgpu::VertexStepMode::Vertex,
				attributes: &wgpu::vertex_attr_array![
					0 => Float32x2,
					1 => Float32x2,
				],
			}],
			compilation_options: wgpu::PipelineCompilationOptions::default(),
		},
		fragment: Some(wgpu::FragmentState {
			module: &shader,
			entry_point: Some("fs_main"),
			targets: &[Some(wgpu::ColorTargetState {
				format: render_state.target_format,
				blend: Some(wgpu::BlendState {
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
				}),
				write_mask: wgpu::ColorWrites::ALL,
			})],
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
			count: 4,
			mask: !0,
			alpha_to_coverage_enabled: false,
		},
		multiview: None,
		cache: None,
	};

	let pipeline = device.create_render_pipeline(&pipeline_desc);

	let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Vertex buffer"),
		contents: bytemuck::cast_slice(&[
			Vertex {
				position: [-1.0, 1.0],
				tex_coords: [0.0, 0.0],
			},
			Vertex {
				position: [1.0, 1.0],
				tex_coords: [1.0, 0.0],
			},
			Vertex {
				position: [-1.0, -1.0],
				tex_coords: [0.0, 1.0],
			},
			Vertex {
				position: [1.0, -1.0],
				tex_coords: [1.0, 1.0],
			},
		]),
		usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
	});

	let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Index buffer"),
		contents: bytemuck::cast_slice(&[1u32, 2u32, 3u32, 0u32, 2u32, 1u32]),
		usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDEX,
	});

	let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		mag_filter: wgpu::FilterMode::Linear,
		min_filter: wgpu::FilterMode::Linear,
		..Default::default()
	});

	render_state
		.renderer
		.write()
		.callback_resources
		.insert(WgpuResources {
			sampler,
			bind_group_layout,
			pipeline,
			vertex_buffer,
			index_buffer,
		});
}

impl egui_wgpu::CallbackTrait for MovieFrame {
	fn prepare(
		&self,
		_device: &wgpu::Device,
		queue: &wgpu::Queue,
		_screen_descriptor: &egui_wgpu::ScreenDescriptor,
		_egui_encoder: &mut wgpu::CommandEncoder,
		callback_resources: &mut egui_wgpu::CallbackResources,
	) -> Vec<wgpu::CommandBuffer> {
		let wgpu_movie: &mut WgpuMovie = callback_resources.get_mut().unwrap();

		if wgpu_movie.time != self.time {
			queue.write_texture(
				wgpu::TexelCopyTextureInfo {
					texture: &wgpu_movie.y_texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
					aspect: wgpu::TextureAspect::All,
				},
				unsafe {
					std::slice::from_raw_parts(
						self.frame.frame.read().data[0],
						self.frame.frame.read().linesize[0] as usize
							* self.frame.frame.read().height as usize,
					)
				},
				wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(unsafe { self.frame.frame.read().linesize[0] as u32 }),
					rows_per_image: Some(wgpu_movie.y_texture.height()),
				},
				wgpu_movie.y_texture.size(),
			);

			queue.write_texture(
				wgpu::TexelCopyTextureInfo {
					texture: &wgpu_movie.cb_texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
					aspect: wgpu::TextureAspect::All,
				},
				unsafe {
					std::slice::from_raw_parts(
						self.frame.frame.read().data[1],
						self.frame.frame.read().linesize[1] as usize
							* (self.frame.frame.read().height as usize / 2),
					)
				},
				wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(unsafe { self.frame.frame.read().linesize[1] as u32 }),
					rows_per_image: Some(wgpu_movie.cb_texture.height()),
				},
				wgpu_movie.cb_texture.size(),
			);

			queue.write_texture(
				wgpu::TexelCopyTextureInfo {
					texture: &wgpu_movie.cr_texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
					aspect: wgpu::TextureAspect::All,
				},
				unsafe {
					std::slice::from_raw_parts(
						self.frame.frame.read().data[2],
						self.frame.frame.read().linesize[2] as usize
							* (self.frame.frame.read().height as usize / 2),
					)
				},
				wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(unsafe { self.frame.frame.read().linesize[2] as u32 }),
					rows_per_image: Some(wgpu_movie.cr_texture.height()),
				},
				wgpu_movie.cr_texture.size(),
			);

			wgpu_movie.time = self.time;
		}

		Vec::new()
	}

	fn paint(
		&self,
		_info: eframe::egui::PaintCallbackInfo,
		render_pass: &mut wgpu::RenderPass<'static>,
		callback_resources: &egui_wgpu::CallbackResources,
	) {
		let resources: &WgpuResources = callback_resources.get().unwrap();
		let wgpu_movie: &WgpuMovie = callback_resources.get().unwrap();
		render_pass.set_pipeline(&resources.pipeline);
		render_pass.set_bind_group(0, &wgpu_movie.bind_group, &[]);
		render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
		render_pass.set_index_buffer(resources.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		render_pass.draw_indexed(0..6, 0, 0..1);
	}
}
