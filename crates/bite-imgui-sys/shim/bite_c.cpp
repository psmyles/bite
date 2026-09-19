#include "bite_c.h"
#include "imgui.h"
#include "imgui_internal.h"
#include "imgui_node_editor.h"
#include <cstring>
#include <string>

namespace ed=ax::NodeEditor;
struct State {ImGuiContext* imgui;ed::EditorContext* editor;std::string ini;};
static ImVec4 color(int r,int g,int b,int a=255){return ImVec4(r/255.0f,g/255.0f,b/255.0f,a/255.0f);}
static void bite_style(){
    ImGui::StyleColorsDark();
    auto& s=ImGui::GetStyle();
    s.WindowPadding=ImVec2(12,10);s.FramePadding=ImVec2(8,5);s.CellPadding=ImVec2(8,5);
    s.ItemSpacing=ImVec2(8,5);s.ItemInnerSpacing=ImVec2(6,4);s.IndentSpacing=16;
    s.ScrollbarSize=12;s.GrabMinSize=12;
    s.WindowRounding=6;s.ChildRounding=4;s.FrameRounding=4;s.PopupRounding=4;
    s.ScrollbarRounding=2;s.GrabRounding=4;s.TabRounding=0;
    s.WindowBorderSize=1;s.ChildBorderSize=1;s.PopupBorderSize=1;s.FrameBorderSize=1;s.TabBorderSize=0;
    auto* c=s.Colors;
    c[ImGuiCol_Text]=color(168,168,168);c[ImGuiCol_TextDisabled]=color(110,110,110);
    c[ImGuiCol_WindowBg]=color(20,20,20);c[ImGuiCol_ChildBg]=color(20,20,20);
    c[ImGuiCol_PopupBg]=color(28,28,28);c[ImGuiCol_Border]=color(88,88,88);c[ImGuiCol_BorderShadow]=color(0,0,0,0);
    c[ImGuiCol_FrameBg]=color(17,17,17);c[ImGuiCol_FrameBgHovered]=color(42,42,42);c[ImGuiCol_FrameBgActive]=color(24,110,160);
    c[ImGuiCol_TitleBg]=color(28,28,28);c[ImGuiCol_TitleBgActive]=color(28,28,28);c[ImGuiCol_TitleBgCollapsed]=color(28,28,28);
    c[ImGuiCol_MenuBarBg]=color(28,28,28);c[ImGuiCol_ScrollbarBg]=color(20,20,20,0);
    c[ImGuiCol_ScrollbarGrab]=color(70,70,70);c[ImGuiCol_ScrollbarGrabHovered]=color(90,90,90);c[ImGuiCol_ScrollbarGrabActive]=color(24,110,160);
    c[ImGuiCol_CheckMark]=color(255,255,255);c[ImGuiCol_SliderGrab]=color(24,110,160);c[ImGuiCol_SliderGrabActive]=color(32,130,185);
    c[ImGuiCol_Button]=color(60,60,60);c[ImGuiCol_ButtonHovered]=color(75,75,75);c[ImGuiCol_ButtonActive]=color(24,110,160);
    c[ImGuiCol_Header]=color(48,48,48);c[ImGuiCol_HeaderHovered]=color(60,60,60);c[ImGuiCol_HeaderActive]=color(24,110,160);
    c[ImGuiCol_Separator]=color(60,60,60);c[ImGuiCol_SeparatorHovered]=color(24,110,160);c[ImGuiCol_SeparatorActive]=color(32,130,185);
    c[ImGuiCol_ResizeGrip]=color(70,70,70,80);c[ImGuiCol_ResizeGripHovered]=color(24,110,160);c[ImGuiCol_ResizeGripActive]=color(32,130,185);
    c[ImGuiCol_Tab]=color(30,30,30);c[ImGuiCol_TabHovered]=color(60,60,60);c[ImGuiCol_TabSelected]=color(37,37,38);
    c[ImGuiCol_TabDimmed]=color(26,26,26);c[ImGuiCol_TabDimmedSelected]=color(32,32,32);
    c[ImGuiCol_DockingPreview]=color(24,110,160,180);c[ImGuiCol_DockingEmptyBg]=color(24,24,24);
    c[ImGuiCol_PlotLines]=color(140,140,140);c[ImGuiCol_PlotLinesHovered]=color(32,130,185);
    c[ImGuiCol_PlotHistogram]=color(24,110,160);c[ImGuiCol_PlotHistogramHovered]=color(32,130,185);
    c[ImGuiCol_TableHeaderBg]=color(37,37,38);c[ImGuiCol_TableBorderStrong]=color(70,70,70);c[ImGuiCol_TableBorderLight]=color(60,60,60);
    c[ImGuiCol_TextSelectedBg]=color(24,110,160,160);c[ImGuiCol_DragDropTarget]=color(32,130,185);
    c[ImGuiCol_NavHighlight]=color(32,130,185);c[ImGuiCol_ModalWindowDimBg]=color(0,0,0,150);
}
static void bite_node_style(){
    auto& s=ed::GetStyle();
    s.NodePadding=ImVec4(0,0,0,5);s.NodeRounding=6;s.NodeBorderWidth=1;
    s.HoveredNodeBorderWidth=2;s.SelectedNodeBorderWidth=2;s.PinRounding=4;s.PinBorderWidth=0;
    s.GroupRounding=6;s.GroupBorderWidth=1;
    s.Colors[ed::StyleColor_Bg]=color(20,20,20);s.Colors[ed::StyleColor_Grid]=color(32,32,32);
    s.Colors[ed::StyleColor_NodeBg]=color(33,33,33);s.Colors[ed::StyleColor_NodeBorder]=color(88,88,88);
    s.Colors[ed::StyleColor_HovNodeBorder]=color(32,130,185);s.Colors[ed::StyleColor_SelNodeBorder]=color(24,110,160);
    s.Colors[ed::StyleColor_NodeSelRect]=color(24,110,160,50);s.Colors[ed::StyleColor_NodeSelRectBorder]=color(32,130,185,180);
    s.Colors[ed::StyleColor_HovLinkBorder]=color(32,130,185);s.Colors[ed::StyleColor_SelLinkBorder]=color(32,130,185);
    s.Colors[ed::StyleColor_HighlightLinkBorder]=color(32,130,185);s.Colors[ed::StyleColor_LinkSelRect]=color(24,110,160,50);
    s.Colors[ed::StyleColor_LinkSelRectBorder]=color(32,130,185,180);s.Colors[ed::StyleColor_PinRect]=color(24,110,160,100);
    s.Colors[ed::StyleColor_PinRectBorder]=color(32,130,185,180);s.Colors[ed::StyleColor_Flow]=color(32,130,185);
    s.Colors[ed::StyleColor_FlowMarker]=color(235,235,235);s.Colors[ed::StyleColor_GroupBg]=color(0,0,0,80);
    s.Colors[ed::StyleColor_GroupBorder]=color(140,140,140,100);
}
void* bite_create(const char* font_path,float font_size,float ui_scale,const char* ini_path){
    auto s=new State;s->imgui=ImGui::CreateContext();auto& io=ImGui::GetIO();
    io.ConfigFlags|=ImGuiConfigFlags_DockingEnable;io.BackendFlags|=ImGuiBackendFlags_RendererHasVtxOffset;
    if(ini_path&&ini_path[0]){s->ini=ini_path;io.IniFilename=s->ini.c_str();}else{io.IniFilename=nullptr;}
    if(ui_scale<=0)ui_scale=1;io.FontGlobalScale=1/ui_scale;
    if(font_path&&font_path[0]&&font_size>0)io.Fonts->AddFontFromFileTTF(font_path,font_size*ui_scale);
    ed::Config c;c.SettingsFile=nullptr;s->editor=ed::CreateEditor(&c);ed::SetCurrentEditor(s->editor);
    bite_style();bite_node_style();return s;
}
void bite_destroy(void* context){auto s=(State*)context;ed::DestroyEditor(s->editor);ImGui::DestroyContext(s->imgui);delete s;}
void bite_frame(float w,float h,float scale,float dt){auto& io=ImGui::GetIO();io.DisplaySize=ImVec2(w,h);io.DisplayFramebufferScale=ImVec2(scale,scale);io.DeltaTime=dt;ImGui::NewFrame();}
void bite_render(){ImGui::Render();}
void bite_mouse_position(float x,float y){ImGui::GetIO().AddMousePosEvent(x,y);}
void bite_mouse_button(int b,int down){ImGui::GetIO().AddMouseButtonEvent(b,down!=0);}
void bite_mouse_wheel(float x,float y){ImGui::GetIO().AddMouseWheelEvent(x,y);}
void bite_key(int key,int down){ImGui::GetIO().AddKeyEvent((ImGuiKey)key,down!=0);}
void bite_text_input(const char* s){ImGui::GetIO().AddInputCharactersUTF8(s);}
void bite_focus(int f){ImGui::GetIO().AddFocusEvent(f!=0);}
int bite_want_text_input(){return ImGui::GetIO().WantTextInput;}
void bite_font_pixels(const unsigned char** pixels,int* w,int* h){unsigned char* p;ImGui::GetIO().Fonts->GetTexDataAsRGBA32(&p,w,h);*pixels=p;}
void bite_font_texture(uint64_t id){ImGui::GetIO().Fonts->SetTexID((ImTextureID)(uintptr_t)id);}
void bite_set_scale(const char* font_path,float font_size,float ui_scale){
    auto& io=ImGui::GetIO();if(ui_scale<=0)ui_scale=1;io.FontGlobalScale=1/ui_scale;
    io.Fonts->Clear();if(font_path&&font_path[0]&&font_size>0)io.Fonts->AddFontFromFileTTF(font_path,font_size*ui_scale);
}
void bite_dockspace(){
    auto viewport=ImGui::GetMainViewport();
    auto dock=ImGui::DockSpaceOverViewport(0,viewport);
    auto node=ImGui::DockBuilderGetNode(dock);
    if(node && node->IsLeafNode() && node->Windows.empty()) {
        ImGui::DockBuilderRemoveNode(dock);
        ImGui::DockBuilderAddNode(dock,ImGuiDockNodeFlags_DockSpace);
        ImGui::DockBuilderSetNodeSize(dock,viewport->Size);
        auto center=dock;
        auto right=ImGui::DockBuilderSplitNode(center,ImGuiDir_Right,0.25f,nullptr,&center);
        auto bottom=ImGui::DockBuilderSplitNode(center,ImGuiDir_Down,0.15f,nullptr,&center);
        auto left=ImGui::DockBuilderSplitNode(center,ImGuiDir_Left,0.22f,nullptr,&center);
        auto preview=ImGui::DockBuilderSplitNode(right,ImGuiDir_Down,0.5f,nullptr,&right);
        ImGui::DockBuilderDockWindow("Library",left);
        ImGui::DockBuilderDockWindow("Canvas",center);
        ImGui::DockBuilderDockWindow("Inspector",right);
        ImGui::DockBuilderDockWindow("Preview",preview);
        ImGui::DockBuilderDockWindow("Filmstrip",bottom);
        ImGui::DockBuilderFinish(dock);
    }
}
int bite_main_menu_bar_begin(){return ImGui::BeginMainMenuBar();}
void bite_main_menu_bar_end(){ImGui::EndMainMenuBar();}
int bite_menu_begin(const char* label){return ImGui::BeginMenu(label);}
void bite_menu_end(){ImGui::EndMenu();}
int bite_menu_item(const char* label,const char* shortcut,int selected,int enabled){return ImGui::MenuItem(label,shortcut,selected!=0,enabled!=0);}
int bite_begin(const char* s){return ImGui::Begin(s);}
void bite_end(){ImGui::End();}
void bite_text(const char* s){ImGui::TextUnformatted(s);}
int bite_button(const char* s){return ImGui::Button(s);}
int bite_drag_float(const char* s,float* v){return ImGui::DragFloat(s,v,0.1f);}
int bite_slider_float(const char* s,float* v,float min,float max){return ImGui::SliderFloat(s,v,min,max);}
int bite_drag_float_n(const char* s,float* v,int count){if(count==2)return ImGui::DragFloat2(s,v,0.1f);if(count==3)return ImGui::DragFloat3(s,v,0.1f);if(count==4)return ImGui::DragFloat4(s,v,0.1f);return 0;}
int bite_color_edit4(const char* s,float* v){return ImGui::ColorEdit4(s,v);}
int bite_input_text(const char* s,char* data,size_t size){return ImGui::InputText(s,data,size);}
int bite_checkbox(const char* s,int* value){bool checked=*value!=0;bool changed=ImGui::Checkbox(s,&checked);*value=checked?1:0;return changed;}
int bite_combo(const char* s,int* current,const char* items){return ImGui::Combo(s,current,items);}
void bite_next_item_full_width(){ImGui::SetNextItemWidth(-FLT_MIN);}
void bite_begin_disabled(int disabled){ImGui::BeginDisabled(disabled!=0);}
void bite_end_disabled(){ImGui::EndDisabled();}
void bite_image(uint64_t id,float w,float h){ImGui::Image((ImTextureID)(uintptr_t)id,ImVec2(w,h));}
int bite_image_button(const char* id,uint64_t texture,float w,float h){return ImGui::ImageButton(id,(ImTextureID)(uintptr_t)texture,ImVec2(w,h));}
void bite_same_line(){ImGui::SameLine();}
void bite_editor_begin(void* context,const char* label){ed::SetCurrentEditor(((State*)context)->editor);ed::Begin(label);}
void bite_editor_end(){ed::End();}
void bite_node_begin(uint64_t id){ed::BeginNode(ed::NodeId((uintptr_t)id));}
void bite_node_end(){ed::EndNode();}
void bite_node_header(const char* text,float r,float g,float b){
    const ImVec2 start=ImGui::GetCursorScreenPos();
    const float width=150.0f, height=28.0f;
    ImGui::Dummy(ImVec2(width,height));
    auto* draw=ImGui::GetWindowDrawList();
    draw->AddRectFilled(start,ImVec2(start.x+width,start.y+height),ImColor(r,g,b,1.0f),6.0f,ImDrawFlags_RoundCornersTop);
    draw->AddText(ImVec2(start.x+8.0f,start.y+(height-ImGui::GetTextLineHeight())*0.5f),ImGui::GetColorU32(ImGuiCol_Text),text);
}
void bite_typed_pin(uint64_t id,int output,const char* label,float r,float g,float b){
    ed::BeginPin(ed::PinId((uintptr_t)id),output?ed::PinKind::Output:ed::PinKind::Input);
    const ImVec2 cursor=ImGui::GetCursorScreenPos();
    const float width=150.0f;
    const ImVec2 size=ImGui::CalcTextSize(label);
    if(output) ImGui::SetCursorScreenPos(ImVec2(cursor.x+width-size.x-8.0f,cursor.y));
    else ImGui::SetCursorScreenPos(ImVec2(cursor.x+8.0f,cursor.y));
    ImGui::TextColored(ImVec4(r,g,b,1.0f),"%s",label);
    const ImVec2 a=ImGui::GetItemRectMin(), z=ImGui::GetItemRectMax();
    const float x=output?cursor.x+width:cursor.x;
    ImGui::GetWindowDrawList()->AddCircleFilled(ImVec2(x,(a.y+z.y)*0.5f),5.0f,ImColor(r,g,b,1.0f));
    ed::EndPin();
}
void bite_pin_begin(uint64_t id,int output){ed::BeginPin(ed::PinId((uintptr_t)id),output?ed::PinKind::Output:ed::PinKind::Input);}
void bite_pin_end(){ed::EndPin();}
void bite_link(uint64_t id,uint64_t a,uint64_t b,float r,float g,float blue){ed::Link(ed::LinkId((uintptr_t)id),ed::PinId((uintptr_t)a),ed::PinId((uintptr_t)b),ImColor(r,g,blue),2);}
int bite_new_link(uint64_t* source,uint64_t* target){int accepted=0;if(ed::BeginCreate()){ed::PinId a,b;if(ed::QueryNewLink(&a,&b)&&a&&b&&ed::AcceptNewItem()){*source=a.Get();*target=b.Get();accepted=1;}}ed::EndCreate();return accepted;}
int bite_new_node(uint64_t* pin){int accepted=0;if(ed::BeginCreate()){ed::PinId p;if(ed::QueryNewNode(&p)&&ed::AcceptNewItem()){*pin=p.Get();accepted=1;}}ed::EndCreate();return accepted;}
int bite_deleted_link(uint64_t* id){int accepted=0;if(ed::BeginDelete()){ed::LinkId link;if(ed::QueryDeletedLink(&link)&&ed::AcceptDeletedItem()){*id=link.Get();accepted=1;}}ed::EndDelete();return accepted;}
void bite_set_node_position(uint64_t id,float x,float y){ed::SetNodePosition(ed::NodeId((uintptr_t)id),ImVec2(x,y));}
void bite_get_node_position(uint64_t id,float* x,float* y){auto p=ed::GetNodePosition(ed::NodeId((uintptr_t)id));*x=p.x;*y=p.y;}
void bite_get_node_size(uint64_t id,float* width,float* height){auto s=ed::GetNodeSize(ed::NodeId((uintptr_t)id));*width=s.x;*height=s.y;}
int bite_node_selected(uint64_t id){return ed::IsNodeSelected(ed::NodeId((uintptr_t)id));}
void bite_group(float w,float h){ed::Group(ImVec2(w,h));}
int bite_background_menu(){return ed::ShowBackgroundContextMenu();}
void bite_canvas_mouse_position(float* x,float* y){auto p=ed::ScreenToCanvas(ImGui::GetMousePos());*x=p.x;*y=p.y;}
int bite_editor_dragging_selection(){return ImGui::IsMouseDragging(ImGuiMouseButton_Left)&&ed::GetSelectedObjectCount()>0;}
void bite_open_popup(const char* id){ImGui::OpenPopup(id);}
int bite_begin_popup(const char* id){return ImGui::BeginPopup(id);}
void bite_end_popup(){ImGui::EndPopup();}
void bite_close_popup(){ImGui::CloseCurrentPopup();}
void bite_navigate(){ed::NavigateToContent(0.0f);}
int bite_draw_list_count(){return ImGui::GetDrawData()->CmdListsCount;}
int bite_vertex_count(int i){return ImGui::GetDrawData()->CmdLists[i]->VtxBuffer.Size;}
int bite_index_count(int i){return ImGui::GetDrawData()->CmdLists[i]->IdxBuffer.Size;}
int bite_command_count(int i){return ImGui::GetDrawData()->CmdLists[i]->CmdBuffer.Size;}
void bite_copy_vertices(int i,BiteVertex* dst){static_assert(sizeof(BiteVertex)==sizeof(ImDrawVert));auto& b=ImGui::GetDrawData()->CmdLists[i]->VtxBuffer;memcpy(dst,b.Data,b.Size*sizeof(BiteVertex));}
void bite_copy_indices(int i,uint32_t* dst){auto& b=ImGui::GetDrawData()->CmdLists[i]->IdxBuffer;for(int j=0;j<b.Size;++j)dst[j]=b[j];}
int bite_get_command(int i,int j,BiteCommand* dst){auto& c=ImGui::GetDrawData()->CmdLists[i]->CmdBuffer[j];if(c.UserCallback)return 0;dst->count=c.ElemCount;dst->index_offset=c.IdxOffset;dst->vertex_offset=c.VtxOffset;dst->clip[0]=c.ClipRect.x;dst->clip[1]=c.ClipRect.y;dst->clip[2]=c.ClipRect.z;dst->clip[3]=c.ClipRect.w;dst->texture=(uint64_t)(uintptr_t)c.TextureId;return 1;}
