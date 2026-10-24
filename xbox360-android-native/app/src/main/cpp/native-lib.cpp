#include <jni.h>

extern "C"
JNIEXPORT jstring JNICALL
Java_org_example_xbox360android_MainActivity_nativeStatus(JNIEnv *env, jobject) {
    return env->NewStringUTF(
        "Native ARM64 app shell is running.\n\n"
        "Xbox 360 emulation core: not implemented yet.\n"
        "This prototype does not launch GTA V."
    );
}
