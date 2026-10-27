#include "VulkanHost.h"

#include <android/log.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <limits>
#include <string>
#include <vector>

#define MUGOAT_LOGE(...) __android_log_print(ANDROID_LOG_ERROR, "MugoatVulkan", __VA_ARGS__)
#define MUGOAT_LOGI(...) __android_log_print(ANDROID_LOG_INFO, "MugoatVulkan", __VA_ARGS__)

namespace mugoat::vkhost {
namespace {

bool ok(VkResult result, const char* operation) {
  if (result == VK_SUCCESS) return true;
  MUGOAT_LOGE("%s failed: VkResult=%d", operation, static_cast<int>(result));
  return false;
}

uint32_t find_family(VkPhysicalDevice physical, VkSurfaceKHR surface, bool present) {
  uint32_t count = 0;
  vkGetPhysicalDeviceQueueFamilyProperties(physical, &count, nullptr);
  std::vector<VkQueueFamilyProperties> families(count);
  vkGetPhysicalDeviceQueueFamilyProperties(physical, &count, families.data());
  for (uint32_t i = 0; i < count; ++i) {
    if (families[i].queueCount == 0) continue;
    if (present) {
      VkBool32 supported = VK_FALSE;
      if (vkGetPhysicalDeviceSurfaceSupportKHR(physical, i, surface, &supported) != VK_SUCCESS || !supported) continue;
    } else if (!(families[i].queueFlags & VK_QUEUE_GRAPHICS_BIT)) {
      continue;
    }
    return i;
  }
  return UINT32_MAX;
}

bool has_device_extension(VkPhysicalDevice physical, const char* wanted) {
  uint32_t count = 0;
  if (vkEnumerateDeviceExtensionProperties(physical, nullptr, &count, nullptr) != VK_SUCCESS) return false;
  std::vector<VkExtensionProperties> extensions(count);
  if (vkEnumerateDeviceExtensionProperties(physical, nullptr, &count, extensions.data()) != VK_SUCCESS) return false;
  return std::any_of(extensions.begin(), extensions.end(), [wanted](const auto& ext) {
    return std::strcmp(ext.extensionName, wanted) == 0;
  });
}

bool select_physical(Host* host) {
  uint32_t count = 0;
  if (!ok(vkEnumeratePhysicalDevices(host->instance, &count, nullptr), "enumerate physical devices") || !count) {
    MUGOAT_LOGE("No Vulkan physical devices found");
    return false;
  }
  std::vector<VkPhysicalDevice> devices(count);
  if (!ok(vkEnumeratePhysicalDevices(host->instance, &count, devices.data()), "read physical devices")) return false;

  for (auto physical : devices) {
    if (!has_device_extension(physical, VK_KHR_SWAPCHAIN_EXTENSION_NAME)) continue;
    const auto graphics = find_family(physical, host->surface, false);
    const auto present = find_family(physical, host->surface, true);
    if (graphics == UINT32_MAX || present == UINT32_MAX) continue;
    uint32_t formats = 0, modes = 0;
    if (vkGetPhysicalDeviceSurfaceFormatsKHR(physical, host->surface, &formats, nullptr) != VK_SUCCESS || !formats) continue;
    if (vkGetPhysicalDeviceSurfacePresentModesKHR(physical, host->surface, &modes, nullptr) != VK_SUCCESS || !modes) continue;
    host->physical = physical;
    host->graphics_family = graphics;
    host->present_family = present;
    VkPhysicalDeviceProperties properties{};
    vkGetPhysicalDeviceProperties(physical, &properties);
    MUGOAT_LOGI("Selected Vulkan GPU: %s (API %u.%u.%u)", properties.deviceName,
      VK_VERSION_MAJOR(properties.apiVersion), VK_VERSION_MINOR(properties.apiVersion),
      VK_VERSION_PATCH(properties.apiVersion));
    return true;
  }
  MUGOAT_LOGE("No device supports graphics, Android surface presentation and VK_KHR_swapchain");
  return false;
}

bool create_device(Host* host) {
  const std::array<uint32_t, 2> families{host->graphics_family, host->present_family};
  std::vector<uint32_t> unique;
  for (auto family : families) if (std::find(unique.begin(), unique.end(), family) == unique.end()) unique.push_back(family);
  const float priority = 1.0f;
  std::vector<VkDeviceQueueCreateInfo> queues;
  for (auto family : unique) {
    VkDeviceQueueCreateInfo queue{VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO};
    queue.queueFamilyIndex = family;
    queue.queueCount = 1;
    queue.pQueuePriorities = &priority;
    queues.push_back(queue);
  }
  const char* extensions[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
  VkPhysicalDeviceFeatures features{};
  VkDeviceCreateInfo create{VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO};
  create.queueCreateInfoCount = static_cast<uint32_t>(queues.size());
  create.pQueueCreateInfos = queues.data();
  create.enabledExtensionCount = 1;
  create.ppEnabledExtensionNames = extensions;
  create.pEnabledFeatures = &features;
  if (!ok(vkCreateDevice(host->physical, &create, nullptr, &host->device), "create device")) return false;
  vkGetDeviceQueue(host->device, host->graphics_family, 0, &host->graphics_queue);
  vkGetDeviceQueue(host->device, host->present_family, 0, &host->present_queue);
  return true;
}

void destroy_swapchain(Host* host) {
  if (host->device != VK_NULL_HANDLE) vkDeviceWaitIdle(host->device);
  for (auto semaphore : host->acquired) if (semaphore) vkDestroySemaphore(host->device, semaphore, nullptr);
  for (auto semaphore : host->rendered) if (semaphore) vkDestroySemaphore(host->device, semaphore, nullptr);
  host->acquired.clear();
  host->rendered.clear();
  for (auto view : host->views) if (view) vkDestroyImageView(host->device, view, nullptr);
  host->views.clear();
  host->images.clear();
  host->image_layouts.clear();
  if (host->swapchain) vkDestroySwapchainKHR(host->device, host->swapchain, nullptr);
  host->swapchain = VK_NULL_HANDLE;
}

bool create_swapchain(Host* host, int32_t requested_width, int32_t requested_height) {
  if (requested_width <= 0 || requested_height <= 0) return false;
  VkSurfaceCapabilitiesKHR caps{};
  if (!ok(vkGetPhysicalDeviceSurfaceCapabilitiesKHR(host->physical, host->surface, &caps), "surface capabilities")) return false;
  uint32_t format_count = 0;
  if (!ok(vkGetPhysicalDeviceSurfaceFormatsKHR(host->physical, host->surface, &format_count, nullptr), "surface format count") || !format_count) return false;
  std::vector<VkSurfaceFormatKHR> formats(format_count);
  if (!ok(vkGetPhysicalDeviceSurfaceFormatsKHR(host->physical, host->surface, &format_count, formats.data()), "surface formats")) return false;
  auto chosen = formats.front();
  for (const auto& candidate : formats) {
    if (candidate.format == VK_FORMAT_B8G8R8A8_UNORM && candidate.colorSpace == VK_COLOR_SPACE_SRGB_NONLINEAR_KHR) {
      chosen = candidate;
      break;
    }
  }
  VkExtent2D extent{};
  if (caps.currentExtent.width != std::numeric_limits<uint32_t>::max()) {
    extent = caps.currentExtent;
  } else {
    extent.width = std::clamp(static_cast<uint32_t>(requested_width), caps.minImageExtent.width, caps.maxImageExtent.width);
    extent.height = std::clamp(static_cast<uint32_t>(requested_height), caps.minImageExtent.height, caps.maxImageExtent.height);
  }
  uint32_t image_count = caps.minImageCount + 1;
  if (caps.maxImageCount && image_count > caps.maxImageCount) image_count = caps.maxImageCount;

  VkSwapchainCreateInfoKHR create{VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR};
  create.surface = host->surface;
  create.minImageCount = image_count;
  create.imageFormat = chosen.format;
  create.imageColorSpace = chosen.colorSpace;
  create.imageExtent = extent;
  create.imageArrayLayers = 1;
  create.imageUsage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT;
  const uint32_t queue_families[] = {host->graphics_family, host->present_family};
  if (host->graphics_family != host->present_family) {
    create.imageSharingMode = VK_SHARING_MODE_CONCURRENT;
    create.queueFamilyIndexCount = 2;
    create.pQueueFamilyIndices = queue_families;
  } else {
    create.imageSharingMode = VK_SHARING_MODE_EXCLUSIVE;
  }
  create.preTransform = caps.currentTransform;
  create.compositeAlpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR;
  create.presentMode = VK_PRESENT_MODE_FIFO_KHR;
  create.clipped = VK_TRUE;
  if (!ok(vkCreateSwapchainKHR(host->device, &create, nullptr, &host->swapchain), "create swapchain")) return false;
  host->surface_format = chosen.format;
  host->extent = extent;

  uint32_t actual_count = 0;
  if (!ok(vkGetSwapchainImagesKHR(host->device, host->swapchain, &actual_count, nullptr), "swapchain image count")) return false;
  host->images.resize(actual_count);
  if (!ok(vkGetSwapchainImagesKHR(host->device, host->swapchain, &actual_count, host->images.data()), "swapchain images")) return false;
  host->views.reserve(actual_count);
  host->acquired.reserve(actual_count);
  host->rendered.reserve(actual_count);
  for (auto image : host->images) {
    VkImageViewCreateInfo view{VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO};
    view.image = image;
    view.viewType = VK_IMAGE_VIEW_TYPE_2D;
    view.format = host->surface_format;
    view.components = {VK_COMPONENT_SWIZZLE_IDENTITY, VK_COMPONENT_SWIZZLE_IDENTITY, VK_COMPONENT_SWIZZLE_IDENTITY, VK_COMPONENT_SWIZZLE_IDENTITY};
    view.subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    VkImageView image_view = VK_NULL_HANDLE;
    if (!ok(vkCreateImageView(host->device, &view, nullptr, &image_view), "create swapchain image view")) return false;
    host->views.push_back(image_view);
    VkSemaphoreCreateInfo semaphore{VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO};
    VkSemaphore a = VK_NULL_HANDLE, b = VK_NULL_HANDLE;
    if (!ok(vkCreateSemaphore(host->device, &semaphore, nullptr, &a), "create acquire semaphore")) return false;
    if (!ok(vkCreateSemaphore(host->device, &semaphore, nullptr, &b), "create render semaphore")) {
      vkDestroySemaphore(host->device, a, nullptr);
      return false;
    }
    host->acquired.push_back(a);
    host->rendered.push_back(b);
  }
  host->image_layouts.assign(actual_count, VK_IMAGE_LAYOUT_UNDEFINED);
  host->frame = 0;
  MUGOAT_LOGI("Swapchain ready: %ux%u, %u images, format=%d", extent.width, extent.height, actual_count, static_cast<int>(host->surface_format));
  return true;
}

}  // namespace

bool initialize(Host* host, ANativeWindow* window, int32_t width, int32_t height) {
  if (!host || !window || width <= 0 || height <= 0) return false;
  *host = Host{};
  const char* extensions[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_ANDROID_SURFACE_EXTENSION_NAME};
  VkApplicationInfo app{VK_STRUCTURE_TYPE_APPLICATION_INFO};
  app.pApplicationName = "Mugoat Android";
  app.applicationVersion = VK_MAKE_API_VERSION(0, 0, 1, 0);
  app.pEngineName = "Mugoat";
  app.engineVersion = VK_MAKE_API_VERSION(0, 0, 1, 0);
  app.apiVersion = VK_API_VERSION_1_1;
  VkInstanceCreateInfo instance_info{VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO};
  instance_info.pApplicationInfo = &app;
  instance_info.enabledExtensionCount = 2;
  instance_info.ppEnabledExtensionNames = extensions;
  if (!ok(vkCreateInstance(&instance_info, nullptr, &host->instance), "create instance")) goto fail;
  {
    VkAndroidSurfaceCreateInfoKHR surface_info{VK_STRUCTURE_TYPE_ANDROID_SURFACE_CREATE_INFO_KHR};
    surface_info.window = window;
    if (!ok(vkCreateAndroidSurfaceKHR(host->instance, &surface_info, nullptr, &host->surface), "create Android surface")) goto fail;
  }
  if (!select_physical(host) || !create_device(host) || !create_swapchain(host, width, height)) goto fail;
  {
    VkCommandPoolCreateInfo pool{VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO};
    pool.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
    pool.queueFamilyIndex = host->graphics_family;
    if (!ok(vkCreateCommandPool(host->device, &pool, nullptr, &host->command_pool), "create command pool")) goto fail;
  }
  host->ready = true;
  return true;
fail:
  shutdown(host);
  return false;
}

bool resize(Host* host, int32_t width, int32_t height) {
  if (!host || !host->device || width <= 0 || height <= 0) return false;
  destroy_swapchain(host);
  return create_swapchain(host, width, height);
}

bool present_clear_frame(Host* host, float red, float green, float blue, float alpha) {
  if (!host || !host->ready || !host->device || !host->swapchain || !host->command_pool) return false;
  uint32_t image_index = 0;
  const VkResult acquired = vkAcquireNextImageKHR(host->device, host->swapchain, UINT64_MAX,
      host->acquired[host->frame % host->acquired.size()], VK_NULL_HANDLE, &image_index);
  if (acquired == VK_ERROR_OUT_OF_DATE_KHR) return false;
  if (acquired != VK_SUCCESS && acquired != VK_SUBOPTIMAL_KHR) return ok(acquired, "acquire swapchain image");
  VkCommandBufferAllocateInfo allocation{VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO};
  allocation.commandPool = host->command_pool;
  allocation.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
  allocation.commandBufferCount = 1;
  VkCommandBuffer command = VK_NULL_HANDLE;
  if (!ok(vkAllocateCommandBuffers(host->device, &allocation, &command), "allocate frame command buffer")) return false;
  VkCommandBufferBeginInfo begin{VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
  begin.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
  bool recorded = ok(vkBeginCommandBuffer(command, &begin), "begin frame command buffer");
  if (recorded) {
    VkImageMemoryBarrier to_clear{VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER};
    to_clear.oldLayout = host->image_layouts[image_index];
    to_clear.newLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
    to_clear.srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    to_clear.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    to_clear.image = host->images[image_index];
    to_clear.subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    to_clear.srcAccessMask = host->image_layouts[image_index] == VK_IMAGE_LAYOUT_UNDEFINED ? 0 : VK_ACCESS_MEMORY_READ_BIT;
    to_clear.dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
    vkCmdPipelineBarrier(command, host->image_layouts[image_index] == VK_IMAGE_LAYOUT_UNDEFINED ? VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT : VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
        VK_PIPELINE_STAGE_TRANSFER_BIT, 0, 0, nullptr, 0, nullptr, 1, &to_clear);
    VkClearColorValue color{{red, green, blue, alpha}};
    VkImageSubresourceRange range{VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    vkCmdClearColorImage(command, host->images[image_index], VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, &color, 1, &range);
    VkImageMemoryBarrier to_present{VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER};
    to_present.oldLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
    to_present.newLayout = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
    to_present.srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    to_present.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    to_present.image = host->images[image_index];
    to_present.subresourceRange = range;
    to_present.srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
    to_present.dstAccessMask = 0;
    vkCmdPipelineBarrier(command, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT, 0, 0, nullptr, 0, nullptr, 1, &to_present);
    recorded = ok(vkEndCommandBuffer(command), "end frame command buffer");
  }
  if (!recorded) { vkFreeCommandBuffers(host->device, host->command_pool, 1, &command); return false; }
  const VkPipelineStageFlags wait_stage = VK_PIPELINE_STAGE_TRANSFER_BIT;
  VkSubmitInfo submit{VK_STRUCTURE_TYPE_SUBMIT_INFO};
  submit.waitSemaphoreCount = 1;
  submit.pWaitSemaphores = &host->acquired[host->frame % host->acquired.size()];
  submit.pWaitDstStageMask = &wait_stage;
  submit.commandBufferCount = 1;
  submit.pCommandBuffers = &command;
  submit.signalSemaphoreCount = 1;
  submit.pSignalSemaphores = &host->rendered[host->frame % host->rendered.size()];
  const bool submitted = ok(vkQueueSubmit(host->graphics_queue, 1, &submit, VK_NULL_HANDLE), "submit clear frame");
  if (submitted) {
    VkPresentInfoKHR present{VK_STRUCTURE_TYPE_PRESENT_INFO_KHR};
    present.waitSemaphoreCount = 1;
    present.pWaitSemaphores = &host->rendered[host->frame % host->rendered.size()];
    present.swapchainCount = 1;
    present.pSwapchains = &host->swapchain;
    present.pImageIndices = &image_index;
    const VkResult result = vkQueuePresentKHR(host->present_queue, &present);
    host->image_layouts[image_index] = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
    ++host->frame;
    vkQueueWaitIdle(host->present_queue);
    vkQueueWaitIdle(host->graphics_queue);
    vkFreeCommandBuffers(host->device, host->command_pool, 1, &command);
    return result == VK_SUCCESS || result == VK_SUBOPTIMAL_KHR;
  }
  vkQueueWaitIdle(host->graphics_queue);
  vkFreeCommandBuffers(host->device, host->command_pool, 1, &command);
  return false;
}

void shutdown(Host* host) {
  if (!host) return;
  if (host->device) vkDeviceWaitIdle(host->device);
  if (host->command_pool && host->device) vkDestroyCommandPool(host->device, host->command_pool, nullptr);
  host->command_pool = VK_NULL_HANDLE;
  destroy_swapchain(host);
  if (host->device) vkDestroyDevice(host->device, nullptr);
  host->device = VK_NULL_HANDLE;
  host->graphics_queue = VK_NULL_HANDLE;
  host->present_queue = VK_NULL_HANDLE;
  if (host->surface && host->instance) vkDestroySurfaceKHR(host->instance, host->surface, nullptr);
  host->surface = VK_NULL_HANDLE;
  if (host->instance) vkDestroyInstance(host->instance, nullptr);
  host->instance = VK_NULL_HANDLE;
  host->physical = VK_NULL_HANDLE;
  host->ready = false;
}

}  // namespace mugoat::vkhost
