#include "../../include/glue5_scripthook.h"

/*
 * Minimal Android-native plugin example.
 *
 * Build this source as an arm64-v8a shared library named sample.so and put
 * it in the app's user-data/scripthook/plugins/ directory. This example
 * resolves GET_GAME_TIMER by name (never a hard-coded native hash), checks
 * its generated signature, then calls it from the script WAIT callback.
 */
static const Glue5HostApi *g_host;
static uint64_t g_timer_hash;
static uint32_t g_wait_count;

static void on_script_wait(void *env, uint64_t info, void *user) {
    (void)info;
    (void)user;

    if (!g_host || !g_host->call_native_marshaled) return;

    /* Keep the demonstration quiet: log once every ~5 seconds at 60 FPS. */
    if (++g_wait_count % 300u != 0u) return;

    uint64_t timer_bits = 0;
    int32_t status = g_host->call_native_marshaled(env, g_timer_hash, 0, 0, &timer_bits);
    if (status == 0 && g_host->log) {
        g_host->log(1, "ScriptHook sample: GET_GAME_TIMER call succeeded");
    } else if (g_host->log) {
        g_host->log(2, "ScriptHook sample: GET_GAME_TIMER call failed");
    }
}

int32_t glue5_scripthook_plugin_init(const Glue5HostApi *host) {
    if (!host || host->abi_version != GLUE5_SCRIPTHOOK_ABI_VERSION ||
        !host->register_script_callback || !host->native_hash_by_name ||
        !host->native_metadata || !host->call_native_marshaled)
        return -1;

    Glue5NativeMetadata metadata = {0};
    if (host->native_hash_by_name("GET_GAME_TIMER", &g_timer_hash) != 0 ||
        host->native_metadata(g_timer_hash, &metadata) != 0 ||
        metadata.parameter_count != 0)
        return -2;

    g_host = host;
    if (host->log) host->log(1, "ScriptHook sample initialized; native signature verified");
    return host->register_script_callback(on_script_wait, 0);
}
