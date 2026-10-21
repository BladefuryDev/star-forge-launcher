#include "pe.hpp"

#include <cstring>

namespace sfp::pe {

template <class T> static bool rd(const uint8_t* d, size_t n, size_t off, T& v) {
    if (off + sizeof(T) > n) return false;
    std::memcpy(&v, d + off, sizeof(T));
    return true;
}

bool parse(const uint8_t* d, size_t n, Info& out, std::string& err) {
    uint16_t mz = 0, nsec = 0, opt_size = 0, magic = 0;
    uint32_t pe_off = 0, sig = 0;
    if (!rd(d, n, 0, mz) || mz != 0x5a4d) { err = "not an MZ file"; return false; }
    if (!rd(d, n, 0x3c, pe_off) || !rd(d, n, pe_off, sig) || sig != 0x4550) { err = "no PE header"; return false; }
    rd(d, n, pe_off + 6, nsec);
    rd(d, n, pe_off + 20, opt_size);
    size_t opt = pe_off + 24;
    if (!rd(d, n, opt, magic)) { err = "truncated optional header"; return false; }
    if (magic != 0x10b) { err = "only PE32 (32-bit) images are supported"; return false; }
    rd(d, n, opt + 28, out.image_base);
    rd(d, n, opt + 32, out.section_alignment);
    rd(d, n, opt + 36, out.file_alignment);
    rd(d, n, opt + 56, out.size_of_image);
    rd(d, n, opt + 96, out.export_rva);
    rd(d, n, opt + 100, out.export_size);
    size_t st = opt + opt_size;
    out.sections.clear();
    for (uint16_t i = 0; i < nsec; i++) {
        size_t o = st + 40 * size_t(i);
        if (o + 40 > n) { err = "truncated section table"; return false; }
        Section s;
        char name[9] = {};
        std::memcpy(name, d + o, 8);
        s.name = name;
        rd(d, n, o + 8, s.vsize);
        rd(d, n, o + 12, s.va);
        rd(d, n, o + 16, s.raw_size);
        rd(d, n, o + 20, s.raw_ptr);
        s.hdr_off = uint32_t(o);
        out.sections.push_back(s);
    }
    return true;
}

const Section* section_for_file_range(const Info& pe, uint32_t rva, uint32_t len) {
    for (auto& s : pe.sections)
        if (rva >= s.va && uint64_t(rva) + len <= uint64_t(s.va) + s.raw_size) return &s;
    return nullptr;
}

bool exports(const uint8_t* d, size_t n, const Info& pe, std::vector<Export>& out, std::string& err) {
    auto off = [&](uint32_t rva) -> size_t {
        for (auto& s : pe.sections)
            if (rva >= s.va && rva < s.va + s.raw_size) return size_t(rva - s.va) + s.raw_ptr;
        return size_t(-1);
    };
    size_t e = off(pe.export_rva);
    if (!pe.export_rva || e == size_t(-1)) { err = "no export directory"; return false; }
    uint32_t base = 0, nnames = 0, names_rva = 0, ords_rva = 0;
    rd(d, n, e + 16, base);
    rd(d, n, e + 24, nnames);
    rd(d, n, e + 32, names_rva);
    rd(d, n, e + 36, ords_rva);
    size_t names = off(names_rva), ords = off(ords_rva);
    if (names == size_t(-1) || ords == size_t(-1)) { err = "bad export tables"; return false; }
    out.clear();
    for (uint32_t i = 0; i < nnames; i++) {
        uint32_t nrva = 0;
        uint16_t ord = 0;
        if (!rd(d, n, names + 4 * i, nrva) || !rd(d, n, ords + 2 * i, ord)) { err = "truncated exports"; return false; }
        size_t so = off(nrva);
        if (so == size_t(-1)) { err = "bad export name"; return false; }
        Export x;
        while (so < n && d[so]) x.name += char(d[so++]);
        x.ordinal = base + ord;
        out.push_back(x);
    }
    return true;
}

} // namespace sfp::pe
