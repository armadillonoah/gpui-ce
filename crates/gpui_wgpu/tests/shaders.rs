const WGPU_SHADER: &str = include_str!("../src/shaders.wgsl");

fn shader_section<'a>(start: &str, end: &str) -> &'a str {
    WGPU_SHADER
        .split_once(start)
        .unwrap_or_else(|| panic!("WGSL shader should contain {start}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("WGSL shader section {start} should precede {end}"))
        .0
}

#[test]
fn complete_wgsl_shader_parses_and_validates() {
    let module = naga::front::wgsl::parse_str(WGPU_SHADER).expect("WGSL shader should parse");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("WGSL shader should pass semantic validation");
}

#[test]
fn three_stop_radial_gradient_is_prepared_and_interpolated_piecewise() {
    let prepare = shader_section("fn prepare_gradient_color", "fn interpolate_gradient_color");
    assert!(prepare.contains("tag == 1u || tag == 4u || tag == 5u"));
    assert!(prepare.contains("result.solid = hsla_to_rgba(solid)"));
    assert!(prepare.contains("result.solid = linear_srgb_to_oklab(result.solid)"));
    assert!(
        !prepare.contains("linear_to_srgba"),
        "default sRGB gradients must not be encoded a second time"
    );

    let gradient = shader_section("fn gradient_color", "// --- quads --- //");
    let radial3 = gradient
        .split_once("case 5u:")
        .expect("WGSL shader should handle RadialGradient3")
        .1
        .split_once("case 2u:")
        .expect("RadialGradient3 should precede pattern handling")
        .0;
    assert!(radial3.contains("distance <= middle_percentage"));
    assert!(radial3.contains("color0, color1, t"));
    assert!(radial3.contains("color1, solid_color, t"));
    assert!(radial3.contains("last_percentage - middle_percentage"));
    let interpolation = shader_section("fn interpolate_gradient_color", "fn dither_gradient");
    assert!(interpolation.contains("start_color.rgb * start_color.a"));
    assert!(interpolation.contains("premultiplied / alpha"));
    assert!(interpolation.contains("return interpolated;"));
    assert!(interpolation.contains("oklab_to_linear_srgb(interpolated)"));
    assert!(
        !interpolation.contains("srgba_to_linear"),
        "default sRGB gradients must remain coded for the non-sRGB target"
    );
}

#[test]
fn gradient_dither_is_centralized_and_preserves_alpha_endpoints() {
    let dither = shader_section("fn dither_gradient", "fn gradient_color");
    assert!(dither.contains("dither_strength <= 0.0"));
    assert!(dither.contains("alpha > 0.0 && alpha < 1.0"));
    assert!(dither.contains("tri * (1.0 / 255.0) * dither_strength"));
    assert!(dither.contains("alpha_tri * (1.0 / 255.0) * dither_strength"));
    assert!(!dither.contains("2.0 / 255.0"));
    assert!(!dither.contains("3.0 / 255.0"));
    assert!(dither.contains("alpha_tri = r1 - r2"));

    let gradient = shader_section("fn gradient_color", "// --- quads --- //");
    assert_eq!(gradient.matches("dither_gradient(").count(), 1);
    assert_eq!(
        gradient
            .matches("unclamped_t > 0.0 && unclamped_t < 1.0")
            .count(),
        2
    );
    assert!(gradient.contains("distance > first_percentage && distance < last_percentage"));
    assert!(gradient.contains("dither_gradient(background_color, position, dither_strength)"));
    assert!(
        gradient.contains("background.tag == 1u || background.tag == 4u || background.tag == 5u")
    );
}
