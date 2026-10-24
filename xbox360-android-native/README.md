# Xbox 360 Android Prototype

An experimental Android app shell for researching Xbox 360 game compatibility.

## Current status
- Android app shell and JNI/native C++ entry point.
- GitHub Actions debug-APK build workflow.
- **This is not an Xbox 360 emulator yet and does not run GTA V.** The CPU, GPU, audio, storage, and Xbox 360 system compatibility layers still need substantial implementation and testing.

## Build
Pushes to `xbox360-android-native-prototype` trigger the workflow at `.github/workflows/xbox360-android-prototype.yml`. A successful run uploads a debug APK artifact.

## Game files
This project does not include GTA V or Xbox 360 firmware. Use only game files and firmware you are legally entitled to use. Do not upload copyrighted game data to this repository.

## Planned milestones
1. Build and launch the Android shell.
2. Research an appropriately licensed Xbox 360 CPU/system emulation foundation.
3. Implement graphics and audio backend integration.
4. Add game-file selection and compatibility diagnostics.
5. Test individual subsystems before attempting GTA V.
