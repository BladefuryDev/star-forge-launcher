// Runs inside every server process from DllMain (DLL_PROCESS_ATTACH), before the exe's own
// CRT startup. Only code patches belong here; anything needing a running game goes in hooks.
#include "engine.hpp"
#include "log.hpp"
#include "midhook.hpp"
#include "patchset.hpp"
#include "pe.hpp"
#include "sha1.hpp"
#include "version.hpp"

#include <windows.h>

#include <cstring>

namespace sfp {

namespace {

bool read_file(const std::wstring& path, std::string& out) {
    HANDLE h = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, 0, nullptr);
    if (h == INVALID_HANDLE_VALUE) return false;
    LARGE_INTEGER sz;
    GetFileSizeEx(h, &sz);
    out.resize(size_t(sz.QuadPart));
    DWORD got = 0;
    bool ok = ReadFile(h, out.data(), DWORD(out.size()), &got, nullptr) && got == out.size();
    CloseHandle(h);
    return ok;
}

bool exists(const std::wstring& p) { return GetFileAttributesW(p.c_str()) != INVALID_FILE_ATTRIBUTES; }

class MemTarget : public Target {
public:
    MemTarget(uint8_t* base, uint32_t image_base, uint32_t size) : base_(base), image_base_(image_base), size_(size) {}
    bool read(uint32_t va, uint8_t* out, size_t n) override {
        uint8_t* p = at(va, n);
        if (!p) return false;
        std::memcpy(out, p, n);
        return true;
    }
    bool write(uint32_t va, const uint8_t* in, size_t n) override {
        uint8_t* p = at(va, n);
        return p && write_code(p, in, n);
    }

private:
    uint8_t* at(uint32_t va, size_t n) {
        if (va < image_base_ || uint64_t(va - image_base_) + n > size_) return nullptr;
        return base_ + (va - image_base_);
    }
    uint8_t* base_;
    uint32_t image_base_, size_;
};

} // namespace

// Returns false when the process must not start (fail-closed).
bool run_loader() {
    wchar_t buf[MAX_PATH * 2];
    DWORD n = GetModuleFileNameW(nullptr, buf, DWORD(std::size(buf)));
    std::wstring exe_path(buf, n);
    size_t slash = exe_path.find_last_of(L"\\/");
    std::wstring dir = exe_path.substr(0, slash + 1), exe = exe_path.substr(slash + 1);
    std::wstring root = dir + L"starforge\\";
    if (!exists(root)) return true; // not configured: behave as the plain original DLL

    CreateDirectoryW((root + L"logs").c_str(), nullptr);
    std::wstring stem = exe.substr(0, exe.find_last_of(L'.'));
    log_open(root + L"logs\\" + stem + L".log");
    log("star-forge patcher %s in %s", kVersion, narrow(exe).c_str());
    log("cmdline: %s", narrow(GetCommandLineW()).c_str());

    Config cfg;
    std::string err, text;
    std::wstring cfg_path = root + L"patcher.toml";
    if (read_file(cfg_path, text) && !parse_config(text, "patcher.toml", cfg, err)) {
        log("FATAL: %s", err.c_str());
        return false;
    }
    auto fatal = [&](const std::string& msg) {
        log("%s: %s", cfg.exit_on_error ? "FATAL" : "ERROR", msg.c_str());
        if (cfg.exit_on_error) log("refusing to start (on_error = \"exit\" in patcher.toml)");
        return !cfg.exit_on_error;
    };

    std::wstring ps_path = root + L"patchsets\\" + exe + L".patchset.toml";
    if (!read_file(ps_path, text)) {
        log("no patchset for %s; running unpatched", narrow(exe).c_str());
        return true;
    }
    Patchset ps;
    if (!parse_patchset(text, narrow(exe) + ".patchset.toml", ps, err)) return fatal(err);

    std::string image;
    if (!read_file(exe_path, image)) return fatal("cannot read own exe for hashing");
    std::string sha = sha1_bytes(image.data(), image.size());
    if (sha != ps.sha1) return fatal("exe sha1 " + sha + " is not the pristine image " + ps.sha1 + " this patchset targets");

    auto base = reinterpret_cast<uint8_t*>(GetModuleHandleW(nullptr));
    pe::Info info;
    if (!pe::parse(base, 0x1000, info, err)) return fatal("in-memory PE header: " + err);
    if (uint32_t(uintptr_t(base)) != ps.image_base) return fatal("exe loaded at " + hex32(uint32_t(uintptr_t(base))) + ", expected " + hex32(ps.image_base));

    std::vector<std::string> warnings;
    auto on = enabled_patches(ps, cfg, warnings);
    for (auto& w : warnings) log("warning: %s", w.c_str());
    auto ov = find_overlaps(on);
    if (!ov.empty()) return fatal("enabled patches overlap: " + ov[0]);

    MemTarget mem(base, ps.image_base, info.size_of_image);
    // Verify everything before writing anything, so a bad patchset never half-applies.
    std::vector<const Patch*> go;
    for (auto p : on) {
        std::string why;
        State s = check(mem, *p, why);
        if (s == State::Pristine) why.clear();
        else why = s == State::Applied ? "already applied in memory" : why;
        for (auto& wr : p->writes)
            if (why.empty() && !wr.midhook.empty() && !find_midhook(wr.midhook.c_str()))
                why = "midhook '" + wr.midhook + "' is not compiled into this DLL";
        if (why.empty()) { go.push_back(p); continue; }
        std::string msg = p->id + ": " + why;
        if (p->optional) { log("skipping optional patch %s", msg.c_str()); continue; }
        if (!fatal(msg)) return false;
    }

    MidhookInstaller mh = [](const Write& w, std::string& e) {
        return install_midhook(w.va, w.orig.data(), w.orig.size(), find_midhook(w.midhook.c_str()), e);
    };
    int applied = 0;
    for (auto p : go) {
        if (!apply(mem, *p, mh, err)) return fatal(p->id + ": " + err);
        log("applied %s (%zu writes)", p->id.c_str(), p->writes.size());
        applied++;
    }
    log("%d/%zu patches applied (%zu in patchset)", applied, on.size(), ps.patches.size());
    return true;
}

} // namespace sfp
