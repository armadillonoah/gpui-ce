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
        prepare_fill_color.contains("tag == 1 || tag == 4"),
        "radial gradients (tag 4) must initialize color0 and color1"
    );
}
