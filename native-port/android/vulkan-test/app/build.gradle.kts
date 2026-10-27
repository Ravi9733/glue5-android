plugins {
    id("com.android.application")
}

android {
    namespace = "local.mugoat.vulkantest"
    compileSdk = 35

    defaultConfig {
        applicationId = "local.mugoat.vulkantest"
        minSdk = 29
        targetSdk = 28
        versionCode = 1
        versionName = "0.1-vulkan-smoke-test"

        externalNativeBuild {
            cmake {
                cppFlags += listOf("-std=c++20", "-Wall", "-Wextra")
                arguments += listOf("-DANDROID_STL=c++_static")
            }
        }

        ndk { abiFilters += listOf("arm64-v8a") }
    }

    externalNativeBuild {
        cmake {
            path = file("../../vulkan-test-native/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    buildTypes {
        debug { isDebuggable = true }
        release {
            isMinifyEnabled = false
        }
    }
}
