# topic/gpui-ce integration — 2026-10-08

Integration branch for the GPUI-CE fork work. Base: `upstream-main` =
`81c2e755f7` (gpui-ce/gpui-ce main). Six finished topics merged with
`git merge --no-ff`, topic commits untouched. Validated on an Apple Silicon
Mac (native) plus `x86_64-pc-windows-msvc` cargo checks; nothing Windows
or Linux has actually run here. This file is for the Windows and Linux
validators.

## Contents (merge order)

| # | Topic | Topic head | Merge commit | What it adds |
|---|-------|-----------|--------------|--------------|
| 1 | `topic/windows-wgpu` | `3e21821ce9` (14 commits from `a3c42c2f4a`) | `8fca4a374f` | Upstream PR #313 rebased onto main + fixes: Windows wgpu renderer (`gpui_ce_windows` feature `wgpu`), custom GPU controls, headless custom-gpu texture compositing test, no DirectX devices under wgpu, vsync fixes, Parley text system under wgpu. |
| 2 | `topic/wgpu-device-requirements` | `b59e30086b` | `4a598deaeb` | Applications request optional/experimental wgpu features and honoured limits. |
| 3 | `topic/path-prompt-file-types` | `7267059bd9` | `69aa145382` | `PathPromptOptions::file_types` (macOS, Windows, Linux file dialogs). |
| 4 | `topic/surface-corner-radii` | `aaa5a08c06` | `2231450e7a` | Surfaces clip to their element's corner radii (Metal, wgpu, DirectX). |
| 5 | `topic/rounded-content-mask` | `9cf2512eae` | `5414d56883` | `ContentMask::corner_radii`; rounded overflow-hidden parents clip children to the curve. Changes shader buffer layouts / path vertex size. |
| 6 | `topic/radial-gradients` | `8994a86f09` | `82c8388461` | `radial_gradient` / `radial_gradient_3` backgrounds; `Background` grows four radial fields. |

Integration fix (after the merges): `aa71f6fda0` — `crates/gpui_macos/src/wgpu_renderer.rs`
test initializer gets `..Default::default()` (it lacked `fade_out` at upstream
main and `corner_radii` after topic 5; only compiled under `macos-wgpu`).

Conflict resolutions are recorded in each merge commit body. The only
textual conflicts were in topic 6 vs topic 5: layout sizes are the sum of
both additions (`PathRasterizationVertex` 168, `Quad` budget 296, `Shadow`
budget 216) and both new headless tests are kept.

Behaviour note: radial gradients default to **Oklab** interpolation.
Call `.color_space(ColorSpace::Srgb)` on the background for the fork's old
sRGB look.

## Building each platform

Common env: `CARGO_BUILD_JOBS=4`, format with `cargo +1.96.0 fmt`.

macOS (Metal default; wgpu via feature):
```
cargo build --workspace --all-targets \
  --features gpui_ce_wgpu/custom-gpu,gpui_ce_wgpu/wgpu-surfaces,gpui_ce_platform/macos-wgpu
```

Windows (wgpu renderer; `windows-wgpu` already enables custom-gpu + wgpu-surfaces):
```
cargo build --workspace --all-targets --features gpui_ce_platform/windows-wgpu
cargo build -p gpui_ce_windows --no-default-features --features wgpu
cargo build -p gpui_ce_windows --no-default-features          # DirectX path
```
Cross-check from a non-Windows host needs `GPUI_RENDER_ALLOW_MISSING_DXBC=1`
and, because `llvm-rc` is absent, `RC_x86_64_pc_windows_msvc=<no-op script
that exits 0>` for `-p gpui_ce_windows`.

Linux (wgpu is always on; pick a windowing backend):
```
cargo build --workspace --all-targets --features gpui_ce_platform/x11,gpui_ce_platform/wayland
```

## Gates passed on macOS (six-way tree + fix)

- `cargo +1.96.0 fmt --all -- --check`: clean
- build (features above): ok
- `cargo clippy --workspace --all-targets <same features> -- -D warnings`: clean
- `cargo test -p gpui-ce --lib`: 403 passed
- `cargo test -p gpui_ce_render`: 32 passed (layout parity, `shader_buffer_layouts_match_host_layouts`)
- `cargo test -p gpui_ce_wgpu --features custom-gpu,wgpu-surfaces,test-support --no-fail-fast`: 40 passed / 1 failed (pre-existing, below); `tests/headless_primitives.rs` 6 passed
- `cargo test -p gpui_ce_macos file_types`: 2 passed
- `cargo build -p gpui-ce --examples`: ok
- `cargo check --target x86_64-pc-windows-msvc -p gpui_ce_wgpu --features custom-gpu,wgpu-surfaces`: ok
- `cargo check --target x86_64-pc-windows-msvc -p gpui_ce_windows --no-default-features [--features wgpu]`: both ok
- `cargo machete`: not installed, skipped

## Tests only Windows can run

- `cargo test -p gpui_ce_windows --no-default-features --features wgpu,test-support`
  and the same without `wgpu` (DirectX): `platform.rs` `test_file_type_spec`
  (topic 3), `directx_renderer.rs` tests (topic 4/5 touched it), and the wgpu
  renderer's `render_to_image` path (topic 1, behind `test-support`).
- `cargo test -p gpui_ce_wgpu --features custom-gpu,wgpu-surfaces,test-support`
  on Windows: exercises `surfaces/windows.rs` (custom-gpu texture compositing,
  topic 1) and `surfaces/windows/` D3D interop on a real D3D12 adapter.
- Manual: open a window with `gpui_ce_platform/windows-wgpu`, set GPU
  requirements before the first window (a later call only logs a warning),
  confirm vsync and device-loss recovery, rounded surfaces/content masks and
  radial gradients render.
- `cargo clippy --workspace --all-targets --features gpui_ce_platform/windows-wgpu -- -D warnings`.

## Tests only Linux can run

- `cargo test -p gpui_ce_linux` (x11 + wayland): `platform.rs`
  `extension_globs_match_either_case`,
  `file_filter_carries_the_label_and_one_glob_per_extension` (topic 3) plus
  the existing suite.
- `cargo test -p gpui_ce_wgpu --features custom-gpu,wgpu-surfaces,test-support`
  on Vulkan (headless primitives: rounded mask, radial gradients, fades,
  surface corner clipping).
- `cargo clippy --workspace --all-targets --features gpui_ce_platform/x11,gpui_ce_platform/wayland -- -D warnings`.

## Known pre-existing failures

- `gpui_ce_wgpu` `wgpu_renderer::tests::quad_border_backgrounds_use_the_fill_coordinate_space`
  fails on macOS Metal: linear-gradient border sample is off by 3/255
  (`got [228,159,103,255], expected [225,158,102,255]`). Present before this
  branch; not fixed here. May pass on D3D12/Vulkan.
- `block v0.1.6` future-incompatibility warning from cargo (dependency, not ours).
