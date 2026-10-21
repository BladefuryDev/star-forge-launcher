#include "sources.hpp"
#include "toml.hpp"

#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <map>
#include <sstream>
#include <variant>

#include <unistd.h>

namespace fs = std::filesystem;

namespace sfp {

namespace {

std::string read_file(const fs::path& p) {
    std::ifstream f(p, std::ios::binary);
    std::ostringstream os;
    os << f.rdbuf();
    return os.str();
}

std::string shq(const std::string& s) {
    std::string r = "'";
    for (char c : s) r += c == '\'' ? std::string("'\\''") : std::string(1, c);
    return r + "'";
}

// Assembles a GNU-as (intel syntax allowed) source linked at `va`, returning the .text bytes.
bool assemble(const fs::path& src, uint32_t va, Bytes& out, std::string& err) {
    fs::path tmp = fs::temp_directory_path() / ("sfp-asm-" + std::to_string(::getpid()) + "-" + std::to_string(va));
    fs::create_directories(tmp);
    std::string o = (tmp / "a.o").string(), elf = (tmp / "a.elf").string(), bin = (tmp / "a.bin").string();
    std::string log = (tmp / "log").string();
    std::string cmd = "as --32 -o " + shq(o) + " " + shq(src.string()) + " >" + shq(log) + " 2>&1 && " +
                      "ld -m elf_i386 -e _start -Ttext=" + hex32(va) + " -o " + shq(elf) + " " + shq(o) + " >>" + shq(log) +
                      " 2>&1 && objcopy -O binary -j .text " + shq(elf) + " " + shq(bin) + " >>" + shq(log) + " 2>&1";
    int rc = std::system(cmd.c_str());
    std::string data = read_file(bin);
    if (rc != 0 || data.empty()) {
        err = "assembling " + src.string() + " failed:\n" + read_file(log);
        fs::remove_all(tmp);
        return false;
    }
    fs::remove_all(tmp);
    out.assign(data.begin(), data.end());
    return true;
}

// x86 opcodes whose encoding depends on where they run (so they can't be copied into a thunk).
bool starts_position_dependent(const Bytes& b) {
    uint8_t o = b[0];
    return o == 0xe8 || o == 0xe9 || o == 0xeb || (o >= 0x70 && o <= 0x7f) || (o >= 0xe0 && o <= 0xe3) ||
           (o == 0x0f && b.size() > 1 && b[1] >= 0x80 && b[1] <= 0x8f);
}

struct Pending {
    Write w;
    std::variant<std::monostate, uint32_t, std::string> to; // jmp/call target
    uint8_t opcode = 0;                                     // 0xe9 jmp, 0xe8 call
};

bool load_patch_file(const fs::path& file, const fs::path& dir, Patch& p, std::string& err) {
    toml::table t;
    try {
        t = toml::parse_file(file.string());
    } catch (const toml::parse_error& e) {
        std::ostringstream os;
        os << e;
        err = os.str();
        return false;
    }
    p.id = t["id"].value_or(file.stem().string());
    p.title = t["title"].value_or(std::string{});
    p.description = t["description"].value_or(std::string{});
    p.enabled_by_default = t["default"].value_or(true);
    p.optional = t["optional"].value_or(false);
    std::string where = file.filename().string();

    std::map<std::string, uint32_t> labels;
    std::vector<Pending> pend;
    auto ws = t["write"].as_array();
    if (!ws || ws->empty()) { err = where + ": no [[write]] entries"; return false; }
    for (auto& we : *ws) {
        auto wt = we.as_table();
        if (!wt) { err = where + ": [[write]] must be a table"; return false; }
        Pending pd;
        Write& w = pd.w;
        auto va = (*wt)["va"].value<int64_t>();
        if (!va) { err = where + ": write without va"; return false; }
        w.va = uint32_t(*va);
        w.comment = (*wt)["comment"].value_or(std::string{});
        std::string at = where + " @" + hex32(w.va);
        if (auto s = (*wt)["orig"].value<std::string>(); s && !parse_hex(*s, w.orig)) { err = at + ": bad orig hex"; return false; }

        int kinds = 0;
        if (auto s = (*wt)["bytes"].value<std::string>()) {
            kinds++;
            if (!parse_hex(*s, w.bytes)) { err = at + ": bad bytes hex"; return false; }
            if (w.orig.size() != w.bytes.size()) { err = at + ": orig and bytes differ in length"; return false; }
        }
        if (auto s = (*wt)["asm"].value<std::string>()) {
            kinds++;
            if (!assemble(dir / *s, w.va, w.bytes, err)) return false;
            if (w.orig.empty()) w.orig.assign(w.bytes.size(), 0); // caves go into zero slack by default
            if (w.orig.size() != w.bytes.size()) {
                err = at + ": " + *s + " assembled to " + std::to_string(w.bytes.size()) + " bytes but orig has " +
                      std::to_string(w.orig.size());
                return false;
            }
            if (w.comment.empty()) w.comment = "asm: " + *s;
        }
        for (auto [key, op] : {std::pair{"jmp", uint8_t(0xe9)}, std::pair{"call", uint8_t(0xe8)}}) {
            auto node = (*wt)[key];
            if (!node) continue;
            kinds++;
            pd.opcode = op;
            if (auto i = node.value<int64_t>()) pd.to = uint32_t(*i);
            else if (auto s = node.value<std::string>()) pd.to = *s;
            else { err = at + ": " + key + " must be an address or a label"; return false; }
            if (w.orig.size() < 5) { err = at + ": " + key + " needs at least 5 orig bytes"; return false; }
        }
        if (auto s = (*wt)["midhook"].value<std::string>()) {
            kinds++;
            w.midhook = *s;
            if (w.orig.size() < 5) { err = at + ": midhook needs at least 5 orig (stolen) bytes"; return false; }
            if (starts_position_dependent(w.orig)) {
                err = at + ": stolen bytes start with a relative jump/call and can't be relocated into a thunk";
                return false;
            }
        }
        if (kinds != 1) { err = at + ": exactly one of bytes / asm / jmp / call / midhook is required"; return false; }
        if (w.orig.empty()) { err = at + ": orig is required"; return false; }
        if (auto l = (*wt)["label"].value<std::string>()) labels[*l] = w.va;
        pend.push_back(std::move(pd));
    }
    for (auto& pd : pend) {
        if (pd.opcode) {
            uint32_t to;
            if (auto* s = std::get_if<std::string>(&pd.to)) {
                auto it = labels.find(*s);
                if (it == labels.end()) { err = where + ": unknown label '" + *s + "'"; return false; }
                to = it->second;
            } else to = std::get<uint32_t>(pd.to);
            Write& w = pd.w;
            w.bytes.assign(w.orig.size(), 0x90);
            w.bytes[0] = pd.opcode;
            uint32_t rel = to - (w.va + 5);
            for (int i = 0; i < 4; i++) w.bytes[1 + i] = uint8_t(rel >> (8 * i));
            if (w.comment.empty()) w.comment = std::string(pd.opcode == 0xe9 ? "jmp " : "call ") + hex32(to);
        }
        p.writes.push_back(std::move(pd.w));
    }
    return true;
}

} // namespace

bool load_sources(const std::string& dir_s, Patchset& out, std::vector<std::string>& warnings, std::string& err) {
    fs::path dir(dir_s);
    toml::table t;
    try {
        t = toml::parse_file((dir / "target.toml").string());
    } catch (const toml::parse_error& e) {
        std::ostringstream os;
        os << e;
        err = os.str();
        return false;
    }
    out = Patchset{};
    out.exe = t["exe"].value_or(dir.filename().string());
    out.sha1 = t["sha1"].value_or(std::string{});
    out.build = t["build"].value_or(std::string{});
    out.image_base = uint32_t(t["image_base"].value_or(int64_t(0x400000)));
    if (out.sha1.size() != 40) { err = (dir / "target.toml").string() + ": sha1 must be 40 hex chars"; return false; }

    std::vector<fs::path> files;
    for (auto& e : fs::directory_iterator(dir))
        if (e.path().extension() == ".toml" && e.path().filename() != "target.toml") files.push_back(e.path());
    std::sort(files.begin(), files.end());
    for (auto& f : files) {
        Patch p;
        if (!load_patch_file(f, dir, p, err)) return false;
        for (auto& q : out.patches)
            if (q.id == p.id) { err = f.string() + ": duplicate patch id '" + p.id + "'"; return false; }
        out.patches.push_back(std::move(p));
    }

    std::vector<const Patch*> on, all;
    for (auto& p : out.patches) {
        all.push_back(&p);
        if (p.enabled_by_default) on.push_back(&p);
    }
    auto hard = find_overlaps(on);
    if (!hard.empty()) {
        err = out.exe + ": default-enabled patches overlap:";
        for (auto& e : hard) err += "\n  " + e;
        return false;
    }
    for (auto& e : find_overlaps(all)) warnings.push_back(out.exe + ": mutually exclusive (don't enable both): " + e);
    return true;
}

} // namespace sfp
