#import "../template.typ": template

#show: template.with(current: "hot-patching")

= Experimental hot patching

GPUI-CE can update UI rendering code in a running desktop application without
restarting its process or recreating its windows. This uses Dioxus CLI's
Subsecond runtime and works in native debug builds.

== Setup

Enable the `hot-patching` feature on your `gpui-ce` dependency. Until a release
includes this feature, use a Git revision containing it:

```toml
gpui-ce = { git = "https://github.com/gpui-ce/gpui-ce", features = ["hot-patching"] }
```

Install a Dioxus CLI version compatible with Subsecond 0.7.10:

```bash
cargo install dioxus-cli --version 0.7.10 --locked
```

From your application's Cargo project, run:

```bash
dx serve --hot-patch
```

Edit and save a `Render::render` function. A successful patch redraws the
existing window without starting a new process. The
#link("https://github.com/gpui-ce/gpui-ce/tree/main/crates/gpui/examples")[repository examples]
can provide starting code for a separate Cargo application.

== Applying patches yourself

By default GPUI connects to the Dioxus CLI and applies each patch the moment it
arrives. A patch that changes the fields of a view's type is only safe once
every entity of that type has been dropped, since old and new code disagree on
its layout. An application that rebuilds such views first can take patches
itself:

```rust
Application::new().with_manual_hot_patches()
```

GPUI then does not connect. The application connects with
`dioxus_devtools::connect_at` and its own message handler, and applies each patch with
`subsecond::apply_patch` on the main thread when it is ready. Windows still
redraw after each patch.

`App::live_entities_of(prefix)` counts the live entities whose type path starts
with `prefix`, such as `"my_crate::"`, so the application can check that none
of the affected views is left before it applies the patch. Both calls exist
only in debug builds with the `hot-patching` feature.

== Limitations

This feature is experimental. A patch can fail or crash the application. Keep
the normal restart workflow available. Dioxus CLI monitors the application
crate and its direct workspace dependencies, but changes in indirect workspace
dependencies require a rebuild. Hot patching does not run on WASM targets or
in release builds. The integration hooks view rendering and element layout,
prepaint, and paint; other functions may require a restart.

DX 0.7.10 can reject edits to examples launched directly from the GPUI-CE
workspace with a `Cycle in workspace dependency graph` error. Run a separate
application that depends on this checkout instead.

Report problems in the #link("https://github.com/gpui-ce/gpui-ce/issues")[GPUI-CE issue tracker].
