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

extern "C" JNIEXPORT jstring JNICALL
Java_com_ravi9733_autorpconverter_NativeAssets_inspectRpfKeys(JNIEnv* env, jclass, jstring key_directory) {
    const char* key_chars = env->GetStringUTFChars(key_directory, nullptr);
    if (key_chars == nullptr) return nullptr;
    std::string report;
    try {
        report = gta_asset_converter::inspect_rpf_keys(key_chars);
    } catch (const std::exception& error) {
        report = std::string("RPF key inspection failed: ") + error.what();
    } catch (...) {
        report = "RPF key inspection failed: unknown native error";
    }
    env->ReleaseStringUTFChars(key_directory, key_chars);
    return env->NewStringUTF(report.c_str());
}

extern "C" JNIEXPORT jstring JNICALL
Java_com_ravi9733_autorpconverter_NativeAssets_extractArchiveWithKeys(
    JNIEnv* env, jclass, jstring source_path, jstring destination_path, jstring key_directory) {
    const char* source_chars = env->GetStringUTFChars(source_path, nullptr);
    if (source_chars == nullptr) return nullptr;
    const char* destination_chars = env->GetStringUTFChars(destination_path, nullptr);
    if (destination_chars == nullptr) {
        env->ReleaseStringUTFChars(source_path, source_chars);
        return nullptr;
    }
    const char* key_chars = env->GetStringUTFChars(key_directory, nullptr);
    if (key_chars == nullptr) {
        env->ReleaseStringUTFChars(source_path, source_chars);
        env->ReleaseStringUTFChars(destination_path, destination_chars);
        return nullptr;
    }

    std::string report;
    try {
        report = gta_asset_converter::extract_rpf_archive(source_chars, destination_chars, key_chars);
    } catch (const std::exception& error) {
        report = std::string("RPF extraction failed: ") + error.what();
    } catch (...) {
        report = "RPF extraction failed: unknown native error";
    }
    env->ReleaseStringUTFChars(source_path, source_chars);
    env->ReleaseStringUTFChars(destination_path, destination_chars);
    env->ReleaseStringUTFChars(key_directory, key_chars);
    return env->NewStringUTF(report.c_str());
}
