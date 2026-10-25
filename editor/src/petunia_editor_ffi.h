// C interface of the Rust editor library (crates/petunia_editor_ffi).
// Every `char*` returned here must be released with petunia_string_free.
#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct EditorHandle EditorHandle;
typedef const void* (*PetuniaGlLoader)(const char* name);

EditorHandle* petunia_editor_create(void);
void petunia_editor_destroy(EditorHandle* handle);

// JSON request in, JSON response out: {"ok":true,"result":...} or {"ok":false,"error":{...}}.
char* petunia_editor_call(EditorHandle* handle, const char* request_json);

// Both return NULL on success, or an error message.
char* petunia_editor_viewport_init(EditorHandle* handle, PetuniaGlLoader loader);
char* petunia_editor_viewport_render(EditorHandle* handle, uint32_t width, uint32_t height);
void petunia_editor_viewport_release(EditorHandle* handle);

void petunia_string_free(char* text);

#ifdef __cplusplus
}
#endif
