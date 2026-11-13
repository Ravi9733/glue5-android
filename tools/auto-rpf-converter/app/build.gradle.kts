plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.ravi9733.autorpconverter"
    compileSdk = 35

    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "com.ravi9733.autorpconverter"
        minSdk = 26
        targetSdk = 35
        versionCode = 2
        versionName = "0.2.0"
        ndk { abiFilters += setOf("arm64-v8a") }
        externalNativeBuild {
            cmake {
                cppFlags += "-std=c++20"
                arguments += "-DANDROID_STL=c++_shared"
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }
}

dependencies {
    implementation("androidx.documentfile:documentfile:1.0.1")
}
