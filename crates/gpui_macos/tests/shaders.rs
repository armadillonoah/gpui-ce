const METAL_SHADER: &str = include_str!("../src/shaders.metal");

#[test]
fn radial_gradient_colors_are_prepared_in_the_vertex_shader() {
    let prepare_fill_color = METAL_SHADER
        .split_once("GradientColor prepare_fill_color")
        .expect("Metal shader should define prepare_fill_color")
        .1
        .split_once("float2x2 rotate2d")
        .expect("prepare_fill_color should precede rotate2d")
        .0;

    assert!(
        prepare_fill_color.contains("tag == 1 || tag == 4 || tag == 5"),
        "both radial-gradient tags must initialize color0 and color1"
    );
    assert!(prepare_fill_color.contains("out.solid = hsla_to_rgba(solid)"));
    assert!(prepare_fill_color.contains("out.solid = srgb_to_oklab(out.solid)"));
}

#[test]
fn three_stop_radial_gradient_is_interpolated_piecewise() {
    let fill_color = METAL_SHADER
        .rsplit_once("float4 fill_color(Background background")
        .expect("Metal shader should define fill_color")
        .1;
    let radial3 = fill_color
        .split_once("case 5:")
        .expect("Metal shader should handle RadialGradient3")
        .1
        .split_once("case 2:")
        .expect("RadialGradient3 should precede pattern handling")
        .0;

    assert!(radial3.contains("distance <= middle_percentage"));
    assert!(radial3.contains("color0, color1, t"));
    assert!(radial3.contains("color1, solid_color, t"));
    assert!(radial3.contains("last_percentage - middle_percentage"));
    let interpolation = METAL_SHADER
        .rsplit_once("float4 interpolate_gradient_color")
        .expect("Metal shader should define interpolate_gradient_color")
        .1
        .split_once("float4 dither_gradient")
        .expect("interpolation should precede dither")
        .0;
    assert!(interpolation.contains("start_color.rgb * start_color.a"));
    assert!(interpolation.contains("premultiplied / alpha"));
}

#[test]
fn gradient_dither_is_centralized_and_preserves_alpha_endpoints() {
    let dither = METAL_SHADER
        .rsplit_once("float4 dither_gradient(float4 color")
        .expect("Metal shader should define dither_gradient")
        .1
        .split_once("float2x2 rotate2d")
        .expect("dither_gradient should precede rotate2d")
        .0;
    assert!(dither.contains("dither_strength <= 0.0"));
    assert!(dither.contains("color.a > 0.0 && color.a < 1.0"));
    assert!(dither.contains("tri * (1.0 / 255.0) * dither_strength"));
    assert!(dither.contains("alpha_tri * (1.0 / 255.0) * dither_strength"));
    assert!(!dither.contains("2.0 / 255.0"));
    assert!(!dither.contains("3.0 / 255.0"));
    assert!(dither.contains("alpha_tri = r1 - r2"));

    let fill_color = METAL_SHADER
        .rsplit_once("float4 fill_color(Background background")
        .expect("Metal shader should define fill_color")
        .1;
    assert_eq!(fill_color.matches("dither_gradient(").count(), 1);
    assert_eq!(
        fill_color
            .matches("unclamped_t > 0.0 && unclamped_t < 1.0")
            .count(),
        2
    );
    assert!(fill_color.contains("distance > first_percentage && distance < last_percentage"));
    assert!(fill_color.contains("dither_gradient(color, position, dither_strength)"));
    assert!(
        fill_color.contains("background.tag == 1 || background.tag == 4 || background.tag == 5")
    );
}
