cmake_minimum_required(VERSION 3.22)
project(mugoat_vulkan_test LANGUAGES C CXX)

if(NOT ANDROID)
  message(FATAL_ERROR "This target must be built with the Android NDK")
endif()

set(NATIVE_APP_GLUE "${CMAKE_ANDROID_NDK}/sources/android/native_app_glue")
if(NOT EXISTS "${NATIVE_APP_GLUE}/android_native_app_glue.c")
  message(FATAL_ERROR "Android native_app_glue not found at ${NATIVE_APP_GLUE}")
endif()

add_library(android_native_app_glue STATIC "${NATIVE_APP_GLUE}/android_native_app_glue.c")
target_include_directories(android_native_app_glue PUBLIC "${NATIVE_APP_GLUE}")

add_library(mugoat_vulkan_test SHARED
  ../vulkan/VulkanHost.cpp
  ../vulkan/CommandRingBridge.cpp
  VulkanTestMain.cpp
)
target_include_directories(mugoat_vulkan_test PRIVATE "${NATIVE_APP_GLUE}" ../vulkan)
target_compile_features(mugoat_vulkan_test PRIVATE cxx_std_20)
target_compile_options(mugoat_vulkan_test PRIVATE -Wall -Wextra -Werror=return-type)
target_link_libraries(mugoat_vulkan_test PRIVATE android_native_app_glue android log vulkan)
