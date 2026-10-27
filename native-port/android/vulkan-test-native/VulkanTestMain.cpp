#include "VulkanHost.h"

#include <android/log.h>
#include <android_native_app_glue.h>
#include <algorithm>
#include <cstdint>

#define TEST_LOGI(...) __android_log_print(ANDROID_LOG_INFO, "MugoatVulkanTest", __VA_ARGS__)
#define TEST_LOGE(...) __android_log_print(ANDROID_LOG_ERROR, "MugoatVulkanTest", __VA_ARGS__)

namespace {
mugoat::vkhost::Host host;
bool initialized = false;

void handle_cmd(android_app* app, int32_t cmd) {
  switch (cmd) {
    case APP_CMD_INIT_WINDOW:
      if (app->window) {
        if (initialized) {
          mugoat::vkhost::shutdown(&host);
          initialized = false;
        }
        const int32_t width = ANativeWindow_getWidth(app->window);
        const int32_t height = ANativeWindow_getHeight(app->window);
        initialized = mugoat::vkhost::initialize(&host, app->window, width, height);
        if (initialized) TEST_LOGI("Vulkan smoke test initialized: %dx%d", width, height);
        else TEST_LOGE("Vulkan smoke test initialization failed");
      }
      break;
    case APP_CMD_TERM_WINDOW:
      if (initialized) {
        mugoat::vkhost::shutdown(&host);
        initialized = false;
      }
      break;
    case APP_CMD_WINDOW_RESIZED:
      if (initialized && app->window) {
        initialized = mugoat::vkhost::resize(
            &host, ANativeWindow_getWidth(app->window), ANativeWindow_getHeight(app->window));
      }
      break;
    default:
      break;
  }
}
}  // namespace

extern "C" void android_main(android_app* app) {
  app->onAppCmd = handle_cmd;
  TEST_LOGI("Mugoat Vulkan test APK started; this is a Vulkan surface smoke test, not GTA V gameplay.");
  while (true) {
    int events = 0;
    android_poll_source* source = nullptr;
    const int timeout = initialized ? 16 : -1;
    while (ALooper_pollOnce(timeout, nullptr, &events, reinterpret_cast<void**>(&source)) >= 0) {
      if (source) source->process(app, source);
      if (app->destroyRequested) {
        if (initialized) mugoat::vkhost::shutdown(&host);
        return;
      }
      if (initialized) break;
    }
    if (app->destroyRequested) {
      if (initialized) mugoat::vkhost::shutdown(&host);
      return;
    }
    if (initialized) {
      const float t = static_cast<float>(host.frame % 360U) / 360.0f;
      const bool ok = mugoat::vkhost::present_clear_frame(
          &host, 0.04f + 0.18f * t, 0.08f, 0.22f + 0.22f * (1.0f - t), 1.0f);
      if (!ok) TEST_LOGE("Vulkan test frame failed at frame %u", host.frame);
    }
  }
}
