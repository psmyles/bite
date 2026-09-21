// The sokol_imgui implementation, compiled by build.rs.
//
// sokol_imgui.h is the Dear ImGui *renderer* backend for sokol_gfx (SOKOL_IMGUI_NO_SOKOL_APP:
// the window, the event loop and input are winit's, not sokol_app's): it uploads ImGui's
// textures, fonts included, through the 1.92 texture protocol as sokol_gfx images, and draws the
// draw lists into the current sokol_gfx pass. It talks to Dear ImGui through the cimgui C API -
// the same API `bite-imgui-sys` compiles and binds - so this translation unit sees the exact
// struct layouts the Rust side does.
//
// There is no `cimgui.h` beside this file: the include path points at
// `../bite-imgui-sys/vendor/cimgui`, so there is one header rather than two that can drift. The
// defines come from build.rs, which mirrors `bite-imgui-sys/build.rs` - see the comment at each
// end. They are not a preference: the two translation units share struct layouts, and
// `IMGUI_DISABLE_OBSOLETE_FUNCTIONS` changes them.
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "sokol_gfx.h"
#include "cimgui.h"
#define SOKOL_IMGUI_IMPL
#include "sokol_imgui.h"
