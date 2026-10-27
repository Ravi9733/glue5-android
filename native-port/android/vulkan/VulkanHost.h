#pragma once

#include <android/native_window.h>
#ifndef VK_USE_PLATFORM_ANDROID_KHR
#define VK_USE_PLATFORM_ANDROID_KHR 1
#endif
#include <vulkan/vulkan.h>
#include <cstdint>
#include <vector>

// Initial Android Vulkan device/swapchain host. This is the platform layer,
// not yet a translation of Mugoat's D3D11-style command ring.
namespace mugoat::vkhost {

struct Host {
  VkInstance instance = VK_NULL_HANDLE;
  VkSurfaceKHR surface = VK_NULL_HANDLE;
  VkPhysicalDevice physical = VK_NULL_HANDLE;
  VkDevice device = VK_NULL_HANDLE;
  VkQueue graphics_queue = VK_NULL_HANDLE;
  VkQueue present_queue = VK_NULL_HANDLE;
  uint32_t graphics_family = UINT32_MAX;
  uint32_t present_family = UINT32_MAX;
  VkSwapchainKHR swapchain = VK_NULL_HANDLE;
  VkFormat surface_format = VK_FORMAT_UNDEFINED;
  VkExtent2D extent{};
  std::vector<VkImage> images;
  std::vector<VkImageView> views;
  std::vector<VkSemaphore> acquired;
  std::vector<VkSemaphore> rendered;
  VkCommandPool command_pool = VK_NULL_HANDLE;
  std::vector<VkImageLayout> image_layouts;
  uint32_t frame = 0;
  bool ready = false;
};

// Caller retains ownership of window; it must stay valid until shutdown.
bool initialize(Host* host, ANativeWindow* window, int32_t width, int32_t height);
bool resize(Host* host, int32_t width, int32_t height);
// Smoke-test frame path: clears and presents a Vulkan frame. This is not yet
// the Mugoat command-ring renderer.
bool present_clear_frame(Host* host, float red, float green, float blue, float alpha);
void shutdown(Host* host);

}  // namespace mugoat::vkhost
