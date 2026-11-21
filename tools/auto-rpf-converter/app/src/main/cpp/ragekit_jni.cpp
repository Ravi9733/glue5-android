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
Java_com_ravi9733_autorpconverter_NativeAssets_buildOpenRpf7(
    JNIEnv* env, jclass, jstring source_directory, jstring output_path) {
    const char* source_chars = env->GetStringUTFChars(source_directory, nullptr);
    if (source_chars == nullptr) return nullptr;
    const char* output_chars = env->GetStringUTFChars(output_path, nullptr);
    if (output_chars == nullptr) {
        env->ReleaseStringUTFChars(source_directory, source_chars);
        return nullptr;
    }
    std::string report;
    try {
        report = gta_asset_converter::build_open_rpf7(source_chars, output_chars);
    } catch (const std::exception& error) {
        report = std::string("RPF7 build failed: ") + error.what();
    } catch (...) {
        report = "RPF7 build failed: unknown native error";
    }
    env->ReleaseStringUTFChars(source_directory, source_chars);
    env->ReleaseStringUTFChars(output_path, output_chars);
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


extern "C" JNIEXPORT jstring JNICALL
Java_com_ravi9733_autorpconverter_NativeAssets_convertArchiveToOpenRpf7(
    JNIEnv* env, jclass, jstring source_path, jstring output_path, jstring key_directory) {
    const char* source_chars = env->GetStringUTFChars(source_path, nullptr);
    if (source_chars == nullptr) return nullptr;
    const char* output_chars = env->GetStringUTFChars(output_path, nullptr);
    if (output_chars == nullptr) {
        env->ReleaseStringUTFChars(source_path, source_chars);
        return nullptr;
    }
    const char* key_chars = env->GetStringUTFChars(key_directory, nullptr);
    if (key_chars == nullptr) {
        env->ReleaseStringUTFChars(source_path, source_chars);
        env->ReleaseStringUTFChars(output_path, output_chars);
        return nullptr;
    }

    std::string report;
    try {
        report = gta_asset_converter::convert_rpf_archive_to_open_rpf7(
            source_chars, output_chars, key_chars);
    } catch (const std::exception& error) {
        report = std::string("RPF7 conversion failed: ") + error.what();
    } catch (...) {
        report = "RPF7 conversion failed: unknown native error";
    }
    env->ReleaseStringUTFChars(source_path, source_chars);
    env->ReleaseStringUTFChars(output_path, output_chars);
    env->ReleaseStringUTFChars(key_directory, key_chars);
    return env->NewStringUTF(report.c_str());
}


extern "C" JNIEXPORT jstring JNICALL
Java_com_ravi9733_autorpconverter_NativeAssets_inspectDlcPackMetadata(
    JNIEnv* env, jclass, jstring source_path, jstring key_directory) {
    const char* source_chars = env->GetStringUTFChars(source_path, nullptr);
    if (source_chars == nullptr) return nullptr;
    const char* key_chars = env->GetStringUTFChars(key_directory, nullptr);
    if (key_chars == nullptr) {
        env->ReleaseStringUTFChars(source_path, source_chars);
        return nullptr;
    }

    std::string report;
    try {
        report = gta_asset_converter::inspect_dlc_pack_metadata(source_chars, key_chars);
    } catch (const std::exception& error) {
        report = std::string("DLC metadata validation failed: ") + error.what();
    } catch (...) {
        report = "DLC metadata validation failed: unknown native error";
    }
    env->ReleaseStringUTFChars(source_path, source_chars);
    env->ReleaseStringUTFChars(key_directory, key_chars);
    return env->NewStringUTF(report.c_str());
}


extern "C" JNIEXPORT jstring JNICALL
Java_com_ravi9733_autorpconverter_NativeAssets_readDlcListEntries(
    JNIEnv* env, jclass, jstring update_rpf_path, jstring key_directory) {
    const char* update_chars = env->GetStringUTFChars(update_rpf_path, nullptr);
    if (update_chars == nullptr) return nullptr;
    const char* key_chars = env->GetStringUTFChars(key_directory, nullptr);
    if (key_chars == nullptr) {
        env->ReleaseStringUTFChars(update_rpf_path, update_chars);
        return nullptr;
    }

    std::string report;
    try {
        report = gta_asset_converter::read_dlc_list_entries(update_chars, key_chars);
    } catch (const std::exception& error) {
        report = std::string("DLC list read failed: ") + error.what();
    } catch (...) {
        report = "DLC list read failed: unknown native error";
    }
    env->ReleaseStringUTFChars(update_rpf_path, update_chars);
    env->ReleaseStringUTFChars(key_directory, key_chars);
    return env->NewStringUTF(report.c_str());
}


extern "C" JNIEXPORT jstring JNICALL
Java_com_ravi9733_autorpconverter_NativeAssets_inspectPlatformDlcPackMetadata(
    JNIEnv* env, jclass, jstring base_archive_path, jstring pack_name, jstring key_directory) {
    const char* base_chars = env->GetStringUTFChars(base_archive_path, nullptr);
    if (base_chars == nullptr) return nullptr;
    const char* pack_chars = env->GetStringUTFChars(pack_name, nullptr);
    if (pack_chars == nullptr) {
        env->ReleaseStringUTFChars(base_archive_path, base_chars);
        return nullptr;
    }
    const char* key_chars = env->GetStringUTFChars(key_directory, nullptr);
    if (key_chars == nullptr) {
        env->ReleaseStringUTFChars(base_archive_path, base_chars);
        env->ReleaseStringUTFChars(pack_name, pack_chars);
        return nullptr;
    }

    std::string report;
    try {
        report = gta_asset_converter::inspect_platform_dlc_pack_metadata(
            base_chars, pack_chars, key_chars);
    } catch (const std::exception& error) {
        report = std::string("Platform DLC scan failed: ") + error.what();
    } catch (...) {
        report = "Platform DLC scan failed: unknown native error";
    }
    env->ReleaseStringUTFChars(base_archive_path, base_chars);
    env->ReleaseStringUTFChars(pack_name, pack_chars);
    env->ReleaseStringUTFChars(key_directory, key_chars);
    return env->NewStringUTF(report.c_str());
}
