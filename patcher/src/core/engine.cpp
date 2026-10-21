#include "engine.hpp"

namespace sfp {

State check(Target& t, const Patch& p, std::string& why) {
    bool all_orig = true, all_new = true;
    for (auto& w : p.writes) {
        Bytes cur(w.orig.size());
        if (!t.read(w.va, cur.data(), cur.size())) {
            why = "cannot read " + hex32(w.va) + " (+" + std::to_string(cur.size()) + ")";
            return State::Mismatch;
        }
        bool is_orig = cur == w.orig;
        bool is_new = w.midhook.empty() ? cur == w.bytes : cur[0] == 0xe9;
        if (!is_orig && !is_new) {
            why = "bytes at " + hex32(w.va) + " are " + to_hex(cur) + ", expected " + to_hex(w.orig) +
                  (w.midhook.empty() ? " (or patched " + to_hex(w.bytes) + ")" : "");
            return State::Mismatch;
        }
        all_orig &= is_orig;
        all_new &= is_new;
    }
    if (all_orig) return State::Pristine;
    if (all_new) return State::Applied;
    why = "partially applied (some writes original, some patched)";
    return State::Mismatch;
}

bool apply(Target& t, const Patch& p, const MidhookInstaller& midhook, std::string& err) {
    for (auto& w : p.writes) {
        if (!w.midhook.empty()) {
            if (!midhook) { err = "midhook '" + w.midhook + "' needs the runtime DLL (cannot bake)"; return false; }
            if (!midhook(w, err)) return false;
            continue;
        }
        if (!t.write(w.va, w.bytes.data(), w.bytes.size())) { err = "write failed at " + hex32(w.va); return false; }
        Bytes back(w.bytes.size());
        if (!t.read(w.va, back.data(), back.size()) || back != w.bytes) { err = "readback mismatch at " + hex32(w.va); return false; }
    }
    return true;
}

} // namespace sfp
