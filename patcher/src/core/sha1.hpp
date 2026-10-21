#pragma once
#include <cstddef>
#include <cstdint>
#include <string>

namespace sfp {

class Sha1 {
public:
    Sha1();
    void update(const void* data, size_t len);
    std::string hex(); // finalizes

private:
    void block(const uint8_t* p);
    uint32_t h_[5];
    uint8_t buf_[64];
    size_t buf_len_ = 0;
    uint64_t total_ = 0;
};

std::string sha1_bytes(const void* data, size_t len);

} // namespace sfp
