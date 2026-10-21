// sfp -- STAR FORGE patcher command-line tool (runs on the build machine / server host).
#include "engine.hpp"
#include "patchset.hpp"
#include "pe.hpp"
#include "sha1.hpp"
#include "sources.hpp"

#include <algorithm>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <set>
#include <sstream>

namespace fs = std::filesystem;
using namespace sfp;

namespace {

constexpr const char* kProxyDll = "libcrypto-1_1_tg.dll";
constexpr const char* kOrigDll = "libcrypto-1_1_tg_orig.dll";
constexpr const char* kMarkerExport = "sfp_version";

int fail(const std::string& msg) {
    std::cerr << "sfp: " << msg << "\n";
    return 1;
}

bool read_bin(const fs::path& p, Bytes& out) {
    std::ifstream f(p, std::ios::binary);
    if (!f) return false;
    out.assign(std::istreambuf_iterator<char>(f), {});
    return true;
}

std::string read_text(const fs::path& p) {
    std::ifstream f(p, std::ios::binary);
    std::ostringstream os;
    os << f.rdbuf();
    return os.str();
}

bool write_bin(const fs::path& p, const void* d, size_t n) {
    std::ofstream f(p, std::ios::binary | std::ios::trunc);
    f.write(static_cast<const char*>(d), std::streamsize(n));
    return bool(f);
}

bool load_ps(const fs::path& p, Patchset& ps, std::string& err) {
    if (!fs::exists(p)) { err = p.string() + ": not found"; return false; }
    return parse_patchset(read_text(p), p.string(), ps, err);
}

// A PE file held in memory, addressed by VA. Tracks writes into section slack so `bake` can
// grow VirtualSize the same way the hand-patched exes did.
class FileTarget : public Target {
public:
    Bytes data;
    pe::Info info;
    std::map<const pe::Section*, uint32_t> slack_end; // section -> max end RVA written

    bool load(const fs::path& p, std::string& err) {
        if (!read_bin(p, data)) { err = p.string() + ": cannot read"; return false; }
        return pe::parse(data.data(), data.size(), info, err);
    }
    bool read(uint32_t va, uint8_t* out, size_t n) override {
        size_t off;
        if (!locate(va, n, off)) return false;
        std::memcpy(out, data.data() + off, n);
        return true;
    }
    bool write(uint32_t va, const uint8_t* in, size_t n) override {
        size_t off;
        if (!locate(va, n, off)) return false;
        std::memcpy(data.data() + off, in, n);
        uint32_t rva = va - info.image_base;
        auto s = pe::section_for_file_range(info, rva, uint32_t(n));
        uint32_t end = rva + uint32_t(n);
        if (end > s->va + s->vsize) slack_end[s] = std::max(slack_end[s], end);
        return true;
    }
    // Grows VirtualSize of sections whose slack was written (rounded to FileAlignment).
    void grow_sections() {
        for (auto& [s, end] : slack_end) {
            uint32_t a = info.file_alignment ? info.file_alignment : 0x200;
            uint32_t vs = (end - s->va + a - 1) / a * a;
            std::memcpy(data.data() + s->hdr_off + 8, &vs, 4);
        }
    }

private:
    bool locate(uint32_t va, size_t n, size_t& off) {
        if (va < info.image_base) return false;
        uint32_t rva = va - info.image_base;
        auto s = pe::section_for_file_range(info, rva, uint32_t(n));
        if (!s) return false;
        off = size_t(rva - s->va) + s->raw_ptr;
        return off + n <= data.size();
    }
};

const char* state_name(State s) { return s == State::Pristine ? "pristine" : s == State::Applied ? "applied" : "MISMATCH"; }

// ---- commands -------------------------------------------------------------------------------

int cmd_build(const std::vector<std::string>& a) {
    if (a.size() != 2) return fail("usage: sfp build <patches-dir> <out-dir>");
    fs::path src(a[0]), out(a[1]);
    fs::create_directories(out);
    int n = 0;
    std::vector<fs::path> dirs;
    for (auto& e : fs::directory_iterator(src))
        if (e.is_directory() && fs::exists(e.path() / "target.toml")) dirs.push_back(e.path());
    std::sort(dirs.begin(), dirs.end());
    for (auto& d : dirs) {
        Patchset ps;
        std::vector<std::string> warn;
        std::string err;
        if (!load_sources(d.string(), ps, warn, err)) return fail(err);
        for (auto& w : warn) std::cerr << "warning: " << w << "\n";
        std::string text = format_patchset(ps);
        Patchset check;
        if (!parse_patchset(text, "generated", check, err)) return fail("internal: generated patchset does not parse: " + err);
        fs::path dst = out / (ps.exe + ".patchset.toml");
        if (!write_bin(dst, text.data(), text.size())) return fail("cannot write " + dst.string());
        size_t writes = 0;
        for (auto& p : ps.patches) writes += p.writes.size();
        std::cout << "built " << dst.string() << " (" << ps.patches.size() << " patches, " << writes << " writes)\n";
        n++;
    }
    if (!n) return fail("no <Exe>/target.toml found under " + src.string());
    return 0;
}

int cmd_verify(const std::vector<std::string>& a) {
    if (a.size() != 2) return fail("usage: sfp verify <X.patchset.toml> <X.exe>");
    Patchset ps;
    std::string err;
    if (!load_ps(a[0], ps, err)) return fail(err);
    FileTarget t;
    if (!t.load(a[1], err)) return fail(err);
    std::string sha = sha1_bytes(t.data.data(), t.data.size());
    bool pristine = sha == ps.sha1;
    std::cout << a[1] << ": sha1 " << sha << (pristine ? " (pristine, matches patchset)" : " (NOT the pristine image)") << "\n";
    int bad = 0;
    for (auto& p : ps.patches) {
        std::string why;
        State s = check(t, p, why);
        if (s == State::Mismatch) bad++;
        std::cout << "  " << state_name(s) << "  " << p.id << (p.enabled_by_default ? "" : " [off by default]")
                  << (why.empty() ? "" : "  -- " + why) << "\n";
    }
    return pristine && !bad ? 0 : 2;
}

int cmd_bake(const std::vector<std::string>& a) {
    std::vector<std::string> pos;
    std::set<std::string> enable, disable;
    std::string out, cfg_path;
    for (size_t i = 0; i < a.size(); i++) {
        if (a[i] == "-o" && i + 1 < a.size()) out = a[++i];
        else if (a[i] == "--enable" && i + 1 < a.size()) enable.insert(a[++i]);
        else if (a[i] == "--disable" && i + 1 < a.size()) disable.insert(a[++i]);
        else if (a[i] == "--config" && i + 1 < a.size()) cfg_path = a[++i];
        else pos.push_back(a[i]);
    }
    if (pos.size() != 2 || out.empty())
        return fail("usage: sfp bake <X.patchset.toml> <pristine X.exe> -o <out.exe> [--config patcher.toml] [--enable id] [--disable id]");
    Patchset ps;
    std::string err;
    if (!load_ps(pos[0], ps, err)) return fail(err);
    FileTarget t;
    if (!t.load(pos[1], err)) return fail(err);
    std::string sha = sha1_bytes(t.data.data(), t.data.size());
    if (sha != ps.sha1) return fail(pos[1] + " has sha1 " + sha + ", patchset expects " + ps.sha1);
    Config cfg;
    if (!cfg_path.empty() && !parse_config(read_text(cfg_path), cfg_path, cfg, err)) return fail(err);
    for (auto& id : enable) cfg.overrides[ps.exe][id] = true;
    for (auto& id : disable) cfg.overrides[ps.exe][id] = false;
    std::vector<std::string> warn;
    auto on = enabled_patches(ps, cfg, warn);
    for (auto& w : warn) std::cerr << "warning: " << w << "\n";
    auto ov = find_overlaps(on);
    if (!ov.empty()) return fail("enabled patches overlap: " + ov[0]);
    for (auto p : on) {
        std::string why;
        if (check(t, *p, why) != State::Pristine) return fail(p->id + ": " + why);
        if (!apply(t, *p, nullptr, err)) return fail(p->id + ": " + err);
        std::cout << "applied " << p->id << "\n";
    }
    t.grow_sections();
    if (!write_bin(out, t.data.data(), t.data.size())) return fail("cannot write " + out);
    std::cout << out << ": sha1 " << sha1_bytes(t.data.data(), t.data.size()) << "\n";
    return 0;
}

int cmd_gen_def(const std::vector<std::string>& a) {
    if (a.size() != 3) return fail("usage: sfp gen-def <original.dll> <forward-module-name> <out.def>");
    Bytes d;
    if (!read_bin(a[0], d)) return fail("cannot read " + a[0]);
    pe::Info info;
    std::string err;
    std::vector<pe::Export> ex;
    if (!pe::parse(d.data(), d.size(), info, err) || !pe::exports(d.data(), d.size(), info, ex, err)) return fail(err);
    std::sort(ex.begin(), ex.end(), [](auto& x, auto& y) { return x.ordinal < y.ordinal; });
    std::ofstream f(a[2], std::ios::trunc);
    f << "; Generated by `sfp gen-def` from " << fs::path(a[0]).filename().string() << " (sha1 "
      << sha1_bytes(d.data(), d.size()) << ")\n; Every export forwards to the renamed original.\n";
    f << "LIBRARY " << fs::path(a[0]).stem().string() << "\nEXPORTS\n";
    for (auto& e : ex) f << "  " << e.name << "=" << a[1] << "." << e.name << " @" << e.ordinal << "\n";
    std::cout << "wrote " << ex.size() << " forwarders to " << a[2] << "\n";
    return 0;
}

bool is_our_proxy(const fs::path& dll) {
    Bytes d;
    pe::Info info;
    std::string err;
    std::vector<pe::Export> ex;
    if (!read_bin(dll, d) || !pe::parse(d.data(), d.size(), info, err) || !pe::exports(d.data(), d.size(), info, ex, err))
        return false;
    return std::any_of(ex.begin(), ex.end(), [](auto& e) { return e.name == kMarkerExport; });
}

constexpr const char* kDefaultConfig = R"(# STAR FORGE patcher runtime config. Read by every server process at startup.
# What to do when a patchset doesn't match the exe or a patch fails to verify:
#   "exit"     -> the process refuses to start (recommended; you never run unpatched by accident)
#   "continue" -> log the problem and run with whatever patches did verify
on_error = "exit"

# Per-exe, per-patch overrides of each patch's `default`. Ids are listed by `sfp status`.
# [patches."DedicatedServer.exe"]
# "my-patch" = false
)";

int cmd_install(const std::vector<std::string>& a) {
    if (a.size() != 3) return fail("usage: sfp install <cloud-dir> <built proxy dll> <patchsets-dir>");
    fs::path cloud(a[0]), dll(a[1]), psdir(a[2]);
    fs::path proxy = cloud / kProxyDll, orig = cloud / kOrigDll;
    if (!fs::exists(proxy)) return fail(proxy.string() + " not found -- is this the game's cloud/ directory?");
    if (!is_our_proxy(dll)) return fail(dll.string() + " is not a STAR FORGE patcher proxy build");
    if (!is_our_proxy(proxy)) {
        if (fs::exists(orig)) return fail(orig.string() + " already exists but " + proxy.string() + " is not ours; sort this out by hand");
        fs::rename(proxy, orig);
        std::cout << "renamed original " << kProxyDll << " -> " << kOrigDll << "\n";
    } else if (!fs::exists(orig)) {
        return fail("proxy installed but " + orig.string() + " is missing");
    }
    fs::copy_file(dll, proxy, fs::copy_options::overwrite_existing);
    std::cout << "installed proxy " << proxy.string() << "\n";
    fs::path root = cloud / "starforge";
    fs::create_directories(root / "patchsets");
    fs::create_directories(root / "logs");
    for (auto& e : fs::directory_iterator(psdir)) {
        auto name = e.path().filename().string();
        const std::string suffix = ".patchset.toml";
        if (name.size() > suffix.size() && name.compare(name.size() - suffix.size(), suffix.size(), suffix) == 0) {
            fs::copy_file(e.path(), root / "patchsets" / name, fs::copy_options::overwrite_existing);
            std::cout << "installed patchset " << name << "\n";
        }
    }
    if (!fs::exists(root / "patcher.toml")) {
        write_bin(root / "patcher.toml", kDefaultConfig, std::strlen(kDefaultConfig));
        std::cout << "wrote default " << (root / "patcher.toml").string() << "\n";
    }
    return 0;
}

int cmd_uninstall(const std::vector<std::string>& a) {
    if (a.size() != 1) return fail("usage: sfp uninstall <cloud-dir>");
    fs::path cloud(a[0]), proxy = cloud / kProxyDll, orig = cloud / kOrigDll;
    if (!is_our_proxy(proxy)) return fail(proxy.string() + " is not the STAR FORGE patcher proxy; nothing to do");
    if (!fs::exists(orig)) return fail(orig.string() + " missing; cannot restore");
    fs::remove(proxy);
    fs::rename(orig, proxy);
    std::cout << "restored original " << kProxyDll << " (starforge/ directory left in place)\n";
    return 0;
}

int cmd_status(const std::vector<std::string>& a) {
    if (a.size() != 1) return fail("usage: sfp status <cloud-dir>");
    fs::path cloud(a[0]), root = cloud / "starforge";
    bool ours = is_our_proxy(cloud / kProxyDll);
    std::cout << "proxy dll: " << (ours ? "installed" : "NOT installed") << "\n";
    Config cfg;
    std::string err;
    if (fs::exists(root / "patcher.toml") && !parse_config(read_text(root / "patcher.toml"), "patcher.toml", cfg, err))
        return fail(err);
    if (!fs::exists(root / "patchsets")) return 0;
    for (auto& e : fs::directory_iterator(root / "patchsets")) {
        Patchset ps;
        if (!load_ps(e.path(), ps, err)) { std::cout << "  " << err << "\n"; continue; }
        FileTarget t;
        if (!t.load(cloud / ps.exe, err)) { std::cout << ps.exe << ": " << err << "\n"; continue; }
        std::string sha = sha1_bytes(t.data.data(), t.data.size());
        std::cout << ps.exe << ": " << (sha == ps.sha1 ? "pristine on disk (good)" : "sha1 " + sha + " != pristine " + ps.sha1 + " (process will refuse patching)") << "\n";
        std::vector<std::string> warn;
        auto on = enabled_patches(ps, cfg, warn);
        for (auto& w : warn) std::cout << "  warning: " << w << "\n";
        for (auto& p : ps.patches) {
            bool en = std::find(on.begin(), on.end(), &p) != on.end();
            std::cout << "  [" << (en ? "on " : "off") << "] " << p.id << (p.title.empty() ? "" : " -- " + p.title) << "\n";
        }
    }
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr << "usage: sfp <build|verify|bake|gen-def|install|uninstall|status> ...\n";
        return 1;
    }
    std::string cmd = argv[1];
    std::vector<std::string> a(argv + 2, argv + argc);
    try {
        if (cmd == "build") return cmd_build(a);
        if (cmd == "verify") return cmd_verify(a);
        if (cmd == "bake") return cmd_bake(a);
        if (cmd == "gen-def") return cmd_gen_def(a);
        if (cmd == "install") return cmd_install(a);
        if (cmd == "uninstall") return cmd_uninstall(a);
        if (cmd == "status") return cmd_status(a);
    } catch (const std::exception& e) {
        return fail(e.what());
    }
    return fail("unknown command " + cmd);
}
