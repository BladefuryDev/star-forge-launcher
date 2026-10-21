// Loads the human-edited patch sources (<project>/<Exe>/) and resolves them into a Patchset:
// assembles `asm` writes, encodes `jmp`/`call` writes, fills implicit orig bytes.
#pragma once
#include "patchset.hpp"

#include <string>
#include <vector>

namespace sfp {

bool load_sources(const std::string& dir, Patchset& out, std::vector<std::string>& warnings, std::string& err);

} // namespace sfp
