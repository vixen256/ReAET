pub mod aet;
pub mod app;
pub mod movie;
pub mod spr;
pub mod spr_db;
pub mod txp;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
	use eframe::egui_wgpu::*;

	let native_options = eframe::NativeOptions {
		viewport: eframe::egui::ViewportBuilder::default()
			.with_inner_size((1280.0, 720.0))
			.with_drag_and_drop(true),
		multisampling: 4,
		renderer: eframe::Renderer::Wgpu,
		wgpu_options: WgpuConfiguration {
			desired_maximum_frame_latency: Some(1),
			wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
				power_preference: wgpu::PowerPreference::HighPerformance,
				device_descriptor: std::sync::Arc::new(|adapter| wgpu::DeviceDescriptor {
					label: Some("egui wgpu device"),
					required_features: wgpu::Features::TEXTURE_COMPRESSION_BC
						| wgpu::Features::DEPTH_CLIP_CONTROL,
					required_limits: wgpu::Limits {
						min_uniform_buffer_offset_alignment: adapter
							.limits()
							.min_uniform_buffer_offset_alignment,
						..Default::default()
					},
					..Default::default()
				}),
				..Default::default()
			}),
			..Default::default()
		},
		..Default::default()
	};
	eframe::run_native(
		"ReAET",
		native_options,
		Box::new(|cc| Ok(Box::new(app::App::new(cc).unwrap()))),
	)
	.unwrap();
}
