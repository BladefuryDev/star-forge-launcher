// Small helpers shared by the CLI and the DLL.
#pragma once
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

namespace sfp {

using Bytes = std::vector<uint8_t>;

// Parses hex like "8b80 0c2e0000" (whitespace and '_' ignored). Returns false on bad input.
inline bool parse_hex(const std::string& s, Bytes& out) {
    out.clear();
    int hi = -1;
    for (char c : s) {
        int v;
        if (c >= '0' && c <= '9') v = c - '0';
        else if (c >= 'a' && c <= 'f') v = c - 'a' + 10;
        else if (c >= 'A' && c <= 'F') v = c - 'A' + 10;
        else if (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '_') continue;
        else return false;
        if (hi < 0) hi = v;
        else { out.push_back(uint8_t(hi << 4 | v)); hi = -1; }
    }
    return hi < 0;
}

inline std::string to_hex(const uint8_t* p, size_t n) {
    static const char* d = "0123456789abcdef";
    std::string s;
    s.reserve(n * 2);
    for (size_t i = 0; i < n; i++) { s += d[p[i] >> 4]; s += d[p[i] & 15]; }
    return s;
}
inline std::string to_hex(const Bytes& b) { return to_hex(b.data(), b.size()); }

inline std::string hex32(uint32_t v) {
    char buf[16];
    std::snprintf(buf, sizeof buf, "0x%x", v);
    return buf;
}

} // namespace sfp
