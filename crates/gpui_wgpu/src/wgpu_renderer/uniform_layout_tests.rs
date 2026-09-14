use super::*;
use std::mem::{offset_of, size_of};

fn assert_layout<T>(module: &naga::Module, name: &str, fields: &[(&str, usize)]) {
    let shader_type = module
        .types
        .iter()
        .find_map(|(_, ty)| (ty.name.as_deref() == Some(name)).then_some(ty))
        .unwrap_or_else(|| panic!("shader has no {name} type"));
    let naga::TypeInner::Struct { members, span } = &shader_type.inner else {
        panic!("{name} must be a shader struct");
    };
    assert_eq!(
        usize::try_from(*span).unwrap(),
        size_of::<T>(),
        "{name} size"
    );
    assert_eq!(members.len(), fields.len(), "{name} field count");
    for (member, &(field, offset)) in members.iter().zip(fields) {
        assert_eq!(member.name.as_deref(), Some(field), "{name} field order");
        assert_eq!(
            usize::try_from(member.offset).unwrap(),
            offset,
            "{name}.{field} offset"
        );
    }
}

#[test]
fn rounded_uniform_layouts_match_the_shader() {
    let module = naga::front::wgsl::parse_str(include_str!("../shaders.wgsl"))
        .expect("the production shader must parse");
    assert_layout::<PodBounds>(
        &module,
        "Bounds",
        &[
            ("origin", offset_of!(PodBounds, origin)),
            ("size", offset_of!(PodBounds, size)),
        ],
    );
    assert_layout::<PodCorners>(
        &module,
        "Corners",
        &[
            ("top_left", offset_of!(PodCorners, top_left)),
            ("top_right", offset_of!(PodCorners, top_right)),
            ("bottom_right", offset_of!(PodCorners, bottom_right)),
            ("bottom_left", offset_of!(PodCorners, bottom_left)),
        ],
    );
    assert_layout::<PodContentMask>(
        &module,
        "ContentMask",
        &[
            ("bounds", offset_of!(PodContentMask, bounds)),
            ("corner_radii", offset_of!(PodContentMask, corner_radii)),
        ],
    );
    assert_layout::<SurfaceParams>(
        &module,
        "SurfaceParams",
        &[
            ("bounds", offset_of!(SurfaceParams, bounds)),
            ("content_mask", offset_of!(SurfaceParams, content_mask)),
            ("corner_radii", offset_of!(SurfaceParams, corner_radii)),
        ],
    );
    assert_layout::<BlurParams>(
        &module,
        "BlurParams",
        &[
            ("bounds", offset_of!(BlurParams, bounds)),
            ("content_mask", offset_of!(BlurParams, content_mask)),
            ("corner_radii", offset_of!(BlurParams, corner_radii)),
            ("direction", offset_of!(BlurParams, direction)),
            ("sigma", offset_of!(BlurParams, sigma)),
            ("opacity", offset_of!(BlurParams, opacity)),
            ("tap_count", offset_of!(BlurParams, tap_count)),
            ("tap_step", offset_of!(BlurParams, tap_step)),
            ("clip_rounded", offset_of!(BlurParams, clip_rounded)),
            ("downsample", offset_of!(BlurParams, downsample)),
        ],
    );
}

#[test]
fn subpixel_sprite_layout_matches_the_shader() {
    let source = format!(
        "enable dual_source_blending;\n{}\n{}",
        include_str!("../shaders.wgsl"),
        include_str!("../shaders_subpixel.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&source)
        .expect("the production dual-source shader must parse");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("the production dual-source shader must validate");
    assert_layout::<SubpixelSprite>(
        &module,
        "SubpixelSprite",
        &[
            ("order", offset_of!(SubpixelSprite, order)),
            ("pad", offset_of!(SubpixelSprite, pad)),
            ("bounds", offset_of!(SubpixelSprite, bounds)),
            ("content_mask", offset_of!(SubpixelSprite, content_mask)),
            ("color", offset_of!(SubpixelSprite, color)),
            ("tile", offset_of!(SubpixelSprite, tile)),
            ("transformation", offset_of!(SubpixelSprite, transformation)),
        ],
    );
}

#[test]
fn mask_upload_preserves_bounds_and_each_corner_radius() {
    let mask = ContentMask {
        bounds: Bounds {
            origin: gpui::point(ScaledPixels(5.0), ScaledPixels(7.0)),
            size: gpui::size(ScaledPixels(101.0), ScaledPixels(103.0)),
        },
        corner_radii: Corners {
            top_left: ScaledPixels(11.0),
            top_right: ScaledPixels(13.0),
            bottom_right: ScaledPixels(17.0),
            bottom_left: ScaledPixels(19.0),
        },
    };
    let uploaded = PodContentMask::from(mask);
    assert_eq!(uploaded.bounds.origin, [5.0, 7.0]);
    assert_eq!(uploaded.bounds.size, [101.0, 103.0]);
    assert_eq!(uploaded.corner_radii.top_left, 11.0);
    assert_eq!(uploaded.corner_radii.top_right, 13.0);
    assert_eq!(uploaded.corner_radii.bottom_right, 17.0);
    assert_eq!(uploaded.corner_radii.bottom_left, 19.0);
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn production_pipelines_validate_without_a_window() -> anyhow::Result<()> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        let dual_source_blending = adapter
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);
        let required_features = if dual_source_blending {
            wgpu::Features::DUAL_SOURCE_BLENDING
        } else {
            wgpu::Features::empty()
        };
        eprintln!(
            "Pipeline validation adapter: {} ({:?}), dual-source blending: {dual_source_blending}",
            adapter.get_info().name,
            adapter.get_info().backend
        );
        let (device, _queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("uniform layout regression device"),
                required_features,
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits())
                    .using_alignment(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await?;
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layouts = WgpuRenderer::create_bind_group_layouts(&device);
        for use_dual_source in [false, true]
            .into_iter()
            .filter(|enabled| !enabled || dual_source_blending)
        {
            let _pipelines = WgpuRenderer::create_pipelines(
                &device,
                &layouts,
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::CompositeAlphaMode::PreMultiplied,
                1,
                use_dual_source,
            );
        }
        if let Some(error) = error_scope.pop().await {
            anyhow::bail!("production pipeline validation failed: {error}");
        }
        Ok(())
    })
}
