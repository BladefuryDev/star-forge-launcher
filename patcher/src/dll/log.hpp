#pragma once
#include <string>

namespace sfp {

// Opens the per-exe log file (append). Until then, log() goes to stderr.
void log_open(const std::wstring& path);
void log(const char* fmt, ...);

std::string narrow(const std::wstring& w); // UTF-8

} // namespace sfp
