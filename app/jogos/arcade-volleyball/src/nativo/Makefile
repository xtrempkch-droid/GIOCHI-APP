# Arcade Volleyball — port nativo C++17 + SDL2.
#
#   make                  # host (x86-64), para testar no PC
#   make arm64            # aarch64, para o aparelho (exige o cache cruzado)
#   make limpar
#
# O caminho recomendado é `bash scripts/compilar-jogo-cpp-arm64.sh`, que monta
# o cache cruzado se ele não existir e chama o alvo `arm64` daqui. Este Makefile
# é a **única** fonte das bandeiras de compilação: o script não repete nenhuma.
#
# Por que não há caminho de include escrito à mão para o SDL2 do aparelho: o
# sysroot cruzado já tem `usr/include/SDL2`, e o `-I` vem só do sysroot. Foi a
# lição do Makefile do antecessor, que apontava para
# `dist/<id>/build/host/<tuple>/sysroot` — caminho de outro repositório.

CXX      ?= c++
CXXSTD   := -std=c++17
AVISOS   := -Wall -Wextra -Wshadow -Wformat=2 -Wundef
OTIMIZ   := -O2
FONTE    := arcade_volleyball.cpp
SAIDA    := arcade-volleyball

# ── host ────────────────────────────────────────────────────
# O prefixo local do `compilar-launcher.sh` tem o SDL2 de desenvolvimento e o
# `_real_SDL_config.h` multiarch, que é o include que o Debian exige.
PREFIXO_SDL2 ?= $(HOME)/.cache/giochi-sdl2/root
INCLUDES_HOST := -I$(PREFIXO_SDL2)/usr/include \
                 -I$(PREFIXO_SDL2)/usr/include/SDL2 \
                 -I$(PREFIXO_SDL2)/usr/include/x86_64-linux-gnu
LIBS_HOST     := -L$(PREFIXO_SDL2)/usr/lib/x86_64-linux-gnu -lSDL2 \
                 -Wl,-rpath,$(PREFIXO_SDL2)/usr/lib/x86_64-linux-gnu

# ── aarch64 ─────────────────────────────────────────────────
# `SYSROOT_CUZ` e `CXX_CUZ` vêm do script. Sem eles, o alvo falha dizendo o que
# fazer — em vez de compilar contra o glibc do PC e falhar no link com uma
# mensagem que não fala de arquitetura.
CACHE_CUZ  ?= $(HOME)/.cache/giochi-crosscpp
SYSROOT_CUZ ?= $(CACHE_CUZ)/root
CXX_CUZ     ?= $(SYSROOT_CUZ)/usr/bin/aarch64-linux-gnu-g++
LIBS_ALVO   ?= $(HOME)/.cache/giochi-arm64/devlibs

.PHONY: todas host arm64 limpar conferir-cache

todas: host

host: $(SAIDA)-host

$(SAIDA)-host: $(FONTE)
	@test -d "$(PREFIXO_SDL2)/usr/include/SDL2" || { \
	  echo "$@: SDL2 de desenvolvimento ausente em $(PREFIXO_SDL2)" >&2; \
	  echo "  rode primeiro: bash scripts/compilar-launcher.sh" >&2; exit 1; }
	$(CXX) $(CXXSTD) $(AVISOS) $(OTIMIZ) -D_REENTRANT $(INCLUDES_HOST) \
	  $(FONTE) -o $@ $(LIBS_HOST)

arm64: $(SAIDA)

$(SAIDA): $(FONTE) conferir-cache
	# `LD_LIBRARY_PATH` aponta para as libs **do host** que o binutils cruzado
	# usa (`libopcodes`, `libbfd`): elas são x86-64, é o assembler que roda aqui.
	LD_LIBRARY_PATH="$(SYSROOT_CUZ)/usr/lib/x86_64-linux-gnu$(if $(LD_LIBRARY_PATH),:$(LD_LIBRARY_PATH))" \
	$(CXX_CUZ) --sysroot=$(SYSROOT_CUZ) $(CXXSTD) $(AVISOS) $(OTIMIZ) -D_REENTRANT \
	  -I$(SYSROOT_CUZ)/usr/include \
	  -I$(SYSROOT_CUZ)/usr/include/SDL2 \
	  -I$(SYSROOT_CUZ)/usr/include/aarch64-linux-gnu \
	  $(FONTE) -o $@ \
	  -L$(LIBS_ALVO) -L$(SYSROOT_CUZ)/usr/aarch64-linux-gnu/lib \
	  -L$(SYSROOT_CUZ)/usr/lib/aarch64-linux-gnu \
	  -lSDL2 -lpthread \
	  -Wl,--dynamic-linker=/lib/ld-linux-aarch64.so.1
	@file -b $@ | cut -d, -f1-2

conferir-cache:
	@test -x "$(CXX_CUZ)" || { \
	  echo "compilador cruzado ausente: $(CXX_CUZ)" >&2; \
	  echo "  rode: bash scripts/compilar-jogo-cpp-arm64.sh (monta o cache)" >&2; exit 1; }

limpar:
	rm -f $(SAIDA) $(SAIDA)-host
