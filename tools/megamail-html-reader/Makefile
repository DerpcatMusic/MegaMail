# SPDX-License-Identifier: AGPL-3.0-or-later
# Parent runs this ONLY in its single batch build window. No Cargo integration.
CC ?= cc
OUT ?= build
UI_PKGS = gtk+-3.0 webkit2gtk-4.1 json-glib-1.0 libxml-2.0
EXT_PKGS = webkit2gtk-web-extension-4.1 json-glib-1.0 gmodule-2.0
HARDEN = -std=c11 -O2 -Wall -Wextra -Wformat=2 -Werror=implicit-function-declaration -fstack-protector-strong -D_FORTIFY_SOURCE=2
LINK_HARDEN = -Wl,-z,relro,-z,now
.PHONY: all mutation clean
all: $(OUT)/megamail-html-reader $(OUT)/extensions/libmegamail-reader-policy.so
$(OUT)/megamail-html-reader: main.c policy.c request_policy.c sandbox.c policy_tests.c policy.h sandbox.h
	mkdir -p $(OUT)
	$(CC) $(HARDEN) $(CFLAGS) $(CPPFLAGS) $$(pkg-config --cflags $(UI_PKGS)) main.c policy.c request_policy.c sandbox.c policy_tests.c -o $@ $(LINK_HARDEN) $$(pkg-config --libs $(UI_PKGS))
$(OUT)/extensions/libmegamail-reader-policy.so: extension.c request_policy.c policy.h
	mkdir -p $(OUT)/extensions
	$(CC) $(HARDEN) $(CFLAGS) $(CPPFLAGS) -fPIC -shared $$(pkg-config --cflags $(EXT_PKGS)) extension.c request_policy.c -o $@ $(LINK_HARDEN) $$(pkg-config --libs $(EXT_PKGS))
# Deliberately distinct synthetic-only package. Ready policy is NOT accepted by
# ReaderHost. Do not install or supply real mail to this variant.
mutation:
	$(MAKE) OUT=build-no-csp CPPFLAGS=-DMM_TEST_NO_CSP all
clean:
	rm -rf build build-no-csp
