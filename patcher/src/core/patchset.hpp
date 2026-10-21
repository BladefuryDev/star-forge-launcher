// Resolved patchset: what the DLL loads and `sfp bake` applies. Produced by `sfp build`
// from the human-edited sources in a patch project's <Exe>/ (see docs/FORMAT.md).
#pragma once
#include "util.hpp"

#include <cstdint>
#include <map>
#include <string>
#include <vector>

namespace sfp {

struct Write {
    uint32_t va = 0;
    Bytes orig;          // bytes that must be present before patching
    Bytes bytes;         // replacement, same length as orig (empty for midhooks)
    std::string midhook; // name of a C++ mid-function hook compiled into the DLL
    std::string comment;
};

struct Patch {
    std::string id;
    std::string title;
    std::string description;
    bool enabled_by_default = true;
    bool optional = false; // a verification failure skips this patch instead of failing the process
    std::vector<Write> writes;
};

struct Patchset {
    static constexpr int kFormat = 1;
    std::string exe;   // e.g. "DedicatedServer.exe"
    std::string sha1;  // pristine image this patchset targets
    std::string build; // informational
    uint32_t image_base = 0;
    std::vector<Patch> patches;
};

// Text-based so callers decide how files are opened (the DLL uses wide-char Win32 paths).
bool parse_patchset(const std::string& text, const std::string& source_name, Patchset& out, std::string& err);
std::string format_patchset(const Patchset& ps);

// Runtime configuration (starforge/patcher.toml next to the exes).
struct Config {
    bool exit_on_error = true;             // on_error = "exit" | "continue"
    std::map<std::string, std::map<std::string, bool>> overrides; // [patches."Exe.exe"] id = true/false
};
bool parse_config(const std::string& text, const std::string& source_name, Config& out, std::string& err);

// Patches that are on after config overrides. Unknown override ids are reported in `warnings`.
std::vector<const Patch*> enabled_patches(const Patchset& ps, const Config& cfg, std::vector<std::string>& warnings);

// One error per pair of writes that touch the same bytes.
std::vector<std::string> find_overlaps(const std::vector<const Patch*>& patches);

} // namespace sfp
