#ifndef GLUE5_NATIVE_MOD_H
#define GLUE5_NATIVE_MOD_H

/*
 * Glue5 Android native-mod ABI v1.
 *
 * This header defines the contract only. The Android host does not yet load
 * plugins or promise that these callbacks are callable. Plugins must not
 * retain host pointers beyond the documented lifetime or call game natives
 * directly. Game interaction is queued through the host and serviced on the
 * game script thread in a future runtime implementation.
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define GLUE5_NATIVE_ABI_V1 1u
#define GLUE5_NATIVE_ABI_NAME "glue5-native-v1"
#define GLUE5_NATIVE_ENGINE_BUILD_MAX 64u

typedef struct Glue5HostV1 Glue5HostV1;
typedef struct Glue5ModV1 Glue5ModV1;

typedef enum Glue5StatusV1 {
    GLUE5_STATUS_OK = 0,
    GLUE5_STATUS_INVALID_ARGUMENT = 1,
    GLUE5_STATUS_UNSUPPORTED = 2,
    GLUE5_STATUS_QUEUE_FULL = 3,
    GLUE5_STATUS_NOT_READY = 4,
    GLUE5_STATUS_INTERNAL_ERROR = 5
} Glue5StatusV1;

typedef enum Glue5LogLevelV1 {
    GLUE5_LOG_INFO = 0,
    GLUE5_LOG_WARNING = 1,
    GLUE5_LOG_ERROR = 2
} Glue5LogLevelV1;

/* Host-owned opaque context. Plugins must not inspect or free it. */
struct Glue5HostV1 {
    uint32_t abi_version;
    uint32_t struct_size;
    const char *engine_build; /* UTF-8, host-owned, valid during init only. */
    void *host_context;       /* Opaque; valid only while plugin is active. */

    void (*log)(void *host_context, uint32_t level,
                const char *message, uint32_t message_bytes);

    /*
     * Queue a bounded, versioned command for the game script thread.
     * This is not a raw native-call interface and does not expose engine
     * addresses, WASM pointers, or Android/JNI objects.
     */
    Glue5StatusV1 (*enqueue_command)(void *host_context,
                                     uint32_t command_kind,
                                     const uint8_t *payload,
                                     uint32_t payload_bytes);
};

/* Plugin-owned callback table returned by the exported entry point. */
struct Glue5ModV1 {
    uint32_t abi_version;
    uint32_t struct_size;
    const char *name; /* UTF-8; plugin-owned, valid until shutdown. */

    Glue5StatusV1 (*on_init)(const Glue5HostV1 *host);
    void (*on_tick)(uint64_t game_time_ms, float delta_seconds);
    void (*on_shutdown)(void);
};

/*
 * Each Android ARM64 plugin exports this exact C symbol:
 *   const Glue5ModV1 *glue5_mod_init(const Glue5HostV1 *host);
 *
 * The returned table and all callbacks must remain valid until shutdown.
 * Callbacks must be bounded and must not block, spawn unmanaged threads,
 * access game memory, or invoke game natives directly.
 */
typedef const Glue5ModV1 *(*Glue5ModInitV1)(const Glue5HostV1 *host);

#ifdef __cplusplus
}
#endif

#endif /* GLUE5_NATIVE_MOD_H */
