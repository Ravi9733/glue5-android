#include "../../include/glue5_scripthook.h"
#include <string.h>

_Static_assert(sizeof(Glue5NativeArg) == 16, "Glue5NativeArg ABI layout changed");
_Static_assert(sizeof(Glue5NativeMetadata) == 24, "Glue5NativeMetadata ABI layout changed");

/*
 * Native-only ScriptHook example: press SELECT to request/spawn an Adder;
 * press BACK to delete the last spawned vehicle. No Luau or managed runtime.
 * Build as arm64-v8a sample.so and place in user-data/scripthook/plugins/.
 *
 * The demonstration uses a fixed world position (0, 0, 72); a later player-
 * position helper can replace it. Model streaming is asynchronous: the game
 * thread is never blocked while REQUEST_MODEL is pending.
 */
static const Glue5HostApi *g_host;
static uint32_t g_model_hash;
static uint64_t g_create_vehicle_hash;
static int32_t g_vehicle_handle;
static int g_model_pending;
static int g_spawn_requested;

static uint32_t f32_bits(float value) {
    uint32_t bits = 0;
    memcpy(&bits, &value, sizeof(bits));
    return bits;
}

static int verify_create_vehicle(void) {
    Glue5NativeMetadata metadata = {0};
    if (g_host->native_hash_by_name("CREATE_VEHICLE", &g_create_vehicle_hash) != 0 ||
        g_host->native_metadata(g_create_vehicle_hash, &metadata) != 0 ||
        metadata.parameter_count != 7)
        return -1;

    /* Generated metadata kind codes: float=2, bool=3, hash=4; direction IN=0. */
    const uint32_t kinds[7] = {4, 2, 2, 2, 2, 3, 3};
    for (uint32_t i = 0; i < 7; ++i) {
        uint32_t kind = 0, direction = 99;
        if (g_host->native_parameter(g_create_vehicle_hash, i, &kind, &direction) != 0 ||
            kind != kinds[i] || direction != 0)
            return -2;
    }
    return 0;
}

static int spawn_adder(void *env) {
    Glue5NativeArg args[7] = {
        {1, 0, g_model_hash},
        {2, 0, f32_bits(0.0f)},
        {2, 0, f32_bits(0.0f)},
        {2, 0, f32_bits(72.0f)},
        {2, 0, f32_bits(0.0f)},
        {4, 0, 0},
        {4, 0, 0}
    };
    int32_t handle = 0;
    int32_t status = g_host->create_entity(env, g_create_vehicle_hash, args, 7, 2, &handle);
    if (status == 0) {
        g_vehicle_handle = handle;
        if (g_host->log) g_host->log(1, "ScriptHook: Adder vehicle spawned");
    } else if (g_host->log) {
        g_host->log(2, "ScriptHook: CREATE_VEHICLE failed validation or spawn");
    }
    g_host->release_model(env, g_model_hash);
    g_model_pending = 0;
    g_spawn_requested = 0;
    return status;
}

static void on_script_wait(void *env, uint64_t info, void *user) {
    (void)info;
    (void)user;
    if (!g_host || !env) return;

    if (g_host->is_control_just_pressed(env, 0, 176) == 1) {
        if (g_vehicle_handle && g_host->validate_entity(env, g_vehicle_handle, 2) == 1) {
            if (g_host->log) g_host->log(1, "ScriptHook: vehicle already spawned; press BACK to delete");
        } else {
            g_spawn_requested = 1;
            g_model_pending = 0;
        }
    }

    if (g_host->is_control_just_pressed(env, 0, 177) == 1 && g_vehicle_handle) {
        if (g_host->delete_entity(env, g_vehicle_handle, 2) == 0) {
            g_vehicle_handle = 0;
            if (g_host->log) g_host->log(1, "ScriptHook: spawned vehicle deleted");
        }
    }

    if (!g_spawn_requested) return;
    if (!g_model_pending) {
        int32_t status = g_host->request_model(env, g_model_hash);
        if (status < 0) {
            g_spawn_requested = 0;
            if (g_host->log) g_host->log(2, "ScriptHook: model request failed");
            return;
        }
        g_model_pending = 1;
        if (status == 1) { spawn_adder(env); return; }
    } else {
        int32_t status = g_host->request_model(env, g_model_hash);
        if (status == 1) { spawn_adder(env); return; }
        if (status < 0) {
            g_spawn_requested = 0;
            g_model_pending = 0;
            if (g_host->log) g_host->log(2, "ScriptHook: model streaming failed");
        }
    }
}

int32_t glue5_scripthook_plugin_init(const Glue5HostApi *host) {
    if (!host || host->abi_version != GLUE5_SCRIPTHOOK_ABI_VERSION ||
        !host->register_script_callback || !host->native_hash_by_name ||
        !host->native_metadata || !host->native_parameter ||
        !host->resolve_model_name || !host->request_model || !host->release_model ||
        !host->create_entity || !host->delete_entity || !host->validate_entity ||
        !host->is_control_just_pressed)
        return -1;

    g_host = host;
    if (host->resolve_model_name("adder", 1, &g_model_hash) != 1)
        return -2;
    if (verify_create_vehicle() != 0)
        return -3;

    if (host->log) host->log(1, "Native ScriptHook vehicle sample ready: SELECT spawns, BACK deletes");
    return host->register_script_callback(on_script_wait, 0);
}
