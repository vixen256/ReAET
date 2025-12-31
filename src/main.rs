pub mod aet;
pub mod app;
pub mod spr;
pub mod spr_db;
pub mod txp;

fn main() {
	use eframe::egui_wgpu::*;

	let native_options = eframe::NativeOptions {
		viewport: eframe::egui::ViewportBuilder::default()
			.with_inner_size((1280.0, 720.0))
			.with_drag_and_drop(true),
		renderer: eframe::Renderer::Wgpu,
		wgpu_options: WgpuConfiguration {
			wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
				device_descriptor: std::sync::Arc::new(|_| wgpu::DeviceDescriptor {
					label: Some("egui wgpu device"),
					required_features: wgpu::Features::TEXTURE_COMPRESSION_BC
						| wgpu::Features::DEPTH_CLIP_CONTROL,
					memory_hints: wgpu::MemoryHints::MemoryUsage,
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
