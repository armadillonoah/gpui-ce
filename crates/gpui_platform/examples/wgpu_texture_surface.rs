//! Live proof that an application texture created on GPUI's WGPU device can be
//! composited by GPUI without a copy or a second device. Pass `--two-windows`
//! to verify that separate native windows expose the same device and queue.

use std::sync::Arc;

use gpui::{
    App, AppContext as _, Bounds, Context, DevicePixels, IntoElement, ObjectFit, Pixels, Render,
    Size, SurfaceSource, Window, WindowBounds, WindowOptions, prelude::*, px, size, surface,
};
use gpui_platform::application;
use gpui_wgpu::{WgpuDeviceRequirements, wgpu};

const TEXTURE_WIDTH: u32 = 256;
const TEXTURE_HEIGHT: u32 = 256;

struct TextureSurface {
    texture: Arc<wgpu::Texture>,
    last_geometry: Option<(f32, Size<Pixels>)>,
}

impl Render for TextureSurface {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = (window.scale_factor(), window.viewport_size());
        if self.last_geometry != Some(geometry) {
            eprintln!(
                "Texture surface: scale={}, viewport={:?}",
                geometry.0, geometry.1
            );
            self.last_geometry = Some(geometry);
        }
        surface(SurfaceSource::Texture {
            texture: self.texture.clone(),
            size: size(
                DevicePixels(TEXTURE_WIDTH as i32),
                DevicePixels(TEXTURE_HEIGHT as i32),
            ),
        })
        .object_fit(ObjectFit::Contain)
        .size_full()
    }
}

fn checkerboard() -> Vec<u8> {
    let mut pixels = vec![0; (TEXTURE_WIDTH * TEXTURE_HEIGHT * 4) as usize];
    for y in 0..TEXTURE_HEIGHT {
        for x in 0..TEXTURE_WIDTH {
            let offset = ((y * TEXTURE_WIDTH + x) * 4) as usize;
            let bright = ((x / 32) + (y / 32)).is_multiple_of(2);
            let color = if bright {
                [0x23, 0xc5, 0x5e, 0xff]
            } else {
                [0x12, 0x1a, 0x2b, 0xff]
            };
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
    pixels
}

fn main() {
    let window_count = if std::env::args().any(|arg| arg == "--two-windows") {
        2
    } else {
        1
    };
    application().run(move |cx: &mut App| {
        cx.set_gpu_requirements(Box::new(WgpuDeviceRequirements {
            features: wgpu::Features::empty(),
            optional_features: wgpu::Features::TEXTURE_COMPRESSION_BC
                | wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D,
            limits: wgpu::Limits::default(),
            ..WgpuDeviceRequirements::default()
        }));
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let mut first_context: Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> = None;
        for index in 0..window_count {
            let mut bounds = Bounds::centered(None, size(px(640.0), px(480.0)), cx);
            bounds.origin.x += px(index as f32 * 80.0);
            bounds.origin.y += px(index as f32 * 60.0);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let context = window
                        .gpu_context()
                        .expect("the WGPU platform must expose its device and queue");
                    let (device, queue) = *context
                        .downcast::<(Arc<wgpu::Device>, Arc<wgpu::Queue>)>()
                        .expect("the GPU context must contain WGPU handles");
                    if let Some((first_device, first_queue)) = &first_context {
                        assert!(
                            Arc::ptr_eq(first_device, &device),
                            "windows must share the device"
                        );
                        assert!(
                            Arc::ptr_eq(first_queue, &queue),
                            "windows must share the queue"
                        );
                        eprintln!("Two native windows share the same WGPU device and queue");
                    } else {
                        first_context = Some((device.clone(), queue.clone()));
                    }
                    eprintln!("Texture surface GPU: {:?}", window.gpu_specs());
                    eprintln!("Texture surface enabled features: {:?}", device.features());
                    let texture = Arc::new(device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("same-device texture surface example"),
                        size: wgpu::Extent3d {
                            width: TEXTURE_WIDTH,
                            height: TEXTURE_HEIGHT,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    }));
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &checkerboard(),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(TEXTURE_WIDTH * 4),
                            rows_per_image: Some(TEXTURE_HEIGHT),
                        },
                        wgpu::Extent3d {
                            width: TEXTURE_WIDTH,
                            height: TEXTURE_HEIGHT,
                            depth_or_array_layers: 1,
                        },
                    );
                    cx.new(|_| TextureSurface {
                        texture,
                        last_geometry: None,
                    })
                },
            )
            .expect("opening the WGPU texture-surface example window");
        }
        cx.activate(true);
    });
}
