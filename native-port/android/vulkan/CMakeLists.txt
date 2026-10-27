cmake_minimum_required(VERSION 3.22)
project(mugoat_vulkan_host LANGUAGES C CXX)

if(NOT ANDROID)
  message(FATAL_ERROR "Mugoat Vulkan host must be built with the Android NDK")
endif()

add_library(mugoat_vulkan_host STATIC VulkanHost.cpp)
target_compile_features(mugoat_vulkan_host PUBLIC cxx_std_20)
target_include_directories(mugoat_vulkan_host PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(mugoat_vulkan_host PUBLIC android log vulkan)
target_compile_options(mugoat_vulkan_host PRIVATE -Wall -Wextra -Werror=return-type)
