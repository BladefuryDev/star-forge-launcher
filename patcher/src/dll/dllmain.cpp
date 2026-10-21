// Proxy for the game's libcrypto-1_1_tg.dll: every export forwards to the renamed original
// (libcrypto-1_1_tg.def), and process attach runs the patch loader.
#include "version.hpp"

#include <windows.h>

namespace sfp {
bool run_loader();
}

// Marker export: `sfp install` uses it to tell this proxy from the original DLL.
extern "C" __declspec(dllexport) const char* __cdecl sfp_version() { return sfp::kVersion; }

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(module);
        try {
            return sfp::run_loader() ? TRUE : FALSE;
        } catch (...) {
            return FALSE;
        }
    }
    return TRUE;
}
