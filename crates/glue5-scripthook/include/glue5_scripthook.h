#ifndef GLUE5_SCRIPTHOOK_H
#define GLUE5_SCRIPTHOOK_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define GLUE5_SCRIPTHOOK_ABI_VERSION 3u

typedef void (*Glue5ScriptCallback)(void *env, uint64_t info, void *user);
typedef void (*Glue5LogCallback)(uint32_t level, const char *message);
typedef int32_t (*Glue5RegisterScriptCallback)(Glue5ScriptCallback callback, void *user);

typedef struct Glue5HostApi {
    uint32_t abi_version;
    Glue5LogCallback log;
    Glue5RegisterScriptCallback register_script_callback;
    uint64_t (*script_stack_top)(void *env);
    uint64_t (*script_stack_alloc)(void *env, uint64_t size);
    int32_t (*script_stack_restore)(void *env, uint64_t top);
    int32_t (*call_native)(void *env, uint64_t hash, uint64_t info);
} Glue5HostApi;

/* Export this symbol from an Android arm64-v8a shared library (.so).
 * Return 0 on success. Do not throw C++ exceptions across this ABI. */
int32_t glue5_scripthook_plugin_init(const Glue5HostApi *host);

#ifdef __cplusplus
}
#endif
#endif
