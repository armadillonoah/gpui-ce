use super::*;

#[test]
fn validates_subpixel_shader() {
    let generated = subpixel_sprite::WGSL_SOURCE.wgsl_source().unwrap();
    wgsl_rs::validate_wgsl_source(&format!("enable dual_source_blending;\n{generated}")).unwrap();
}

#[test]
fn build_generated_shaders_match_rust_sources() {
    assert_eq!(
        base::WGSL_SOURCE.wgsl_source().unwrap(),
        crate::artifacts::BASE_WGSL
    );
    let subpixel = subpixel_sprite::WGSL_SOURCE.wgsl_source().unwrap();
    assert_eq!(
        format!("enable dual_source_blending;\n{subpixel}"),
        crate::artifacts::SUBPIXEL_DUAL_SOURCE_WGSL
    );
}

#[test]
fn shader_interface_matches_generated_sources() {
    let standard_source = base::WGSL_SOURCE.wgsl_source().unwrap();
    let subpixel_source = subpixel_sprite::WGSL_SOURCE.wgsl_source().unwrap();
    for pipeline in interface::ALL {
        let source = if pipeline.label == interface::SUBPIXEL_SPRITES.label {
            &subpixel_source
        } else {
            &standard_source
        };
        assert!(
            source.contains(&format!("fn {}(", pipeline.vertex_entry)),
            "missing vertex entry point {}",
            pipeline.vertex_entry,
        );
        assert!(
            source.contains(&format!("fn {}(", pipeline.fragment_entry)),
            "missing fragment entry point {}",
            pipeline.fragment_entry,
        );
    }

    assert_eq!(common::GLOBALS.group, interface::GLOBAL_BIND_GROUP);
    assert_eq!(common::GLOBALS.binding, interface::GLOBAL_UNIFORMS_BINDING);
    assert_eq!(
        common::FONT_RASTERIZATION.group,
        interface::GLOBAL_BIND_GROUP
    );
    assert_eq!(
        common::FONT_RASTERIZATION.binding,
        interface::FONT_RASTERIZATION_BINDING
    );
    assert_eq!(quad::QUADS.group(), interface::DATA_BIND_GROUP);
    assert_eq!(quad::QUADS.binding(), interface::DATA_BUFFER_BINDING);
}

fn vertex_output_shape(module: &naga::Module, entry_name: &str) -> (usize, u32) {
    let entry = module
        .entry_points
        .iter()
        .find(|entry| entry.name == entry_name)
        .unwrap_or_else(|| panic!("missing shader entry point {entry_name}"));
    let result = entry
        .function
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("shader entry point {entry_name} has no output"));
    let naga::TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
        panic!("shader entry point {entry_name} must return a struct");
    };

    members
        .iter()
        .filter(|member| matches!(member.binding, Some(naga::Binding::Location { .. })))
        .fold((0, 0), |(locations, components), member| {
            let member_components = match module.types[member.ty].inner {
                naga::TypeInner::Scalar(_) => 1,
                naga::TypeInner::Vector { size, .. } => u32::from(size),
                ref ty => panic!("unsupported stage output type {ty:?} in {entry_name}"),
            };
            (locations + 1, components + member_components)
        })
}

#[test]
fn ordinary_vertex_interfaces_stay_within_compact_budgets() {
    let module = naga::front::wgsl::parse_str(&base::WGSL_SOURCE.wgsl_source().unwrap()).unwrap();
    for (entry, maximum_shape) in [
        ("vertex_quad", (8, 29)),
        ("vertex_shadow", (5, 17)),
        ("vertex_polychrome_sprite", (3, 7)),
        ("vertex_blur_composite", (2, 6)),
    ] {
        let actual_shape = vertex_output_shape(&module, entry);
        assert!(
            actual_shape.0 <= maximum_shape.0 && actual_shape.1 <= maximum_shape.1,
            "{entry} uses {actual_shape:?}, exceeding {maximum_shape:?} (locations, components)"
        );
    }
}

#[test]
fn scene_instance_storage_sizes_are_bounded() {
    for (name, actual, maximum) in [
        ("Quad", std::mem::size_of::<gpui::Quad>(), 296),
        ("Shadow", std::mem::size_of::<gpui::Shadow>(), 216),
        (
            "PolychromeSprite",
            std::mem::size_of::<gpui::PolychromeSprite>(),
            128,
        ),
    ] {
        assert!(
            actual <= maximum,
            "{name} is {actual} bytes, exceeding its {maximum}-byte storage budget"
        );
    }
}

#[test]
fn shader_buffer_layouts_match_host_layouts() {
    let modules = [
        crate::artifacts::BASE_WGSL,
        crate::artifacts::SUBPIXEL_DUAL_SOURCE_WGSL,
    ]
    .map(|source| naga::front::wgsl::parse_str(source).unwrap());

    for layouts in [gpui::SCENE_BUFFER_LAYOUTS, interface::RENDER_BUFFER_LAYOUTS] {
        for layout in layouts {
            let ty = modules
                .iter()
                .find_map(|module| {
                    module
                        .types
                        .iter()
                        .find_map(|(_, ty)| (ty.name.as_deref() == Some(layout.name)).then_some(ty))
                })
                .unwrap_or_else(|| panic!("missing shader ABI type {}", layout.name));
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                panic!("shader ABI type {} must be a struct", layout.name);
            };

            assert_eq!(
                *span as usize, layout.size,
                "shader ABI size: {}",
                layout.name
            );
            assert_eq!(
                members.len(),
                layout.fields.len(),
                "shader ABI fields: {}",
                layout.name
            );
            for (member, (name, offset)) in members.iter().zip(layout.fields) {
                assert_eq!(
                    member.name.as_deref(),
                    Some(*name),
                    "shader ABI field: {}",
                    layout.name
                );
                assert_eq!(
                    member.offset as usize, *offset,
                    "shader ABI offset: {}.{name}",
                    layout.name
                );
            }
        }
    }
}

#[test]
fn shader_discriminants_match_scene_types() {
    assert_eq!(common::ShaderBool::Disabled as u32, 0);
    assert_eq!(common::ShaderBool::Enabled as u32, 1);
    assert_eq!(
        common::BackgroundTag::Solid as u32,
        gpui::BackgroundTag::Solid as u32
    );
    assert_eq!(
        common::BackgroundTag::LinearGradient as u32,
        gpui::BackgroundTag::LinearGradient as u32
    );
    assert_eq!(
        common::BackgroundTag::PatternSlash as u32,
        gpui::BackgroundTag::PatternSlash as u32
    );
    assert_eq!(
        common::BackgroundTag::Checkerboard as u32,
        gpui::BackgroundTag::Checkerboard as u32
    );
    assert_eq!(
        common::BackgroundTag::RadialGradient as u32,
        gpui::BackgroundTag::RadialGradient as u32
    );
    assert_eq!(
        common::BackgroundTag::RadialGradient3 as u32,
        gpui::BackgroundTag::RadialGradient3 as u32
    );
    assert_eq!(
        common::ColorSpace::Srgb as u32,
        gpui::ColorSpace::Srgb as u32
    );
    assert_eq!(
        common::ColorSpace::Oklab as u32,
        gpui::ColorSpace::Oklab as u32
    );
    assert_eq!(
        common::BorderStyle::Solid as u32,
        gpui::BorderStyle::Solid as u32
    );
    assert_eq!(
        common::BorderStyle::Dashed as u32,
        gpui::BorderStyle::Dashed as u32
    );
}

#[test]
fn linear_gradients_preserve_native_dithering() {
    use common::*;
    use wgsl_rs::std::*;

    let color = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.5,
        a: 0.5,
    };
    let background = Background {
        tag: BackgroundTag::LinearGradient,
        color_space: ColorSpace::Srgb,
        solid: color,
        gradient_angle_or_pattern_height: 90.0,
        colors: [
            LinearColorStop {
                color,
                percentage: 0.0,
            },
            LinearColorStop {
                color,
                percentage: 1.0,
            },
        ],
        padding: 0,
        radial_center_x: 0.0,
        radial_center_y: 0.0,
        radial_radius_x: 0.0,
        radial_radius_y: 0.0,
    };
    let bounds = Bounds {
        origin: vec2f(0.0, 0.0),
        size: vec2f(100.0, 100.0),
    };
    let paint = Paint::new(background, bounds);
    let prepared = prepare_paint(paint);
    let first = paint_color(paint, vec2f(10.0, 10.0), prepared);
    let second = paint_color(paint, vec2f(11.0, 10.0), prepared);

    assert_ne!(first.w, second.w);
}

fn radial_stop(lightness: f32, percentage: f32) -> common::LinearColorStop {
    common::LinearColorStop {
        color: common::Hsla {
            h: 0.0,
            s: 0.0,
            l: lightness,
            a: 1.0,
        },
        percentage,
    }
}

/// A 200x100 paint whose radial gradient is centred at (0.5, 0.5) with radii (0.5, 0.5), so
/// the ellipse touches all four edges and distance 1.0 lands on them.
fn radial_paint(
    tag: common::BackgroundTag,
    last_stop: common::LinearColorStop,
    stops: [common::LinearColorStop; 2],
) -> common::Paint {
    common::Paint::new(
        common::Background {
            tag,
            color_space: common::ColorSpace::Srgb,
            solid: last_stop.color,
            gradient_angle_or_pattern_height: last_stop.percentage,
            colors: stops,
            padding: 0,
            radial_center_x: 0.5,
            radial_center_y: 0.5,
            radial_radius_x: 0.5,
            radial_radius_y: 0.5,
        },
        common::Bounds {
            origin: wgsl_rs::std::vec2f(10.0, 20.0),
            size: wgsl_rs::std::vec2f(200.0, 100.0),
        },
    )
}

#[test]
fn radial_gradient_distance_is_elliptical_in_the_bounds() {
    use common::*;
    use wgsl_rs::std::*;

    let paint = radial_paint(
        BackgroundTag::RadialGradient,
        radial_stop(0.0, 1.0),
        [radial_stop(1.0, 0.0), radial_stop(0.0, 1.0)],
    );
    let distance = |x: f32, y: f32| radial_gradient_distance(paint, vec2f(x, y));

    assert!(distance(110.0, 70.0).abs() < 1e-6, "center");
    assert!((distance(210.0, 70.0) - 1.0).abs() < 1e-6, "right edge");
    assert!((distance(110.0, 120.0) - 1.0).abs() < 1e-6, "bottom edge");
    assert!(
        (distance(160.0, 70.0) - 0.5).abs() < 1e-6,
        "half way along x"
    );
    assert!(
        (distance(110.0, 95.0) - 0.5).abs() < 1e-6,
        "half way along y"
    );
    assert!(
        (distance(210.0, 120.0) - std::f32::consts::SQRT_2).abs() < 1e-5,
        "corner"
    );

    let mut degenerate = paint;
    degenerate.background.radial_radius_x = 0.0;
    degenerate.background.radial_radius_y = 0.0;
    assert!(radial_gradient_distance(degenerate, vec2f(110.0, 70.0)).is_finite());
    assert!(radial_gradient_distance(degenerate, vec2f(111.0, 70.0)) > 1.0);
}

#[test]
fn radial_gradients_interpolate_between_their_stops() {
    use common::*;
    use wgsl_rs::std::*;

    // Two stops: white at the center, black on the edge, before dithering.
    let paint = radial_paint(
        BackgroundTag::RadialGradient,
        radial_stop(0.0, 1.0),
        [radial_stop(1.0, 0.0), radial_stop(0.0, 1.0)],
    );
    let prepared = prepare_paint(paint);
    let undithered =
        |x: f32, y: f32| paint_color(paint, vec2f(x, y), prepared) - gradient_dither(vec2f(x, y));
    assert!(
        (undithered(110.0, 70.0).x - 1.0).abs() < 1e-5,
        "white center"
    );
    assert!(undithered(210.0, 70.0).x.abs() < 1e-5, "black edge");
    assert!(
        undithered(260.0, 70.0).x.abs() < 1e-5,
        "black beyond the edge"
    );
    let middle = undithered(160.0, 70.0).x;
    assert!(
        middle > 0.1 && middle < 0.9,
        "mid grey at half radius: {middle}"
    );
    assert_eq!(undithered(110.0, 70.0).w, 1.0);

    // Three stops: white at the center, black at 0.5, white again on the edge.
    let paint = radial_paint(
        BackgroundTag::RadialGradient3,
        radial_stop(1.0, 1.0),
        [radial_stop(1.0, 0.0), radial_stop(0.0, 0.5)],
    );
    let prepared = prepare_paint(paint);
    let undithered =
        |x: f32, y: f32| paint_color(paint, vec2f(x, y), prepared) - gradient_dither(vec2f(x, y));
    assert!(
        (undithered(110.0, 70.0).x - 1.0).abs() < 1e-5,
        "white center"
    );
    assert!(undithered(160.0, 70.0).x.abs() < 1e-5, "black middle stop");
    assert!((undithered(210.0, 70.0).x - 1.0).abs() < 1e-5, "white edge");
    let quarter = undithered(135.0, 70.0).x;
    let three_quarters = undithered(185.0, 70.0).x;
    assert!(
        quarter > 0.1 && quarter < 0.9,
        "grey before the middle: {quarter}"
    );
    assert!(
        three_quarters > 0.1 && three_quarters < 0.9,
        "grey after the middle: {three_quarters}"
    );
}
