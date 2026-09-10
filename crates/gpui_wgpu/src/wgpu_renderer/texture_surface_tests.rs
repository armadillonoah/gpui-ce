//! Metal readback of the production texture-surface compositor. CAMetalLayers
//! stay offscreen, so these tests do not require an AppKit window or event loop.

use super::*;
use metal::foreign_types::ForeignType as _;
use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle};

#[derive(Debug)]
struct AppKitDisplay;

impl HasDisplayHandle for AppKitDisplay {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::appkit())
    }
}

fn surface(instance: &wgpu::Instance, layer: &metal::MetalLayer) -> wgpu::Surface<'static> {
    // SAFETY: Each caller keeps its owned CAMetalLayer alive until after both
    // the surface and the renderer holding it have been dropped.
    unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                layer.as_ptr().cast(),
            ))
            .expect("creating an offscreen Metal surface")
    }
}

fn renderer(
    context: &WgpuContext,
    surface: wgpu::Surface<'static>,
    width: i32,
    height: i32,
) -> anyhow::Result<WgpuRenderer> {
    WgpuRenderer::new_internal(
        None,
        context,
        surface,
        WgpuSurfaceConfig {
            size: gpui::size(DevicePixels(width), DevicePixels(height)),
            transparent: false,
            preferred_present_mode: None,
        },
        None,
        None,
        Arc::new(WgpuAtlas::from_context(context)),
    )
}

fn texture(context: &WgpuContext, color: [u8; 4]) -> Arc<wgpu::Texture> {
    let texture = Arc::new(context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("same-device surface test source"),
        size: wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    }));
    context.queue.write_texture(
        texture.as_image_copy(),
        &color.repeat(4),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(2),
        },
        texture.size(),
    );
    texture
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: gpui::point(ScaledPixels(x), ScaledPixels(y)),
        size: gpui::size(ScaledPixels(width), ScaledPixels(height)),
    }
}

fn paint_surface(
    texture: Arc<wgpu::Texture>,
    bounds: Bounds<ScaledPixels>,
    mask: Bounds<ScaledPixels>,
) -> PaintSurface {
    PaintSurface {
        order: 0,
        bounds,
        content_mask: gpui::ContentMask { bounds: mask },
        corner_radii: gpui::Corners::default(),
        texture,
        texture_size: gpui::size(DevicePixels(2), DevicePixels(2)),
    }
}

/// Render scene surface batches through the real compositor pipeline into a
/// copyable attachment. The current-pin renderer has no headless frame API;
/// this deliberately tests draw_surfaces rather than claiming swapchain proof.
fn readback(renderer: &mut WgpuRenderer, scene: &Scene) -> anyhow::Result<Vec<[u8; 4]>> {
    let width = renderer.surface_config.width;
    let height = renderer.surface_config.height;
    let format = renderer.surface_config.format;
    anyhow::ensure!(matches!(
        format,
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
    ));
    assert!(renderer.ensure_surface_params_capacity(scene.surfaces.len() as u64));
    renderer.surface_params_slot.set(0);
    let resources = renderer.resources();
    let device = &resources.device;
    let queue = &resources.queue;
    let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("surface compositor readback target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let bytes_per_row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("surface compositor readback buffer"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    queue.write_buffer(
        &resources.globals_buffer,
        0,
        bytemuck::bytes_of(&GlobalParams {
            viewport_size: [width as f32, height as f32],
            premultiplied_alpha: 0,
            pad: 0,
        }),
    );
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("production texture-surface compositor"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Surfaces(surfaces) => {
                    assert!(renderer.draw_surfaces(&scene.surfaces[surfaces], &mut pass));
                }
                _ => anyhow::bail!("test scenes must contain only texture surfaces"),
            }
        }
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).expect("readback test receiver exists");
        });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    receiver.recv()??;
    anyhow::ensure!(pollster::block_on(error_scope.pop()).is_none());
    anyhow::ensure!(renderer.last_error.lock().unwrap().is_none());
    let mapped = buffer.slice(..).get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in mapped.chunks_exact(bytes_per_row as usize) {
        for pixel in row[..width as usize * 4].chunks_exact(4) {
            let mut rgba: [u8; 4] = pixel.try_into()?;
            if format == wgpu::TextureFormat::Bgra8Unorm {
                rgba.swap(0, 2);
            }
            pixels.push(rgba);
        }
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}

#[test]
fn same_device_texture_surface_renders_through_scene_compositor() -> anyhow::Result<()> {
    const GREEN: [u8; 4] = [0x23, 0xc5, 0x5e, 0xff];
    const BLUE: [u8; 4] = [0x12, 0x1a, 0xab, 0xff];
    let first_layer = metal::MetalLayer::new();
    let second_layer = metal::MetalLayer::new();
    let instance = WgpuContext::instance(Box::new(AppKitDisplay));
    let first_surface = surface(&instance, &first_layer);
    let context = WgpuContext::new(instance, &first_surface, None, None)?;
    assert_eq!(context.adapter.get_info().backend, wgpu::Backend::Metal);
    eprintln!(
        "Metal readback adapter: {}",
        context.adapter.get_info().name
    );
    let second_surface = surface(&context.instance, &second_layer);
    context.check_compatible_with_surface(&second_surface)?;
    let mut first_renderer = renderer(&context, first_surface, 4, 4)?;
    let mut second_renderer = renderer(&context, second_surface, 8, 4)?;
    let (first_device, first_queue) = first_renderer.gpu_context();
    let (second_device, second_queue) = second_renderer.gpu_context();
    assert!(Arc::ptr_eq(&first_device, &context.device));
    assert!(Arc::ptr_eq(&first_device, &second_device));
    assert!(Arc::ptr_eq(&first_queue, &second_queue));

    let green = texture(&context, GREEN);
    let blue = texture(&context, BLUE);
    let full = bounds(0.0, 0.0, 4.0, 4.0);
    let mut scene = Scene::default();
    scene.insert_primitive(paint_surface(green.clone(), full, full));
    scene.finish();
    assert_eq!(readback(&mut first_renderer, &scene)?, vec![GREEN; 16]);

    // The same texture is sampled by another renderer on the same device. A
    // second surface validates distinct uniform slots and the content mask.
    let right = bounds(4.0, 0.0, 4.0, 4.0);
    let clipped = bounds(5.0, 1.0, 2.0, 2.0);
    let mut scene = Scene::default();
    scene.insert_primitive(paint_surface(green, full, full));
    scene.insert_primitive(paint_surface(blue, right, clipped));
    scene.finish();
    let actual = readback(&mut second_renderer, &scene)?;
    for y in 0..4 {
        for x in 0..8 {
            let expected = if x < 4 {
                GREEN
            } else if (5..7).contains(&x) && (1..3).contains(&y) {
                BLUE
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(actual[y * 8 + x], expected, "pixel ({x}, {y})");
        }
    }
    Ok(())
}
