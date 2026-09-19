#pragma once
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct BiteVertex {float pos[2];float uv[2];uint32_t color;} BiteVertex;
typedef struct BiteCommand {uint32_t count;uint32_t index_offset;uint32_t vertex_offset;float clip[4];uint64_t texture;} BiteCommand;
void* bite_create(void);
void bite_destroy(void* context);
void bite_frame(float width,float height,float scale,float delta);
void bite_render(void);
void bite_mouse_position(float x,float y);
void bite_mouse_button(int button,int down);
void bite_mouse_wheel(float x,float y);
void bite_key(int key,int down);
void bite_text_input(const char* text);
void bite_focus(int focused);
void bite_font_pixels(const unsigned char** pixels,int* width,int* height);
void bite_font_texture(uint64_t texture);
void bite_dockspace(void);
int bite_begin(const char* title);
void bite_end(void);
void bite_text(const char* text);
int bite_button(const char* text);
int bite_drag_float(const char* label,float* value);
int bite_input_text(const char* label,char* data,size_t size);
void bite_image(uint64_t texture,float width,float height);
void bite_same_line(void);
void bite_editor_begin(void* context,const char* label);
void bite_editor_end(void);
void bite_node_begin(uint64_t id);
void bite_node_end(void);
void bite_pin_begin(uint64_t id,int output);
void bite_pin_end(void);
void bite_link(uint64_t id,uint64_t source,uint64_t target,float r,float g,float b);
int bite_new_link(uint64_t* source,uint64_t* target);
int bite_new_node(uint64_t* pin);
int bite_deleted_link(uint64_t* id);
void bite_set_node_position(uint64_t id,float x,float y);
void bite_get_node_position(uint64_t id,float* x,float* y);
int bite_node_selected(uint64_t id);
void bite_group(float width,float height);
int bite_background_menu(void);
void bite_navigate(void);
int bite_draw_list_count(void);
int bite_vertex_count(int list);
int bite_index_count(int list);
int bite_command_count(int list);
void bite_copy_vertices(int list,BiteVertex* vertices);
void bite_copy_indices(int list,uint32_t* indices);
int bite_get_command(int list,int index,BiteCommand* command);
#ifdef __cplusplus
}
#endif
