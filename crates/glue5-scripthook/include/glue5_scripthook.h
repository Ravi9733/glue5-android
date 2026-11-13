#ifndef GLUE5_SCRIPTHOOK_H
#define GLUE5_SCRIPTHOOK_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#define GLUE5_SCRIPTHOOK_ABI_VERSION 7u
typedef void (*Glue5ScriptCallback)(void *env, uint64_t info, void *user);
typedef void (*Glue5LogCallback)(uint32_t level, const char *message);
typedef int32_t (*Glue5RegisterScriptCallback)(Glue5ScriptCallback callback, void *user);
/* NativeArg kinds: 0=i32, 1=u32/hash, 2=f32 bits, 3=wasm-memory offset, 4=bool. */
typedef struct Glue5NativeArg { uint32_t kind; uint32_t reserved; uint64_t value; } Glue5NativeArg;
typedef struct Glue5NativeMetadata { uint64_t hash; uint64_t handler; uint32_t parameter_count; uint32_t return_kind; } Glue5NativeMetadata;
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
    /* Signature-aware: supports scalar, string/vector input, and OUT/INOUT destinations. */
    int32_t (*call_native_marshaled)(void *env, uint64_t hash, const Glue5NativeArg *args,
                                     uint32_t count, uint64_t *result_bits);
    int32_t (*native_metadata)(uint64_t hash, Glue5NativeMetadata *output);
    /* Exact GTA native name, e.g. CREATE_VEHICLE; returns its game-build hash. */
    int32_t (*native_hash_by_name)(const char *name, uint64_t *hash);
    /* expected_kind: 0=any model, 1=vehicle, 2=ped. Returns 1=found, 0=not installed. */
    int32_t (*resolve_model_name)(const char *name, uint32_t expected_kind, uint32_t *model_hash);
    /* expected_pool: 0=any entity, 1=ped, 2=vehicle, 3=object; returns 1/0. */
    int32_t (*validate_entity)(void *env, int32_t handle, uint32_t expected_pool);
    int32_t (*native_parameter)(uint64_t hash, uint32_t index, uint32_t *kind, uint32_t *direction);
    /* pool: 0=peds, 1=vehicles, 2=objects; returns count or negative error. */
    int32_t (*enumerate_entities)(void *env, uint32_t pool, int32_t *output, uint32_t capacity);
} Glue5HostApi;
int32_t glue5_scripthook_plugin_init(const Glue5HostApi *host);
#ifdef __cplusplus
}
#endif
#endif
