#include <jni.h>
#include <exception>
#include <string>
#include "asset_converter.h"
extern "C" JNIEXPORT jstring JNICALL
Java_com_ravi9733_autorpconverter_NativeAssets_convertDirectory(JNIEnv* env, jclass, jstring root_path) {
    const char* chars = env->GetStringUTFChars(root_path, nullptr);
    if (chars == nullptr) return nullptr;
    std::string report;
    try {
        report = gta_asset_converter::convert_directory(chars);
    } catch (const std::exception& error) {
        report = std::string("Asset conversion failed: ") + error.what();
    } catch (...) {
        report = "Asset conversion failed: unknown native error";
    }
    env->ReleaseStringUTFChars(root_path, chars);
    return env->NewStringUTF(report.c_str());
}
