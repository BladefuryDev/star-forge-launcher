// Verifies and applies patches against an abstract image (a file on disk or the live process).
#pragma once
#include "patchset.hpp"

#include <functional>
#include <string>
#include <vector>

namespace sfp {

class Target {
public:
    virtual ~Target() = default;
    virtual bool read(uint32_t va, uint8_t* out, size_t n) = 0;
    virtual bool write(uint32_t va, const uint8_t* in, size_t n) = 0;
};

enum class State { Pristine, Applied, Mismatch };

// Pristine = every write's orig bytes are present; Applied = every write's replacement is
// present (midhook writes count as applied when the site holds a jmp). Mismatch otherwise,
// with details in `why`.
State check(Target& t, const Patch& p, std::string& why);

// Installs a C++ midhook at w.va. Returns false with `err` set on failure.
using MidhookInstaller = std::function<bool(const Write& w, std::string& err)>;

// Applies a patch whose state is Pristine. Writes are re-read afterwards to confirm.
bool apply(Target& t, const Patch& p, const MidhookInstaller& midhook, std::string& err);

} // namespace sfp
