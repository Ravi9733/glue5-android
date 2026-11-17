plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}
android {
    namespace = "com.ravi9733.rpfexplorer"
    compileSdk = 35
    compileOptions {\n        sourceCompatibility = JavaVersion.VERSION_17\n        targetCompatibility = JavaVersion.VERSION_17\n    }\n    kotlinOptions { jvmTarget = "17" }\n    defaultConfig {
        applicationId = "com.ravi9733.rpfexplorer"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }
}
