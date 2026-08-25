const HLSL_SHADER: &str = include_str!("../src/shaders.hlsl");

fn shader_section<'a>(start: &str, end: &str) -> &'a str {
    HLSL_SHADER
        .split_once(start)
        .unwrap_or_else(|| panic!("HLSL shader should contain {start}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("HLSL shader section {start} should precede {end}"))
        .0
}

#[test]
fn three_stop_radial_gradient_is_prepared_and_interpolated_piecewise() {
    let prepare = shader_section("GradientColor prepare_gradient_color", "float2x2 rotate2d");
    assert!(prepare.contains("tag == 1 || tag == 4 || tag == 5"));
    assert!(prepare.contains("output.solid = hsla_to_rgba(solid)"));
    assert!(prepare.contains("output.solid = srgb_to_oklab(output.solid)"));

    let gradient = shader_section("float4 gradient_color", "// Returns the dash velocity");
    let radial3 = gradient
        .split_once("case 5:")
        .expect("HLSL shader should handle RadialGradient3")
        .1
        .split_once("case 2:")
        .expect("RadialGradient3 should precede pattern handling")
        .0;
    assert!(radial3.contains("distance <= middle_percentage"));
    assert!(radial3.contains("color0, color1, t"));
    assert!(radial3.contains("color1, solid_color, t"));
    assert!(radial3.contains("last_percentage - middle_percentage"));
    let interpolation = shader_section(
        "float4 interpolate_gradient_color",
        "float4 dither_gradient",
    );
    assert!(interpolation.contains("start_color.rgb * start_color.a"));
    assert!(interpolation.contains("premultiplied / alpha"));
}

#[test]
fn gradient_dither_is_centralized_and_preserves_alpha_endpoints() {
    let dither = shader_section("float4 dither_gradient", "float4 gradient_color");
    assert!(dither.contains("dither_strength <= 0.0"));
    assert!(dither.contains("color.a > 0.0 && color.a < 1.0"));
    assert!(dither.contains("tri * (1.0 / 255.0) * dither_strength"));
    assert!(dither.contains("alpha_tri * (1.0 / 255.0) * dither_strength"));
    assert!(!dither.contains("2.0 / 255.0"));
    assert!(!dither.contains("3.0 / 255.0"));
    assert!(dither.contains("alpha_tri = r1 - r2"));

    let gradient = shader_section("float4 gradient_color", "// Returns the dash velocity");
    assert_eq!(gradient.matches("dither_gradient(").count(), 1);
    assert_eq!(
        gradient
            .matches("unclamped_t > 0.0 && unclamped_t < 1.0")
            .count(),
        2
    );
    assert!(gradient.contains("distance > first_percentage && distance < last_percentage"));
    assert!(gradient.contains("dither_gradient(color, position, dither_strength)"));
    assert!(gradient.contains("background.tag == 1 || background.tag == 4 || background.tag == 5"));
}
