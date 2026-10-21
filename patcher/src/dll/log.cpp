#include "log.hpp"

#include <windows.h>

#include <cstdarg>
#include <cstdio>

namespace sfp {

static FILE* g_log = nullptr;

void log_open(const std::wstring& path) {
    if (g_log) return;
    g_log = _wfopen(path.c_str(), L"ab");
}

void log(const char* fmt, ...) {
    char msg[2048];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof msg, fmt, ap);
    va_end(ap);
    SYSTEMTIME t;
    GetLocalTime(&t);
    FILE* f = g_log ? g_log : stderr;
    fprintf(f, "%04d-%02d-%02d %02d:%02d:%02d.%03d [%lu] %s\n", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond,
            t.wMilliseconds, GetCurrentProcessId(), msg);
    fflush(f);
}

std::string narrow(const std::wstring& w) {
    if (w.empty()) return {};
    int n = WideCharToMultiByte(CP_UTF8, 0, w.data(), int(w.size()), nullptr, 0, nullptr, nullptr);
    std::string s(size_t(n), '\0');
    WideCharToMultiByte(CP_UTF8, 0, w.data(), int(w.size()), s.data(), n, nullptr, nullptr);
    return s;
}

} // namespace sfp
