// Minimal PE32 header parsing: sections and exports.
#pragma once
#include <cstdint>
#include <string>
#include <vector>

namespace sfp::pe {

struct Section {
    std::string name;
    uint32_t va = 0; // RVA
    uint32_t vsize = 0;
    uint32_t raw_ptr = 0;
    uint32_t raw_size = 0;
    uint32_t hdr_off = 0; // file offset of this section header
};

struct Info {
    uint32_t image_base = 0;
    uint32_t size_of_image = 0;
    uint32_t file_alignment = 0;
    uint32_t section_alignment = 0;
    uint32_t export_rva = 0, export_size = 0;
    std::vector<Section> sections;
};

bool parse(const uint8_t* d, size_t n, Info& out, std::string& err);

// Section whose file-backed bytes contain [rva, rva+len). This includes the slack past
// VirtualSize up to SizeOfRawData, which is where code caves live. nullptr if none.
const Section* section_for_file_range(const Info& pe, uint32_t rva, uint32_t len);

struct Export {
    std::string name;
    uint32_t ordinal = 0;
};
bool exports(const uint8_t* d, size_t n, const Info& pe, std::vector<Export>& out, std::string& err);

} // namespace sfp::pe
