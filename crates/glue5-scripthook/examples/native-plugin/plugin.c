#include "../../include/glue5_scripthook.h"
#include <string.h>

_Static_assert(sizeof(Glue5NativeArg) == 16, "Glue5NativeArg ABI layout changed");
_Static_assert(sizeof(Glue5NativeMetadata) == 24, "Glue5NativeMetadata ABI layout changed");

/*
 * Native-only ScriptHook spawn sample. UP/DOWN selects vehicle or ped,
 * SELECT spawns, BACK deletes the last spawned entity. No Luau/.NET runtime.
 * Build as arm64-v8a sample.so and place in user-data/scripthook/plugins/.
 *
 * This API currently resolves vehicle/ped archetypes, not object archetypes.
 * Spawn point is resolved 4 metres in front of the local player.
 */
enum { SPAWN_VEHICLE = 0, SPAWN_PED = 1 };
static const Glue5HostApi *g_host;
static uint32_t g_model_hash[2];
static uint64_t g_create_hash[2];
static int32_t g_entity_handle;
static uint32_t g_entity_pool;
static int g_selection;
static int g_pending_selection = -1;

static uint32_t f32_bits(float value) {
    uint32_t bits = 0;
    memcpy(&bits, &value, sizeof(bits));
    return bits;
}

/* Verify every argument against this build's generated native metadata. */
static int verify_native(const char *name, const uint32_t *kinds,
                         uint32_t count, uint64_t *hash) {
    Glue5NativeMetadata metadata = {0};
    if (g_host->native_hash_by_name(name, hash) != 0 ||
        g_host->native_metadata(*hash, &metadata) != 0 ||
        metadata.parameter_count != count)
        return -1;
    for (uint32_t i = 0; i < count; ++i) {
        uint32_t kind = 0, direction = 99;
        if (g_host->native_parameter(*hash, i, &kind, &direction) != 0 ||
            kind != kinds[i] || direction != 0)
            return -2;
    }
    return 0;
}

static int spawn_selected(void *env, int selection) {
    float spawn[4] = {0};
    int32_t transform_status = g_host->get_player_spawn_transform(env, 4.0f, spawn);
    if (transform_status != 0) {
        g_host->release_model(env, g_model_hash[selection]);
        g_pending_selection = -1;
        if (g_host->log) g_host->log(2, "ScriptHook: could not read player spawn transform");
        return transform_status;
    }
    Glue5NativeArg vehicle_args[7] = {
        {1, 0, g_model_hash[SPAWN_VEHICLE]},
        {2, 0, f32_bits(spawn[0])}, {2, 0, f32_bits(spawn[1])},
        {2, 0, f32_bits(spawn[2])}, {2, 0, f32_bits(spawn[3])},
        {4, 0, 0}, {4, 0, 0}
    };
    Glue5NativeArg ped_args[8] = {
        {0, 0, 4}, {1, 0, g_model_hash[SPAWN_PED]},
        {2, 0, f32_bits(spawn[0])}, {2, 0, f32_bits(spawn[1])},
        {2, 0, f32_bits(spawn[2])}, {2, 0, f32_bits(spawn[3])},
        {4, 0, 0}, {4, 0, 0}
    };
    int32_t handle = 0;
    uint32_t pool = selection == SPAWN_VEHICLE ? 2u : 1u;
    int32_t status = selection == SPAWN_VEHICLE
        ? g_host->create_entity(env, g_create_hash[SPAWN_VEHICLE], vehicle_args, 7, pool, &handle)
        : g_host->create_entity(env, g_create_hash[SPAWN_PED], ped_args, 8, pool, &handle);

    g_host->release_model(env, g_model_hash[selection]);
    g_pending_selection = -1;
    if (status == 0) {
        g_entity_handle = handle;
        g_entity_pool = pool;
        if (g_host->log) g_host->log(1, selection == SPAWN_VEHICLE
            ? "ScriptHook: Adder spawned" : "ScriptHook: security ped spawned");
    } else if (g_host->log) {
        g_host->log(2, "ScriptHook: entity creation failed");
    }
    return status;
}

static void on_script_wait(void *env, uint64_t info, void *user) {
    (void)info; (void)user;
    if (!g_host || !env) return;

    if (g_host->is_control_just_pressed(env, 0, 172) == 1) {
        g_selection = SPAWN_VEHICLE;
        if (g_host->log) g_host->log(1, "ScriptHook selection: vehicle (Adder)");
    }
    if (g_host->is_control_just_pressed(env, 0, 173) == 1) {
        g_selection = SPAWN_PED;
        if (g_host->log) g_host->log(1, "ScriptHook selection: ped (security)");
    }

    if (g_host->is_control_just_pressed(env, 0, 177) == 1) {
        if (g_pending_selection >= 0) {
            g_host->release_model(env, g_model_hash[g_pending_selection]);
            g_pending_selection = -1;
        } else if (g_entity_handle &&
                   g_host->validate_entity(env, g_entity_handle, g_entity_pool) == 1) {
            if (g_host->delete_entity(env, g_entity_handle, g_entity_pool) == 0) {
                g_entity_handle = 0;
                g_entity_pool = 0;
                if (g_host->log) g_host->log(1, "ScriptHook: last spawned entity deleted");
            }
        }
    }

    if (g_host->is_control_just_pressed(env, 0, 176) == 1) {
        if (g_pending_selection >= 0) {
            if (g_host->log) g_host->log(2, "ScriptHook: model request already pending");
        } else if (g_entity_handle &&
                   g_host->validate_entity(env, g_entity_handle, g_entity_pool) == 1) {
            if (g_host->log) g_host->log(2, "ScriptHook: delete current entity with BACK first");
        } else {
            g_pending_selection = g_selection;
            }
    }

    if (g_pending_selection < 0) return;
    int selection = g_pending_selection;
    int32_t loaded = g_host->request_model(env, g_model_hash[selection]);
    if (loaded == 1) {
        spawn_selected(env, selection);
    } else if (loaded < 0) {
        g_pending_selection = -1;
        if (g_host->log) g_host->log(2, "ScriptHook: model streaming failed");
    } else {
    }
}

int32_t glue5_scripthook_plugin_init(const Glue5HostApi *host) {
    if (!host || host->abi_version != GLUE5_SCRIPTHOOK_ABI_VERSION ||
        !host->register_script_callback || !host->native_hash_by_name ||
        !host->native_metadata || !host->native_parameter ||
        !host->resolve_model_name || !host->request_model || !host->release_model ||
        !host->create_entity || !host->delete_entity || !host->validate_entity ||
        !host->is_control_just_pressed || !host->get_player_spawn_transform)
        return -1;

    g_host = host;
    if (host->resolve_model_name("adder", 1, &g_model_hash[SPAWN_VEHICLE]) != 1)
        return -2;
    if (host->resolve_model_name("s_m_y_cop_01", 2, &g_model_hash[SPAWN_PED]) != 1)
        return -3;

    const uint32_t vehicle_kinds[7] = {1, 2, 2, 2, 2, 4, 4};
    const uint32_t ped_kinds[8] = {0, 1, 2, 2, 2, 2, 4, 4};
    if (verify_native("CREATE_VEHICLE", vehicle_kinds, 7,
                      &g_create_hash[SPAWN_VEHICLE]) != 0)
        return -4;
    if (verify_native("CREATE_PED", ped_kinds, 8,
                      &g_create_hash[SPAWN_PED]) != 0)
        return -5;

    if (host->log) host->log(1, "Native ScriptHook spawn sample ready: UP vehicle, DOWN ped, SELECT spawn, BACK delete");
    return host->register_script_callback(on_script_wait, 0);
}
