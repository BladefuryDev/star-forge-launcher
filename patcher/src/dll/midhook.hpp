// Midhook installation (internal; hook authors use sfp_hook.hpp).
#pragma once
#include "sfp_hook.hpp"

#include <cstddef>
#include <string>

namespace sfp {

// Writes to code pages (VirtualProtect + memcpy + FlushInstructionCache).
bool write_code(void* addr, const void* src, size_t n);

// Replaces `n` (>= 5) bytes at `site` with a jmp to a thunk calling `fn`; `stolen` are the
// original bytes, re-executed from the thunk when fn returns 0.
bool install_midhook(uint32_t site, const uint8_t* stolen, size_t n, MidhookFn fn, std::string& err);

} // namespace sfp
