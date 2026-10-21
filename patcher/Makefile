# STAR FORGE patcher build (run from patcher/).
#   make                       -> build/sfp (Linux CLI) + build/win32/libcrypto-1_1_tg.dll
#   make PATCHES=<dir>         -> also build/dist/*.patchset.toml from a server's patch sources, with
#                                 <dir>/hooks/*.cpp compiled into the DLL
#   make test [PATCHES=<dir>]  -> midhook test under wine (+ bake test if <dir>/expected_sha1.txt exists)
# A server's patches live in its own repo, not here; see README "Patch projects".
# Windows side cross-compiles with clang-cl + lld-link against the MSVC CRT/SDK fetched by xwin
# (see README "Toolchain").

PATCHES    ?=
XWIN       ?= $(HOME)/.cache/star-forge-xwin/xwin
SC_CLOUD   ?= $(or $(STAR_FORGE_GAME_DIR),$(HOME)/.local/share/Steam/steamapps/common/star conflict)/cloud
CXX_NATIVE ?= g++
CLANG_CL   ?= clang-cl
LLD_LINK   ?= lld-link
WINE       ?= wine

B := build
CORE := src/core/sha1.cpp src/core/pe.cpp src/core/patchset.cpp src/core/engine.cpp
TOML := src/core/toml_impl.cpp
CORE_H := $(wildcard src/core/*.hpp)

NATIVE_FLAGS := -std=c++17 -O2 -g -Wall -Wextra -Isrc/core -Ithird_party/tomlplusplus -DTOML_HEADER_ONLY=0

WIN_INC := /imsvc $(XWIN)/crt/include /imsvc $(XWIN)/sdk/include/ucrt /imsvc $(XWIN)/sdk/include/um /imsvc $(XWIN)/sdk/include/shared
WIN_CXX := --target=i686-pc-windows-msvc /std:c++17 /EHsc /O2 /Zi /MT /W3 /DNOMINMAX /DWIN32_LEAN_AND_MEAN /D_CRT_SECURE_NO_WARNINGS -Wno-deprecated-literal-operator \
           /DTOML_HEADER_ONLY=0 /Isrc/core /Isrc/dll /Ithird_party/tomlplusplus $(WIN_INC) -Wno-unused-command-line-argument
WIN_LIBPATH := /libpath:$(XWIN)/crt/lib/x86 /libpath:$(XWIN)/sdk/lib/um/x86 /libpath:$(XWIN)/sdk/lib/ucrt/x86
WIN_LD := /nologo /machine:x86 /debug /ignore:4099 $(WIN_LIBPATH)

# Project hooks are staged into build/hooks/ (a path without spaces) before the DLL is linked.
HOOKS_STAGE := $(B)/hooks
DLL_SRC := src/dll/dllmain.cpp src/dll/loader.cpp src/dll/midhook.cpp src/dll/log.cpp $(wildcard $(HOOKS_STAGE)/*.cpp)
DLL_OBJ := $(patsubst %.cpp,$(B)/win32/obj/%.obj,$(DLL_SRC) $(CORE) $(TOML))
DEF := src/dll/libcrypto-1_1_tg.def

.PHONY: all native dll dll-link stage-hooks patchsets test bake-test hook-test clean
all: native patchsets dll
native: $(B)/sfp

$(B)/native/toml_impl.o: $(TOML)
	@mkdir -p $(@D)
	$(CXX_NATIVE) $(NATIVE_FLAGS) -c $< -o $@

$(B)/sfp: src/cli/sfp.cpp src/cli/sources.cpp src/cli/sources.hpp $(CORE) $(CORE_H) $(B)/native/toml_impl.o
	@mkdir -p $(@D)
	$(CXX_NATIVE) $(NATIVE_FLAGS) src/cli/sfp.cpp src/cli/sources.cpp $(CORE) $(B)/native/toml_impl.o -o $@

patchsets: $(B)/sfp
	@rm -rf $(B)/dist
	@if [ -z "$(PATCHES)" ]; then echo "patchsets: PATCHES not set, skipping (make PATCHES=<server repo>/patches)"; \
	else $(B)/sfp build "$(PATCHES)" $(B)/dist; fi

# $(B)/hooks.list changes whenever the set of staged hooks does, forcing a relink (so a build
# without PATCHES never keeps a previous project's hooks).
stage-hooks:
	@rm -rf $(HOOKS_STAGE) && mkdir -p $(HOOKS_STAGE)
	@if [ -n "$(PATCHES)" ] && ls "$(PATCHES)"/hooks/*.cpp >/dev/null 2>&1; then \
		cp -p "$(PATCHES)"/hooks/*.cpp $(HOOKS_STAGE)/ && echo "hooks: $$(ls $(HOOKS_STAGE) | tr '\n' ' ')"; fi
	@ls $(HOOKS_STAGE) > $(B)/hooks.list.new; \
	if cmp -s $(B)/hooks.list.new $(B)/hooks.list; then rm $(B)/hooks.list.new; else mv $(B)/hooks.list.new $(B)/hooks.list; fi

# Re-run make after staging so $(wildcard $(HOOKS_STAGE)/*.cpp) sees the project's hooks.
dll: stage-hooks
	@$(MAKE) --no-print-directory dll-link
dll-link: $(B)/win32/libcrypto-1_1_tg.dll

$(B)/win32/obj/%.obj: %.cpp $(CORE_H) $(wildcard src/dll/*.hpp)
	@mkdir -p $(@D)
	$(CLANG_CL) $(WIN_CXX) /c $< /Fo$@

$(B)/win32/libcrypto-1_1_tg.dll: $(DLL_OBJ) $(DEF) $(B)/hooks.list
	$(LLD_LINK) $(WIN_LD) /dll /def:$(DEF) /out:$@ $(DLL_OBJ) kernel32.lib user32.lib

$(B)/win32/hooktest.exe: tests/hooktest.cpp $(B)/win32/obj/src/dll/midhook.obj $(B)/win32/obj/src/dll/log.obj
	@mkdir -p $(B)/win32/obj/tests
	$(CLANG_CL) $(WIN_CXX) /c tests/hooktest.cpp /Fo$(B)/win32/obj/tests/hooktest.obj
	$(LLD_LINK) $(WIN_LD) /subsystem:console /out:$@ $(B)/win32/obj/tests/hooktest.obj $(B)/win32/obj/src/dll/midhook.obj $(B)/win32/obj/src/dll/log.obj kernel32.lib

test: hook-test bake-test

# With PATCHES set and <PATCHES>/expected_sha1.txt present: baking every default patch onto the
# pristine Steam exes must reproduce the listed sha1s byte for byte.
bake-test: native patchsets
	@if [ -n "$(PATCHES)" ] && [ -f "$(PATCHES)/expected_sha1.txt" ]; then \
		SC_CLOUD="$(SC_CLOUD)" tests/bake_test.sh $(B) "$(PATCHES)/expected_sha1.txt"; \
	else echo "bake-test: no PATCHES/expected_sha1.txt, skipping"; fi

hook-test: $(B)/win32/hooktest.exe
	WINEDEBUG=-all $(WINE) $(B)/win32/hooktest.exe

clean:
	rm -rf $(B)
