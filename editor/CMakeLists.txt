cmake_minimum_required(VERSION 3.21)
project(petunia_editor LANGUAGES CXX)

# The editor is a thin Qt shell. The engine lives in Rust and is linked as a static library:
#   cargo build -p petunia_editor_ffi   ->   libpetunia_editor_ffi.a
# `scripts/editor.sh` builds both and passes the path below.
set(PETUNIA_FFI_LIB "" CACHE FILEPATH "Path to libpetunia_editor_ffi.a")
if(NOT EXISTS "${PETUNIA_FFI_LIB}")
    message(FATAL_ERROR "PETUNIA_FFI_LIB must point to libpetunia_editor_ffi.a (run scripts/editor.sh)")
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
# The compilation database lets clangd/clang-tidy resolve Qt's include paths. It is written to the
# build directory, and `scripts/editor.sh` links it next to the sources. Without it the editor shows
# spurious errors such as 'QJsonObject file not found'.
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

find_package(Qt6 6.4 REQUIRED COMPONENTS Widgets OpenGLWidgets Svg)
find_package(Threads REQUIRED)

add_executable(petunia-editor
    src/main.cpp
    src/engine_bridge.cpp
    src/viewport_widget.cpp
    src/viewport_header.cpp
    src/viewport_overlay.cpp
    src/ui_kit/petunia_nav_cube.cpp
    src/ui_kit/shell/petunia_pie_menu.cpp
    src/hierarchy_panel.cpp
    src/inspector_panel.cpp
    src/info_panels.cpp
    src/code_editor.cpp
    src/code_panel.cpp
    src/graph_panel.cpp
    src/node_canvas.cpp
    src/main_window.cpp
    src/panel_title_bar.cpp
    src/command_palette.cpp
    src/ui_kit/shell/petunia_segmented.cpp
    src/appearance.cpp
    src/ui_kit/petunia_theme.cpp
    src/ui_kit/petunia_scrub.cpp
    src/ui_kit/petunia_number_field.cpp
    src/ui_kit/petunia_vector3_field.cpp
)
target_include_directories(petunia-editor PRIVATE src src/ui_kit)

# Icons (":/icons/<name>.svg") and themes (":/themes/<id>.json") are embedded in the executable.
file(GLOB PETUNIA_ICONS CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/assets/icons/*.svg")
file(GLOB PETUNIA_THEMES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/assets/themes/*.json")
qt_add_resources(petunia-editor "icons" PREFIX "/icons"
    BASE "${CMAKE_CURRENT_SOURCE_DIR}/assets/icons" FILES ${PETUNIA_ICONS})
qt_add_resources(petunia-editor "themes" PREFIX "/themes"
    BASE "${CMAKE_CURRENT_SOURCE_DIR}/assets/themes" FILES ${PETUNIA_THEMES})
target_compile_options(petunia-editor PRIVATE -Wall -Wextra -Wpedantic)
target_link_libraries(petunia-editor PRIVATE
    Qt6::Widgets Qt6::OpenGLWidgets Qt6::Svg
    "${PETUNIA_FFI_LIB}"
    Threads::Threads ${CMAKE_DL_LIBS} m
)
