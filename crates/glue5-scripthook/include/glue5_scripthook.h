#ifndef GLUE5_SCRIPTHOOK_H
#define GLUE5_SCRIPTHOOK_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#define GLUE5_SCRIPTHOOK_ABI_VERSION 4u
typedef void (*Glue5ScriptCallback)(void *env, uint64_t info, void *user);
typedef void (*Glue5LogCallback)(uint32_t level, const char *message);
typedef int32_t (*Glue5RegisterScriptCallback)(Glue5ScriptCallback callback, void *user);
/* kind: 0=signed integer, 1=unsigned/hash, 2=f32 bit pattern, 3=wasm-memory offset. */
typedef struct Glue5NativeArg { uint32_t kind; uint32_t reserved; uint64_t value; } Glue5NativeArg;
typedef struct Glue5HostApi {
    uint32_t abi_version;
    Glue5LogCallback log;
    Glue5RegisterScriptCallback register_script_callback;
    uint64_t (*script_stack_top)(void *env);
    uint64_t (*script_stack_alloc)(void *env, uint64_t size);
    int32_t (*script_stack_restore)(void *env, uint64_t top);
    /* Low-level: info must point to a prepared scrThread::Info. */
    int32_t (*call_native)(void *env, uint64_t hash, uint64_t info);
    /* Typed scalar call. Result is returned as raw bits through result_bits. */
    int32_t (*call_native_typed)(void *env, uint64_t hash, const Glue5NativeArg *args,
                                 uint32_t count, uint64_t *result_bits);
} Glue5HostApi;
int32_t glue5_scripthook_plugin_init(const Glue5HostApi *host);
#ifdef __cplusplus
}
#endif
#endif
