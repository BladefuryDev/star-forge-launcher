// Exercises the midhook thunk under wine/Windows: continue path, stack-arg edits, register
// edits and the redirect path. Prints PASS/FAIL, exit code = failures.
#include "midhook.hpp"

#include <windows.h>

#include <cstdio>

// victim(x) = x + 1, with a known 7-byte relocatable prologue to steal.
extern "C" __declspec(naked) int __cdecl victim(int) {
    __asm {
        mov eax, [esp+4]   // 8b 44 24 04
        add eax, 1         // 83 c0 01
        ret
    }
}

// Redirect target: entered with the hook-site stack (return address on top).
extern "C" __declspec(naked) int __cdecl alt_ret() {
    __asm {
        mov eax, 999
        ret
    }
}

// Calls victim(x) with ecx = 7 and stores the ecx seen after the call in *ecx_out.
extern "C" __declspec(naked) int __cdecl call_with_regs(int, int*) {
    __asm {
        mov ecx, 7
        push dword ptr [esp+4]   // x
        call victim
        add esp, 4
        mov edx, [esp+8]
        mov [edx], ecx
        ret
    }
}

static int g_calls = 0;

SFP_MIDHOOK(test_hook) {
    g_calls++;
    uint32_t* s = r->stack(); // s[0] = return address, s[1] = x
    if (s[1] == 42) return uint32_t(uintptr_t(&alt_ret));
    s[1] *= 10;
    r->ecx = 1234;
    return 0;
}

int main() {
    int fails = 0;
    auto expect = [&](const char* what, int got, int want) {
        std::printf("%s %s: got %d want %d\n", got == want ? "PASS" : "FAIL", what, got, want);
        if (got != want) fails++;
    };
    expect("unhooked victim(5)", victim(5), 6);

    std::string err;
    auto fn = sfp::find_midhook("test_hook");
    if (!fn || !sfp::install_midhook(uint32_t(uintptr_t(&victim)), reinterpret_cast<const uint8_t*>("\x8b\x44\x24\x04\x83\xc0\x01"), 7, fn, err)) {
        std::printf("FAIL install: %s\n", err.c_str());
        return 1;
    }
    expect("continue path: victim(5) with arg*10", victim(5), 51);
    expect("redirect path: victim(42)", victim(42), 999);
    int ecx = 0;
    expect("call_with_regs(3)", call_with_regs(3, &ecx), 31);
    expect("register edit survives (ecx)", ecx, 1234);
    expect("hook calls", g_calls, 3);
    std::printf("%s (%d failures)\n", fails ? "FAILED" : "ALL PASS", fails);
    return fails;
}
