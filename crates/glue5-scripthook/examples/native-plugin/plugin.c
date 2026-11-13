#include "../../include/glue5_scripthook.h"

static const Glue5HostApi *g_host;
static uint32_t g_wait_count;

static void on_script_wait(void *env, uint64_t info, void *user) {
    (void)env;
    (void)info;
    (void)user;
    if (++g_wait_count == 1 && g_host && g_host->log) {
        g_host->log(1, "Example Android ScriptHook plugin callback active");
    }
}

int32_t glue5_scripthook_plugin_init(const Glue5HostApi *host) {
    if (!host || host->abi_version != GLUE5_SCRIPTHOOK_ABI_VERSION ||
        !host->register_script_callback || !host->call_native ||\n        !host->script_stack_alloc || !host->script_stack_top || !host->script_stack_restore) {
        return -1;
    }
    g_host = host;
    return host->register_script_callback(on_script_wait, 0);
}
