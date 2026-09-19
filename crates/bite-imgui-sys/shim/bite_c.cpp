#include "bite_c.h"
#include "imgui.h"
#include "imgui_internal.h"
#include "imgui_node_editor.h"
#include <cstring>

namespace ed=ax::NodeEditor;
struct State {ImGuiContext* imgui;ed::EditorContext* editor;};
void* bite_create(){auto s=new State;s->imgui=ImGui::CreateContext();auto& io=ImGui::GetIO();io.ConfigFlags|=ImGuiConfigFlags_DockingEnable;io.BackendFlags|=ImGuiBackendFlags_RendererHasVtxOffset;io.IniFilename=nullptr;ed::Config c;c.SettingsFile=nullptr;s->editor=ed::CreateEditor(&c);ImGui::StyleColorsDark();return s;}
void bite_destroy(void* context){auto s=(State*)context;ed::DestroyEditor(s->editor);ImGui::DestroyContext(s->imgui);delete s;}
void bite_frame(float w,float h,float scale,float dt){auto& io=ImGui::GetIO();io.DisplaySize=ImVec2(w,h);io.DisplayFramebufferScale=ImVec2(scale,scale);io.DeltaTime=dt;ImGui::NewFrame();}
void bite_render(){ImGui::Render();}
void bite_mouse_position(float x,float y){ImGui::GetIO().AddMousePosEvent(x,y);}
void bite_mouse_button(int b,int down){ImGui::GetIO().AddMouseButtonEvent(b,down!=0);}
void bite_mouse_wheel(float x,float y){ImGui::GetIO().AddMouseWheelEvent(x,y);}
void bite_key(int key,int down){ImGui::GetIO().AddKeyEvent((ImGuiKey)key,down!=0);}
void bite_text_input(const char* s){ImGui::GetIO().AddInputCharactersUTF8(s);}
void bite_focus(int f){ImGui::GetIO().AddFocusEvent(f!=0);}
void bite_font_pixels(const unsigned char** pixels,int* w,int* h){unsigned char* p;ImGui::GetIO().Fonts->GetTexDataAsRGBA32(&p,w,h);*pixels=p;}
void bite_font_texture(uint64_t id){ImGui::GetIO().Fonts->SetTexID((ImTextureID)(uintptr_t)id);}
void bite_dockspace(){
    auto viewport=ImGui::GetMainViewport();
    auto dock=ImGui::DockSpaceOverViewport(0,viewport);
    auto node=ImGui::DockBuilderGetNode(dock);
    if(node && node->IsLeafNode() && node->Windows.empty()) {
        ImGui::DockBuilderRemoveNode(dock);
        ImGui::DockBuilderAddNode(dock,ImGuiDockNodeFlags_DockSpace);
        ImGui::DockBuilderSetNodeSize(dock,viewport->Size);
        auto center=dock;
        auto left=ImGui::DockBuilderSplitNode(center,ImGuiDir_Left,0.15f,nullptr,&center);
        auto right=ImGui::DockBuilderSplitNode(center,ImGuiDir_Right,0.25f,nullptr,&center);
        auto bottom=ImGui::DockBuilderSplitNode(center,ImGuiDir_Down,0.15f,nullptr,&center);
        auto preview=ImGui::DockBuilderSplitNode(right,ImGuiDir_Down,0.5f,nullptr,&right);
        ImGui::DockBuilderDockWindow("Library",left);
        ImGui::DockBuilderDockWindow("Canvas",center);
        ImGui::DockBuilderDockWindow("Inspector",right);
        ImGui::DockBuilderDockWindow("Preview",preview);
        ImGui::DockBuilderDockWindow("Filmstrip",bottom);
        ImGui::DockBuilderFinish(dock);
    }
}
int bite_begin(const char* s){return ImGui::Begin(s);}
void bite_end(){ImGui::End();}
void bite_text(const char* s){ImGui::TextUnformatted(s);}
int bite_button(const char* s){return ImGui::Button(s);}
int bite_drag_float(const char* s,float* v){return ImGui::DragFloat(s,v,0.1f);}
int bite_input_text(const char* s,char* data,size_t size){return ImGui::InputTextMultiline(s,data,size,ImVec2(-1,100));}
void bite_image(uint64_t id,float w,float h){ImGui::Image((ImTextureID)(uintptr_t)id,ImVec2(w,h));}
void bite_same_line(){ImGui::SameLine();}
void bite_editor_begin(void* context,const char* label){ed::SetCurrentEditor(((State*)context)->editor);ed::Begin(label);}
void bite_editor_end(){ed::End();}
void bite_node_begin(uint64_t id){ed::BeginNode(ed::NodeId((uintptr_t)id));}
void bite_node_end(){ed::EndNode();}
void bite_pin_begin(uint64_t id,int output){ed::BeginPin(ed::PinId((uintptr_t)id),output?ed::PinKind::Output:ed::PinKind::Input);}
void bite_pin_end(){ed::EndPin();}
void bite_link(uint64_t id,uint64_t a,uint64_t b,float r,float g,float blue){ed::Link(ed::LinkId((uintptr_t)id),ed::PinId((uintptr_t)a),ed::PinId((uintptr_t)b),ImColor(r,g,blue),2);}
int bite_new_link(uint64_t* source,uint64_t* target){int accepted=0;if(ed::BeginCreate()){ed::PinId a,b;if(ed::QueryNewLink(&a,&b)&&a&&b&&ed::AcceptNewItem()){*source=a.Get();*target=b.Get();accepted=1;}}ed::EndCreate();return accepted;}
int bite_new_node(uint64_t* pin){int accepted=0;if(ed::BeginCreate()){ed::PinId p;if(ed::QueryNewNode(&p)&&ed::AcceptNewItem()){*pin=p.Get();accepted=1;}}ed::EndCreate();return accepted;}
int bite_deleted_link(uint64_t* id){int accepted=0;if(ed::BeginDelete()){ed::LinkId link;if(ed::QueryDeletedLink(&link)&&ed::AcceptDeletedItem()){*id=link.Get();accepted=1;}}ed::EndDelete();return accepted;}
void bite_set_node_position(uint64_t id,float x,float y){ed::SetNodePosition(ed::NodeId((uintptr_t)id),ImVec2(x,y));}
void bite_get_node_position(uint64_t id,float* x,float* y){auto p=ed::GetNodePosition(ed::NodeId((uintptr_t)id));*x=p.x;*y=p.y;}
int bite_node_selected(uint64_t id){return ed::IsNodeSelected(ed::NodeId((uintptr_t)id));}
void bite_group(float w,float h){ed::Group(ImVec2(w,h));}
int bite_background_menu(){return ed::ShowBackgroundContextMenu();}
void bite_navigate(){ed::NavigateToContent(0.0f);}
int bite_draw_list_count(){return ImGui::GetDrawData()->CmdListsCount;}
int bite_vertex_count(int i){return ImGui::GetDrawData()->CmdLists[i]->VtxBuffer.Size;}
int bite_index_count(int i){return ImGui::GetDrawData()->CmdLists[i]->IdxBuffer.Size;}
int bite_command_count(int i){return ImGui::GetDrawData()->CmdLists[i]->CmdBuffer.Size;}
void bite_copy_vertices(int i,BiteVertex* dst){static_assert(sizeof(BiteVertex)==sizeof(ImDrawVert));auto& b=ImGui::GetDrawData()->CmdLists[i]->VtxBuffer;memcpy(dst,b.Data,b.Size*sizeof(BiteVertex));}
void bite_copy_indices(int i,uint32_t* dst){auto& b=ImGui::GetDrawData()->CmdLists[i]->IdxBuffer;for(int j=0;j<b.Size;++j)dst[j]=b[j];}
int bite_get_command(int i,int j,BiteCommand* dst){auto& c=ImGui::GetDrawData()->CmdLists[i]->CmdBuffer[j];if(c.UserCallback)return 0;dst->count=c.ElemCount;dst->index_offset=c.IdxOffset;dst->vertex_offset=c.VtxOffset;dst->clip[0]=c.ClipRect.x;dst->clip[1]=c.ClipRect.y;dst->clip[2]=c.ClipRect.z;dst->clip[3]=c.ClipRect.w;dst->texture=(uint64_t)(uintptr_t)c.TextureId;return 1;}
