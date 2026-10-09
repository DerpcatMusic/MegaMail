# Locked dependency and license inventory

Generated from the locked `apps/megamail` Cargo graph by `tools/license-inventory.py`.
This inventory records declared SPDX identifiers and copies actual package-root license,
copyright, and notice files when present. A declaration is not a substitute for a
missing license text; missing or skipped files are listed explicitly below. GPUI Kit's
crates.io archive omits its Apache file, so its exact tagged upstream copy is included.

Lockfile SHA-256: `79608a42d35e143bcf5c1c4fefc26123c47add0fa6767466d47df35aa0ceddbc`

Resolved remote package graph SHA-256: `9db40d791f9dcf257c262d648464ad613ec68f71e3d667e30ef270aeddf9aec9`

Remote packages: 951

## Project and bundled asset notices

| Bundle file | Source | SHA-256 |
|---|---|---|
| [`project/LICENSE`](project/LICENSE) | `LICENSE` | `3856ebcc85b6d97cc45645b7195dc9d622ddcfce8739ada6ab876b6724a44d8b` |
| [`project/zeron-theme/LICENSE-MIT`](project/zeron-theme/LICENSE-MIT) | `crates/zeron-theme/LICENSE-MIT` | `e776600f641baae3fd37387829b40fe2d7c12d01785dc8b846658b7cbb0342e3` |
| [`project/THIRD_PARTY_NOTICES.md`](project/THIRD_PARTY_NOTICES.md) | `THIRD_PARTY_NOTICES.md` | `9500a2ea78226eeb9165cf598422ea1c5221c75532c6c353c0dd2ffa786c1643` |
| [`project/gpui-base/LICENSE-APACHE`](project/gpui-base/LICENSE-APACHE) | `vendor/gpui-base/LICENSE-APACHE` | `d1b0449e5478c574ba4f686c2656df7fe77d66821a61f8b6ed3378a58ed9a811` |
| [`project/gpui-base/MEGAMAIL_PATCH.md`](project/gpui-base/MEGAMAIL_PATCH.md) | `vendor/gpui-base/MEGAMAIL_PATCH.md` | `0297419b18d384447480924d94a061f8fa72ba60a471da0a5f12362b1e575d10` |
| [`project/gpui-pre/LICENSE-APACHE`](project/gpui-pre/LICENSE-APACHE) | `vendor/gpui-pre/LICENSE-APACHE` | `752daf2fb234ca4a1fa372c073fe127f44b7b90fd2529ae44273a64f9d53da7a` |
| [`project/gpui-pre/MEGAMAIL_PATCH.md`](project/gpui-pre/MEGAMAIL_PATCH.md) | `vendor/gpui-pre/MEGAMAIL_PATCH.md` | `55120cd3333f66fe1ca0436c52aabe362f8a0d939086b5cfee001679774aa199` |
| [`project/gpui-pre/IBM-Plex-OFL.txt`](project/gpui-pre/IBM-Plex-OFL.txt) | `vendor/gpui-pre/test-fonts/ibm-plex-sans/license.txt` | `91c25c350d3cac39da2736d74f7ba37ef648f5237a4e330a240615bc8d8c4360` |
| [`project/gpui-pre/Lilex-OFL.txt`](project/gpui-pre/Lilex-OFL.txt) | `vendor/gpui-pre/test-fonts/lilex/OFL.txt` | `a356708a399e3f54b937ec7d9b722c26dabba21365b60f95c56eb7ebca0dee40` |
| [`project/gpui-pre-wgpu/LICENSE-APACHE`](project/gpui-pre-wgpu/LICENSE-APACHE) | `vendor/gpui-pre-wgpu/LICENSE-APACHE` | `752daf2fb234ca4a1fa372c073fe127f44b7b90fd2529ae44273a64f9d53da7a` |
| [`project/gpui-pre-wgpu/MEGAMAIL_PATCH.md`](project/gpui-pre-wgpu/MEGAMAIL_PATCH.md) | `vendor/gpui-pre-wgpu/MEGAMAIL_PATCH.md` | `8a8af4757ea2a0e8cea8cd7bb9f345ab98152b1e8e3415c7a9eb97f5785c4201` |
| [`project/cosmic-text/LICENSE-APACHE`](project/cosmic-text/LICENSE-APACHE) | `vendor/cosmic-text/LICENSE-APACHE` | `000b4962e6b27176a0ff89cce4be555b16472cafb5671eb2804a8fdac6854793` |
| [`project/cosmic-text/LICENSE-MIT`](project/cosmic-text/LICENSE-MIT) | `vendor/cosmic-text/LICENSE-MIT` | `95557bafe728379e206f3b6d4aceeaa054d271c71d722411a6222e2dec01138c` |
| [`project/cosmic-text/MEGAMAIL_PATCH.md`](project/cosmic-text/MEGAMAIL_PATCH.md) | `vendor/cosmic-text/MEGAMAIL_PATCH.md` | `bc15d65eccb6b92ed79496341d58ba93500de4d873b7cdc7608b416fd12375cb` |
| [`project/assets/Geist-OFL.txt`](project/assets/Geist-OFL.txt) | `apps/megamail/assets/fonts/licenses/Geist-OFL.txt` | `942560b236adfa83745b2c64e5fc09ebaf91cb331751b1157eb92187e5d6e930` |
| [`project/assets/LICENSE-LUCIDE`](project/assets/LICENSE-LUCIDE) | `apps/megamail/assets/icons/LICENSE-LUCIDE` | `b495047bd93a9b06913511076f504daba17d5bbeb3e0650f3bb53a4220329c57` |

`project/THIRD_PARTY_NOTICES.md` is included verbatim. It contains the full Zeron MIT notice;
the Geist OFL and Lucide ISC/Feather MIT texts are included as separate files above.
GPUI Kit's tagged Apache-2.0 text appears with its dependency entry below.

## Locked remote dependency packages

| Package | Version | Cargo SPDX declaration | Bundled license/notice files |
|---|---:|---|---|
| `accesskit` | `0.24.1` | `MIT OR Apache-2.0` | **No regular license file found** |
| `accesskit_atspi_common` | `0.19.1` | `MIT OR Apache-2.0` | **No regular license file found** |
| `accesskit_consumer` | `0.38.0` | `MIT OR Apache-2.0` | **No regular license file found** |
| `accesskit_macos` | `0.26.3` | `MIT OR Apache-2.0` | **No regular license file found** |
| `accesskit_unix` | `0.22.1` | `MIT OR Apache-2.0` | **No regular license file found** |
| `accesskit_windows` | `0.34.0` | `MIT OR Apache-2.0` | **No regular license file found** |
| `addr2line` | `0.25.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/addr2line-0.25.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/addr2line-0.25.1/LICENSE-MIT) |
| `adler2` | `2.0.1` | `0BSD OR MIT OR Apache-2.0` | [`LICENSE-0BSD`](dependencies/adler2-2.0.1/LICENSE-0BSD), [`LICENSE-APACHE`](dependencies/adler2-2.0.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/adler2-2.0.1/LICENSE-MIT) |
| `aes` | `0.8.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/aes-0.8.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/aes-0.8.4/LICENSE-MIT) |
| `ahash` | `0.8.12` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ahash-0.8.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ahash-0.8.12/LICENSE-MIT) |
| `aho-corasick` | `1.1.5` | `Unlicense OR MIT` | [`COPYING`](dependencies/aho-corasick-1.1.5/COPYING), [`LICENSE-MIT`](dependencies/aho-corasick-1.1.5/LICENSE-MIT) |
| `aligned` | `0.4.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/aligned-0.4.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/aligned-0.4.3/LICENSE-MIT) |
| `aligned-vec` | `0.6.4` | `MIT` | [`LICENSE`](dependencies/aligned-vec-0.6.4/LICENSE) |
| `allocator-api2` | `0.2.21` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/allocator-api2-0.2.21/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/allocator-api2-0.2.21/LICENSE-MIT) |
| `ammonia` | `4.2.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ammonia-4.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ammonia-4.2.1/LICENSE-MIT) |
| `android_system_properties` | `0.1.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/android_system_properties-0.1.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/android_system_properties-0.1.6/LICENSE-MIT) |
| `annotate-snippets` | `0.12.16` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/annotate-snippets-0.12.16/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/annotate-snippets-0.12.16/LICENSE-MIT) |
| `anstyle` | `1.0.14` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/anstyle-1.0.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/anstyle-1.0.14/LICENSE-MIT) |
| `anyhow` | `1.0.104` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/anyhow-1.0.104/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/anyhow-1.0.104/LICENSE-MIT) |
| `ar_archive_writer` | `0.5.3` | `Apache-2.0 WITH LLVM-exception` | [`LICENSE.txt`](dependencies/ar_archive_writer-0.5.3/LICENSE.txt) |
| `arbitrary` | `1.5.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/arbitrary-1.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/arbitrary-1.5.0/LICENSE-MIT) |
| `arc-swap` | `1.9.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/arc-swap-1.9.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/arc-swap-1.9.2/LICENSE-MIT) |
| `arg_enum_proc_macro` | `0.3.4` | `MIT` | [`LICENSE`](dependencies/arg_enum_proc_macro-0.3.4/LICENSE) |
| `arraydeque` | `0.5.1` | `MIT/Apache-2.0` | [`LICENSE`](dependencies/arraydeque-0.5.1/LICENSE) |
| `arrayref` | `0.3.9` | `BSD-2-Clause` | [`LICENSE`](dependencies/arrayref-0.3.9/LICENSE) |
| `arrayvec` | `0.7.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/arrayvec-0.7.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/arrayvec-0.7.8/LICENSE-MIT) |
| `as-raw-xcb-connection` | `1.0.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/as-raw-xcb-connection-1.0.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/as-raw-xcb-connection-1.0.1/LICENSE-MIT) |
| `as-slice` | `0.2.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/as-slice-0.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/as-slice-0.2.1/LICENSE-MIT) |
| `ash` | `0.38.0+1.3.281` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ash-0.38.0+1.3.281/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ash-0.38.0+1.3.281/LICENSE-MIT) |
| `ashpd` | `0.13.13` | `MIT` | [`LICENSE`](dependencies/ashpd-0.13.13/LICENSE) |
| `async-broadcast` | `0.7.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/async-broadcast-0.7.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-broadcast-0.7.2/LICENSE-MIT) |
| `async-channel` | `1.9.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-channel-1.9.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-channel-1.9.0/LICENSE-MIT) |
| `async-channel` | `2.5.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-channel-2.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-channel-2.5.0/LICENSE-MIT) |
| `async-compression` | `0.4.50` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/async-compression-0.4.50/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-compression-0.4.50/LICENSE-MIT) |
| `async-executor` | `1.14.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-executor-1.14.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-executor-1.14.0/LICENSE-MIT) |
| `async-fs` | `2.2.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-fs-2.2.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-fs-2.2.0/LICENSE-MIT) |
| `async-imap` | `0.10.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/async-imap-0.10.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-imap-0.10.4/LICENSE-MIT) |
| `async-io` | `2.6.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-io-2.6.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-io-2.6.0/LICENSE-MIT) |
| `async-lock` | `3.4.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-lock-3.4.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-lock-3.4.2/LICENSE-MIT) |
| `async-native-tls` | `0.5.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/async-native-tls-0.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-native-tls-0.5.0/LICENSE-MIT) |
| `async-net` | `2.0.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-net-2.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-net-2.0.0/LICENSE-MIT) |
| `async-process` | `2.5.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-process-2.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-process-2.5.0/LICENSE-MIT) |
| `async-recursion` | `1.2.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/async-recursion-1.2.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-recursion-1.2.0/LICENSE-MIT) |
| `async-signal` | `0.2.14` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-signal-0.2.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-signal-0.2.14/LICENSE-MIT) |
| `async-task` | `4.7.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/async-task-4.7.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-task-4.7.1/LICENSE-MIT) |
| `async-trait` | `0.1.92` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/async-trait-0.1.92/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/async-trait-0.1.92/LICENSE-MIT) |
| `atomic` | `0.5.3` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/atomic-0.5.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/atomic-0.5.3/LICENSE-MIT) |
| `atomic-waker` | `1.1.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/atomic-waker-1.1.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/atomic-waker-1.1.2/LICENSE-MIT), [`LICENSE-THIRD-PARTY`](dependencies/atomic-waker-1.1.2/LICENSE-THIRD-PARTY) |
| `atspi` | `0.29.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE2.txt`](dependencies/atspi-0.29.0/LICENSE-APACHE2.txt), [`LICENSE-MIT.txt`](dependencies/atspi-0.29.0/LICENSE-MIT.txt) |
| `atspi-common` | `0.13.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE2.txt`](dependencies/atspi-common-0.13.0/LICENSE-APACHE2.txt), [`LICENSE-MIT.txt`](dependencies/atspi-common-0.13.0/LICENSE-MIT.txt) |
| `atspi-proxies` | `0.13.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE2.txt`](dependencies/atspi-proxies-0.13.0/LICENSE-APACHE2.txt), [`LICENSE-MIT.txt`](dependencies/atspi-proxies-0.13.0/LICENSE-MIT.txt) |
| `autocfg` | `1.5.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/autocfg-1.5.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/autocfg-1.5.1/LICENSE-MIT) |
| `av-scenechange` | `0.14.1` | `MIT` | [`LICENSE`](dependencies/av-scenechange-0.14.1/LICENSE) |
| `av1-grain` | `0.2.5` | `BSD-2-Clause` | [`LICENSE`](dependencies/av1-grain-0.2.5/LICENSE) |
| `avif-serialize` | `0.8.9` | `BSD-3-Clause` | [`LICENSE`](dependencies/avif-serialize-0.8.9/LICENSE) |
| `backtrace` | `0.3.76` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/backtrace-0.3.76/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/backtrace-0.3.76/LICENSE-MIT) |
| `base62` | `2.2.6` | `MIT` | [`LICENSE`](dependencies/base62-2.2.6/LICENSE) |
| `base64` | `0.22.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/base64-0.22.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/base64-0.22.1/LICENSE-MIT) |
| `base64` | `0.23.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/base64-0.23.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/base64-0.23.1/LICENSE-MIT) |
| `bindgen` | `0.72.1` | `BSD-3-Clause` | [`LICENSE`](dependencies/bindgen-0.72.1/LICENSE) |
| `bit-set` | `0.9.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/bit-set-0.9.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bit-set-0.9.1/LICENSE-MIT) |
| `bit-vec` | `0.9.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/bit-vec-0.9.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bit-vec-0.9.1/LICENSE-MIT) |
| `bit_field` | `0.10.3` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/bit_field-0.10.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bit_field-0.10.3/LICENSE-MIT) |
| `bitflags` | `1.3.2` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/bitflags-1.3.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bitflags-1.3.2/LICENSE-MIT) |
| `bitflags` | `2.13.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/bitflags-2.13.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bitflags-2.13.2/LICENSE-MIT) |
| `bitstream-io` | `4.10.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/bitstream-io-4.10.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bitstream-io-4.10.0/LICENSE-MIT) |
| `block` | `0.1.6` | `MIT` | **No regular license file found** |
| `block-buffer` | `0.10.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/block-buffer-0.10.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/block-buffer-0.10.4/LICENSE-MIT) |
| `block-buffer` | `0.12.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/block-buffer-0.12.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/block-buffer-0.12.1/LICENSE-MIT) |
| `block-padding` | `0.3.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/block-padding-0.3.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/block-padding-0.3.3/LICENSE-MIT) |
| `block2` | `0.5.1` | `MIT` | **No regular license file found** |
| `block2` | `0.6.2` | `MIT` | **No regular license file found** |
| `blocking` | `1.7.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/blocking-1.7.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/blocking-1.7.0/LICENSE-MIT) |
| `bstr` | `1.13.1` | `MIT OR Apache-2.0` | [`COPYING`](dependencies/bstr-1.13.1/COPYING), [`LICENSE-APACHE`](dependencies/bstr-1.13.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bstr-1.13.1/LICENSE-MIT) |
| `built` | `0.8.1` | `MIT` | [`LICENSE`](dependencies/built-0.8.1/LICENSE) |
| `bumpalo` | `3.20.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/bumpalo-3.20.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bumpalo-3.20.3/LICENSE-MIT) |
| `bytemuck` | `1.25.2` | `Zlib OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/bytemuck-1.25.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bytemuck-1.25.2/LICENSE-MIT), [`LICENSE-ZLIB`](dependencies/bytemuck-1.25.2/LICENSE-ZLIB) |
| `bytemuck_derive` | `1.12.1` | `Zlib OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/bytemuck_derive-1.12.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bytemuck_derive-1.12.1/LICENSE-MIT), [`LICENSE-ZLIB`](dependencies/bytemuck_derive-1.12.1/LICENSE-ZLIB) |
| `byteorder` | `1.5.0` | `Unlicense OR MIT` | [`COPYING`](dependencies/byteorder-1.5.0/COPYING), [`LICENSE-MIT`](dependencies/byteorder-1.5.0/LICENSE-MIT) |
| `byteorder-lite` | `0.1.0` | `Unlicense OR MIT` | [`LICENSE-MIT`](dependencies/byteorder-lite-0.1.0/LICENSE-MIT) |
| `bytes` | `1.12.1` | `MIT` | [`LICENSE`](dependencies/bytes-1.12.1/LICENSE) |
| `bzip2` | `0.6.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/bzip2-0.6.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/bzip2-0.6.1/LICENSE-MIT) |
| `calloop` | `0.14.5` | `MIT` | [`LICENSE.txt`](dependencies/calloop-0.14.5/LICENSE.txt) |
| `calloop-wayland-source` | `0.4.1` | `MIT` | [`LICENSE.txt`](dependencies/calloop-wayland-source-0.4.1/LICENSE.txt) |
| `cbc` | `0.1.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cbc-0.1.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cbc-0.1.2/LICENSE-MIT) |
| `cbindgen` | `0.28.0` | `MPL-2.0` | [`LICENSE`](dependencies/cbindgen-0.28.0/LICENSE) |
| `cc` | `1.6.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cc-1.6.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cc-1.6.0/LICENSE-MIT) |
| `cexpr` | `0.6.0` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/cexpr-0.6.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cexpr-0.6.0/LICENSE-MIT) |
| `cfg-expr` | `0.20.10` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cfg-expr-0.20.10/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cfg-expr-0.20.10/LICENSE-MIT) |
| `cfg-if` | `1.0.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cfg-if-1.0.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cfg-if-1.0.5/LICENSE-MIT) |
| `cfg_aliases` | `0.2.2` | `MIT` | [`LICENSE`](dependencies/cfg_aliases-0.2.2/LICENSE), [`NOTICES.md`](dependencies/cfg_aliases-0.2.2/NOTICES.md) |
| `cgl` | `0.3.2` | `MIT / Apache-2.0` | [`COPYING`](dependencies/cgl-0.3.2/COPYING), [`LICENSE-APACHE`](dependencies/cgl-0.3.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cgl-0.3.2/LICENSE-MIT) |
| `chacha20` | `0.10.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/chacha20-0.10.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/chacha20-0.10.2/LICENSE-MIT) |
| `charset` | `0.1.5` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/charset-0.1.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/charset-0.1.5/LICENSE-MIT) |
| `chrono` | `0.4.45` | `MIT OR Apache-2.0` | [`LICENSE.txt`](dependencies/chrono-0.4.45/LICENSE.txt) |
| `chumsky` | `0.13.0` | `MIT` | [`LICENSE`](dependencies/chumsky-0.13.0/LICENSE) |
| `cipher` | `0.4.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cipher-0.4.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cipher-0.4.4/LICENSE-MIT) |
| `clang-sys` | `1.9.1` | `Apache-2.0` | [`LICENSE.txt`](dependencies/clang-sys-1.9.1/LICENSE.txt) |
| `cocoa` | `0.25.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cocoa-0.25.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cocoa-0.25.0/LICENSE-MIT) |
| `cocoa` | `0.26.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cocoa-0.26.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cocoa-0.26.1/LICENSE-MIT) |
| `cocoa-foundation` | `0.1.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cocoa-foundation-0.1.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cocoa-foundation-0.1.2/LICENSE-MIT) |
| `cocoa-foundation` | `0.2.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cocoa-foundation-0.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cocoa-foundation-0.2.1/LICENSE-MIT) |
| `codespan-reporting` | `0.13.1` | `Apache-2.0` | [`LICENSE`](dependencies/codespan-reporting-0.13.1/LICENSE) |
| `color_quant` | `1.1.0` | `MIT` | [`LICENSE`](dependencies/color_quant-1.1.0/LICENSE) |
| `compression-codecs` | `0.4.45` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/compression-codecs-0.4.45/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/compression-codecs-0.4.45/LICENSE-MIT) |
| `compression-core` | `0.4.33` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/compression-core-0.4.33/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/compression-core-0.4.33/LICENSE-MIT) |
| `concurrent-queue` | `2.5.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/concurrent-queue-2.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/concurrent-queue-2.5.0/LICENSE-MIT) |
| `console_error_panic_hook` | `0.1.7` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/console_error_panic_hook-0.1.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/console_error_panic_hook-0.1.7/LICENSE-MIT) |
| `const-oid` | `0.10.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/const-oid-0.10.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/const-oid-0.10.2/LICENSE-MIT) |
| `const-random` | `0.1.18` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/const-random-0.1.18/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/const-random-0.1.18/LICENSE-MIT) |
| `const-random-macro` | `0.1.16` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/const-random-macro-0.1.16/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/const-random-macro-0.1.16/LICENSE-MIT) |
| `convert_case` | `0.10.0` | `MIT` | [`LICENSE`](dependencies/convert_case-0.10.0/LICENSE) |
| `core-foundation` | `0.10.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-foundation-0.10.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-foundation-0.10.1/LICENSE-MIT) |
| `core-foundation` | `0.9.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-foundation-0.9.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-foundation-0.9.4/LICENSE-MIT) |
| `core-foundation-sys` | `0.8.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-foundation-sys-0.8.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-foundation-sys-0.8.7/LICENSE-MIT) |
| `core-graphics` | `0.23.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-graphics-0.23.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-graphics-0.23.2/LICENSE-MIT) |
| `core-graphics` | `0.24.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-graphics-0.24.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-graphics-0.24.0/LICENSE-MIT) |
| `core-graphics-helmer-fork` | `0.24.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-graphics-helmer-fork-0.24.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-graphics-helmer-fork-0.24.0/LICENSE-MIT) |
| `core-graphics-types` | `0.1.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-graphics-types-0.1.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-graphics-types-0.1.3/LICENSE-MIT) |
| `core-graphics-types` | `0.2.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-graphics-types-0.2.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-graphics-types-0.2.0/LICENSE-MIT) |
| `core-graphics2` | `0.5.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-graphics2-0.5.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-graphics2-0.5.2/LICENSE-MIT) |
| `core-text` | `21.0.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-text-21.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-text-21.0.0/LICENSE-MIT) |
| `core-video` | `0.5.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/core-video-0.5.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core-video-0.5.2/LICENSE-MIT) |
| `core_detect` | `1.0.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/core_detect-1.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/core_detect-1.0.0/LICENSE-MIT) |
| `core_maths` | `0.1.1` | `MIT` | [`LICENSE`](dependencies/core_maths-0.1.1/LICENSE) |
| `cpufeatures` | `0.2.17` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cpufeatures-0.2.17/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cpufeatures-0.2.17/LICENSE-MIT) |
| `cpufeatures` | `0.3.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/cpufeatures-0.3.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/cpufeatures-0.3.1/LICENSE-MIT) |
| `crc32fast` | `1.5.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/crc32fast-1.5.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/crc32fast-1.5.2/LICENSE-MIT) |
| `crossbeam-deque` | `0.8.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/crossbeam-deque-0.8.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/crossbeam-deque-0.8.8/LICENSE-MIT) |
| `crossbeam-epoch` | `0.9.21` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/crossbeam-epoch-0.9.21/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/crossbeam-epoch-0.9.21/LICENSE-MIT) |
| `crossbeam-queue` | `0.3.14` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/crossbeam-queue-0.3.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/crossbeam-queue-0.3.14/LICENSE-MIT) |
| `crossbeam-utils` | `0.8.23` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/crossbeam-utils-0.8.23/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/crossbeam-utils-0.8.23/LICENSE-MIT) |
| `crunchy` | `0.2.4` | `MIT` | [`LICENSE`](dependencies/crunchy-0.2.4/LICENSE) |
| `crypto-common` | `0.1.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/crypto-common-0.1.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/crypto-common-0.1.7/LICENSE-MIT) |
| `crypto-common` | `0.2.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/crypto-common-0.2.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/crypto-common-0.2.2/LICENSE-MIT) |
| `css-inline` | `0.21.2` | `MIT` | **No regular license file found** |
| `cssparser` | `0.37.0` | `MPL-2.0` | [`LICENSE`](dependencies/cssparser-0.37.0/LICENSE) |
| `cssparser` | `0.38.0` | `MPL-2.0` | [`LICENSE`](dependencies/cssparser-0.38.0/LICENSE) |
| `cssparser-macros` | `0.7.1` | `MPL-2.0` | [`LICENSE`](dependencies/cssparser-macros-0.7.1/LICENSE) |
| `ctor` | `1.0.13` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/ctor-1.0.13/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ctor-1.0.13/LICENSE-MIT) |
| `data-url` | `0.3.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/data-url-0.3.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/data-url-0.3.2/LICENSE-MIT) |
| `dbus` | `0.9.12` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/dbus-0.9.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dbus-0.9.12/LICENSE-MIT) |
| `dbus-secret-service` | `4.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/dbus-secret-service-4.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dbus-secret-service-4.1.0/LICENSE-MIT) |
| `deranged` | `0.5.8` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/deranged-0.5.8/LICENSE-Apache), [`LICENSE-MIT`](dependencies/deranged-0.5.8/LICENSE-MIT) |
| `derive_more` | `2.1.1` | `MIT` | [`LICENSE`](dependencies/derive_more-2.1.1/LICENSE) |
| `derive_more-impl` | `2.1.1` | `MIT` | [`LICENSE`](dependencies/derive_more-impl-2.1.1/LICENSE) |
| `digest` | `0.10.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/digest-0.10.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/digest-0.10.7/LICENSE-MIT) |
| `digest` | `0.11.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/digest-0.11.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/digest-0.11.3/LICENSE-MIT) |
| `dirs` | `5.0.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/dirs-5.0.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dirs-5.0.1/LICENSE-MIT) |
| `dirs` | `6.0.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/dirs-6.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dirs-6.0.0/LICENSE-MIT) |
| `dirs-sys` | `0.4.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/dirs-sys-0.4.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dirs-sys-0.4.1/LICENSE-MIT) |
| `dirs-sys` | `0.5.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/dirs-sys-0.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dirs-sys-0.5.0/LICENSE-MIT) |
| `dispatch` | `0.2.0` | `MIT` | **No regular license file found** |
| `dispatch2` | `0.3.1` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `displaydoc` | `0.2.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/displaydoc-0.2.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/displaydoc-0.2.7/LICENSE-MIT) |
| `dlib` | `0.5.3` | `MIT` | [`LICENSE.txt`](dependencies/dlib-0.5.3/LICENSE.txt) |
| `document-features` | `0.2.12` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/document-features-0.2.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/document-features-0.2.12/LICENSE-MIT) |
| `downcast-rs` | `1.2.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/downcast-rs-1.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/downcast-rs-1.2.1/LICENSE-MIT) |
| `dtoa` | `1.0.11` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/dtoa-1.0.11/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dtoa-1.0.11/LICENSE-MIT) |
| `dtoa-short` | `0.3.5` | `MPL-2.0` | [`LICENSE`](dependencies/dtoa-short-0.3.5/LICENSE) |
| `dunce` | `1.0.5` | `CC0-1.0 OR MIT-0 OR Apache-2.0` | [`LICENSE`](dependencies/dunce-1.0.5/LICENSE) |
| `dwrote` | `0.11.5` | `MPL-2.0` | **No regular license file found** |
| `dyn-clone` | `1.0.20` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/dyn-clone-1.0.20/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/dyn-clone-1.0.20/LICENSE-MIT) |
| `either` | `1.19.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/either-1.19.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/either-1.19.0/LICENSE-MIT) |
| `email-encoding` | `0.4.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/email-encoding-0.4.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/email-encoding-0.4.2/LICENSE-MIT) |
| `email_address` | `0.2.9` | `MIT` | [`LICENSE`](dependencies/email_address-0.2.9/LICENSE) |
| `embed-resource` | `3.0.12` | `MIT` | [`LICENSE`](dependencies/embed-resource-3.0.12/LICENSE) |
| `encoding_rs` | `0.8.42` | `(Apache-2.0 OR MIT) AND BSD-3-Clause` | [`LICENSE-APACHE`](dependencies/encoding_rs-0.8.42/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/encoding_rs-0.8.42/LICENSE-MIT), [`LICENSE-WHATWG`](dependencies/encoding_rs-0.8.42/LICENSE-WHATWG) |
| `encoding_rs_io` | `0.1.8` | `MIT OR Apache-2.0` | [`COPYING`](dependencies/encoding_rs_io-0.1.8/COPYING), [`LICENSE-APACHE`](dependencies/encoding_rs_io-0.1.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/encoding_rs_io-0.1.8/LICENSE-MIT) |
| `endi` | `1.1.1` | `MIT` | [`LICENSE-MIT`](dependencies/endi-1.1.1/LICENSE-MIT) |
| `enum-iterator` | `2.3.0` | `0BSD` | [`LICENSE`](dependencies/enum-iterator-2.3.0/LICENSE) |
| `enum-iterator-derive` | `1.5.0` | `0BSD` | [`LICENSE`](dependencies/enum-iterator-derive-1.5.0/LICENSE) |
| `enumflags2` | `0.7.12` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/enumflags2-0.7.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/enumflags2-0.7.12/LICENSE-MIT) |
| `enumflags2_derive` | `0.7.12` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/enumflags2_derive-0.7.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/enumflags2_derive-0.7.12/LICENSE-MIT) |
| `enumn` | `0.1.14` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/enumn-0.1.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/enumn-0.1.14/LICENSE-MIT) |
| `equator` | `0.4.2` | `MIT` | [`LICENSE`](dependencies/equator-0.4.2/LICENSE) |
| `equator-macro` | `0.4.2` | `MIT` | [`LICENSE`](dependencies/equator-macro-0.4.2/LICENSE) |
| `equivalent` | `1.0.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/equivalent-1.0.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/equivalent-1.0.2/LICENSE-MIT) |
| `erased-serde` | `0.4.10` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/erased-serde-0.4.10/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/erased-serde-0.4.10/LICENSE-MIT) |
| `errno` | `0.3.14` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/errno-0.3.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/errno-0.3.14/LICENSE-MIT) |
| `etagere` | `0.2.15` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/etagere-0.2.15/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/etagere-0.2.15/LICENSE-MIT) |
| `euclid` | `0.22.14` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/euclid-0.22.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/euclid-0.22.14/LICENSE-MIT) |
| `event-listener` | `2.5.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/event-listener-2.5.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/event-listener-2.5.3/LICENSE-MIT) |
| `event-listener` | `5.4.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/event-listener-5.4.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/event-listener-5.4.2/LICENSE-MIT) |
| `event-listener-strategy` | `0.5.4` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/event-listener-strategy-0.5.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/event-listener-strategy-0.5.4/LICENSE-MIT) |
| `exr` | `1.74.2` | `BSD-3-Clause` | [`LICENSE.md`](dependencies/exr-1.74.2/LICENSE.md) |
| `fallible-iterator` | `0.3.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/fallible-iterator-0.3.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/fallible-iterator-0.3.0/LICENSE-MIT) |
| `fallible-streaming-iterator` | `0.1.9` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/fallible-streaming-iterator-0.1.9/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/fallible-streaming-iterator-0.1.9/LICENSE-MIT) |
| `fastrand` | `2.5.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/fastrand-2.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/fastrand-2.5.0/LICENSE-MIT) |
| `fax` | `0.2.7` | `MIT` | [`LICENSE`](dependencies/fax-0.2.7/LICENSE) |
| `fdeflate` | `0.3.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/fdeflate-0.3.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/fdeflate-0.3.7/LICENSE-MIT) |
| `filedescriptor` | `0.8.3` | `MIT` | [`LICENSE.md`](dependencies/filedescriptor-0.8.3/LICENSE.md) |
| `find-msvc-tools` | `0.1.14` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/find-msvc-tools-0.1.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/find-msvc-tools-0.1.14/LICENSE-MIT) |
| `fixedbitset` | `0.5.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/fixedbitset-0.5.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/fixedbitset-0.5.7/LICENSE-MIT) |
| `flate2` | `1.1.10` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/flate2-1.1.10/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/flate2-1.1.10/LICENSE-MIT) |
| `float-cmp` | `0.9.0` | `MIT` | [`LICENSE`](dependencies/float-cmp-0.9.0/LICENSE) |
| `float-ord` | `0.3.2` | `MIT / Apache-2.0` | [`LICENSE-APACHE`](dependencies/float-ord-0.3.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/float-ord-0.3.2/LICENSE-MIT) |
| `float_next_after` | `1.0.0` | `MIT` | [`LICENSE`](dependencies/float_next_after-1.0.0/LICENSE) |
| `fluent-uri` | `0.1.4` | `MIT` | [`LICENSE`](dependencies/fluent-uri-0.1.4/LICENSE) |
| `flume` | `0.12.0` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/flume-0.12.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/flume-0.12.0/LICENSE-MIT) |
| `fnv` | `1.0.7` | `Apache-2.0 / MIT` | [`LICENSE-APACHE`](dependencies/fnv-1.0.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/fnv-1.0.7/LICENSE-MIT) |
| `foldhash` | `0.1.5` | `Zlib` | [`LICENSE`](dependencies/foldhash-0.1.5/LICENSE) |
| `foldhash` | `0.2.0` | `Zlib` | [`LICENSE`](dependencies/foldhash-0.2.0/LICENSE) |
| `font-types` | `0.11.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/font-types-0.11.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/font-types-0.11.3/LICENSE-MIT) |
| `font-types` | `0.12.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/font-types-0.12.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/font-types-0.12.6/LICENSE-MIT) |
| `fontconfig-parser` | `0.5.8` | `MIT` | [`LICENSE`](dependencies/fontconfig-parser-0.5.8/LICENSE) |
| `fontdb` | `0.23.0` | `MIT` | [`LICENSE`](dependencies/fontdb-0.23.0/LICENSE) |
| `foreign-types` | `0.3.2` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/foreign-types-0.3.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/foreign-types-0.3.2/LICENSE-MIT) |
| `foreign-types` | `0.5.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/foreign-types-0.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/foreign-types-0.5.0/LICENSE-MIT) |
| `foreign-types-macros` | `0.2.4` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/foreign-types-macros-0.2.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/foreign-types-macros-0.2.4/LICENSE-MIT) |
| `foreign-types-shared` | `0.1.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/foreign-types-shared-0.1.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/foreign-types-shared-0.1.1/LICENSE-MIT) |
| `foreign-types-shared` | `0.3.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/foreign-types-shared-0.3.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/foreign-types-shared-0.3.1/LICENSE-MIT) |
| `form_urlencoded` | `1.2.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/form_urlencoded-1.2.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/form_urlencoded-1.2.2/LICENSE-MIT) |
| `freetype-sys` | `0.20.1` | `MIT` | [`LICENSE`](dependencies/freetype-sys-0.20.1/LICENSE) |
| `fsevent-sys` | `4.1.0` | `MIT` | [`LICENSE`](dependencies/fsevent-sys-4.1.0/LICENSE) |
| `futf` | `0.1.5` | `MIT / Apache-2.0` | [`LICENSE-APACHE`](dependencies/futf-0.1.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futf-0.1.5/LICENSE-MIT) |
| `futures` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-0.3.34/LICENSE-MIT) |
| `futures-channel` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-channel-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-channel-0.3.34/LICENSE-MIT) |
| `futures-concurrency` | `7.7.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-concurrency-7.7.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-concurrency-7.7.1/LICENSE-MIT) |
| `futures-core` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-core-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-core-0.3.34/LICENSE-MIT) |
| `futures-executor` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-executor-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-executor-0.3.34/LICENSE-MIT) |
| `futures-io` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-io-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-io-0.3.34/LICENSE-MIT) |
| `futures-lite` | `2.6.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/futures-lite-2.6.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-lite-2.6.1/LICENSE-MIT), [`LICENSE-THIRD-PARTY`](dependencies/futures-lite-2.6.1/LICENSE-THIRD-PARTY) |
| `futures-macro` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-macro-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-macro-0.3.34/LICENSE-MIT) |
| `futures-sink` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-sink-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-sink-0.3.34/LICENSE-MIT) |
| `futures-task` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-task-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-task-0.3.34/LICENSE-MIT) |
| `futures-util` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/futures-util-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/futures-util-0.3.34/LICENSE-MIT) |
| `generic-array` | `0.14.7` | `MIT` | [`LICENSE`](dependencies/generic-array-0.14.7/LICENSE) |
| `gethostname` | `1.1.0` | `Apache-2.0` | [`LICENSE`](dependencies/gethostname-1.1.0/LICENSE) |
| `getrandom` | `0.2.17` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/getrandom-0.2.17/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/getrandom-0.2.17/LICENSE-MIT) |
| `getrandom` | `0.3.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/getrandom-0.3.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/getrandom-0.3.4/LICENSE-MIT) |
| `getrandom` | `0.4.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/getrandom-0.4.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/getrandom-0.4.3/LICENSE-MIT) |
| `gettext-rs` | `0.8.0` | `MIT` | [`LICENSE.txt`](dependencies/gettext-rs-0.8.0/LICENSE.txt) |
| `gettext-sys` | `0.27.0` | `MIT` | [`LICENSE.txt`](dependencies/gettext-sys-0.27.0/LICENSE.txt) |
| `gif` | `0.13.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/gif-0.13.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/gif-0.13.3/LICENSE-MIT) |
| `gif` | `0.14.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/gif-0.14.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/gif-0.14.2/LICENSE-MIT) |
| `gimli` | `0.32.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/gimli-0.32.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/gimli-0.32.3/LICENSE-MIT) |
| `gio` | `0.20.12` | `MIT` | [`LICENSE`](dependencies/gio-0.20.12/LICENSE) |
| `gio-sys` | `0.20.10` | `MIT` | [`LICENSE`](dependencies/gio-sys-0.20.10/LICENSE) |
| `gl_generator` | `0.14.0` | `Apache-2.0` | **No regular license file found** |
| `glib` | `0.20.12` | `MIT` | [`LICENSE`](dependencies/glib-0.20.12/LICENSE) |
| `glib-macros` | `0.20.12` | `MIT` | [`LICENSE`](dependencies/glib-macros-0.20.12/LICENSE) |
| `glib-sys` | `0.20.10` | `MIT` | [`LICENSE`](dependencies/glib-sys-0.20.10/LICENSE) |
| `glob` | `0.3.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/glob-0.3.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/glob-0.3.4/LICENSE-MIT) |
| `globset` | `0.4.20` | `Unlicense OR MIT` | [`COPYING`](dependencies/globset-0.4.20/COPYING), [`LICENSE-MIT`](dependencies/globset-0.4.20/LICENSE-MIT) |
| `globwalk` | `0.8.1` | `MIT` | [`LICENSE`](dependencies/globwalk-0.8.1/LICENSE) |
| `glow` | `0.17.0` | `MIT OR Apache-2.0 OR Zlib` | [`LICENSE-APACHE`](dependencies/glow-0.17.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/glow-0.17.0/LICENSE-MIT), [`LICENSE-ZLIB`](dependencies/glow-0.17.0/LICENSE-ZLIB) |
| `glutin_wgl_sys` | `0.6.1` | `Apache-2.0` | [`LICENSE`](dependencies/glutin_wgl_sys-0.6.1/LICENSE) |
| `gobject-sys` | `0.20.10` | `MIT` | [`LICENSE`](dependencies/gobject-sys-0.20.10/LICENSE) |
| `gpu-allocator` | `0.28.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpu-allocator-0.28.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/gpu-allocator-0.28.0/LICENSE-MIT) |
| `gpu-descriptor` | `0.3.2` | `MIT OR Apache-2.0` | **No regular license file found** |
| `gpu-descriptor-types` | `0.2.0` | `MIT OR Apache-2.0` | **No regular license file found** |
| `gpui-component` | `0.7.1` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-component-0.7.1/LICENSE-APACHE) |
| `gpui-component-macros` | `0.7.1` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-component-macros-0.7.1/LICENSE-APACHE) |
| `gpui-kit` | `0.7.1` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-kit-0.7.1/LICENSE-APACHE) (from [the matching upstream tag](https://raw.githubusercontent.com/longbridge/gpui-kit/v0.7.1/LICENSE-APACHE)) |
| `gpui-kit-assets` | `0.7.1` | `Apache-2.0` | [`LICENSE-LUCIDE`](dependencies/gpui-kit-assets-0.7.1/LICENSE-LUCIDE) |
| `gpui-pre-apple` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-apple-0.3.8/LICENSE-APACHE) |
| `gpui-pre-collections` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-collections-0.3.8/LICENSE-APACHE) |
| `gpui-pre-derive-refineable` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-derive-refineable-0.3.8/LICENSE-APACHE) |
| `gpui-pre-http-client` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-http-client-0.3.8/LICENSE-APACHE) |
| `gpui-pre-linux` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-linux-0.3.8/LICENSE-APACHE) |
| `gpui-pre-macos` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-macos-0.3.8/LICENSE-APACHE) |
| `gpui-pre-macros` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-macros-0.3.8/LICENSE-APACHE) |
| `gpui-pre-perf` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-perf-0.3.8/LICENSE-APACHE) |
| `gpui-pre-platform` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-platform-0.3.8/LICENSE-APACHE) |
| `gpui-pre-refineable` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-refineable-0.3.8/LICENSE-APACHE) |
| `gpui-pre-reqwest` | `0.12.15` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-reqwest-0.12.15/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/gpui-pre-reqwest-0.12.15/LICENSE-MIT) |
| `gpui-pre-scheduler` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-scheduler-0.3.8/LICENSE-APACHE) |
| `gpui-pre-shared-string` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-shared-string-0.3.8/LICENSE-APACHE) |
| `gpui-pre-sum-tree` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-sum-tree-0.3.8/LICENSE-APACHE) |
| `gpui-pre-util` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-util-0.3.8/LICENSE-APACHE) |
| `gpui-pre-util-macros` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-util-macros-0.3.8/LICENSE-APACHE) |
| `gpui-pre-web` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-web-0.3.8/LICENSE-APACHE) |
| `gpui-pre-windows` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-windows-0.3.8/LICENSE-APACHE) |
| `gpui-pre-zlog` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-zlog-0.3.8/LICENSE-APACHE) |
| `gpui-pre-ztracing` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-ztracing-0.3.8/LICENSE-APACHE) |
| `gpui-pre-ztracing-macro` | `0.3.8` | `Apache-2.0` | [`LICENSE-APACHE`](dependencies/gpui-pre-ztracing-macro-0.3.8/LICENSE-APACHE) |
| `granit-parser` | `1.3.0` | `MIT OR Apache-2.0` | [`LICENSE`](dependencies/granit-parser-1.3.0/LICENSE) |
| `h2` | `0.4.20` | `MIT` | [`LICENSE`](dependencies/h2-0.4.20/LICENSE) |
| `half` | `2.7.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/half-2.7.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/half-2.7.1/LICENSE-MIT) |
| `harfrust` | `0.5.2` | `MIT` | **No regular license file found** |
| `hash32` | `0.3.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hash32-0.3.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hash32-0.3.1/LICENSE-MIT) |
| `hashbrown` | `0.14.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hashbrown-0.14.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hashbrown-0.14.5/LICENSE-MIT) |
| `hashbrown` | `0.15.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hashbrown-0.15.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hashbrown-0.15.5/LICENSE-MIT) |
| `hashbrown` | `0.16.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hashbrown-0.16.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hashbrown-0.16.1/LICENSE-MIT) |
| `hashbrown` | `0.17.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hashbrown-0.17.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hashbrown-0.17.1/LICENSE-MIT) |
| `hashlink` | `0.9.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hashlink-0.9.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hashlink-0.9.1/LICENSE-MIT) |
| `heapless` | `0.9.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/heapless-0.9.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/heapless-0.9.3/LICENSE-MIT) |
| `heck` | `0.4.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/heck-0.4.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/heck-0.4.1/LICENSE-MIT) |
| `heck` | `0.5.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/heck-0.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/heck-0.5.0/LICENSE-MIT) |
| `hermit-abi` | `0.5.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hermit-abi-0.5.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hermit-abi-0.5.3/LICENSE-MIT) |
| `hex` | `0.4.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hex-0.4.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hex-0.4.3/LICENSE-MIT) |
| `hexf-parse` | `0.2.1` | `CC0-1.0` | **No regular license file found** |
| `hkdf` | `0.12.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hkdf-0.12.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hkdf-0.12.4/LICENSE-MIT) |
| `hmac` | `0.12.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hmac-0.12.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hmac-0.12.1/LICENSE-MIT) |
| `html5ever` | `0.27.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/html5ever-0.27.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/html5ever-0.27.0/LICENSE-MIT) |
| `html5ever` | `0.39.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/html5ever-0.39.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/html5ever-0.39.0/LICENSE-MIT) |
| `html5ever` | `0.40.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/html5ever-0.40.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/html5ever-0.40.1/LICENSE-MIT) |
| `http` | `1.5.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/http-1.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/http-1.5.0/LICENSE-MIT) |
| `http-body` | `1.1.0` | `MIT` | [`LICENSE`](dependencies/http-body-1.1.0/LICENSE) |
| `http-body-util` | `0.1.5` | `MIT` | [`LICENSE`](dependencies/http-body-util-0.1.5/LICENSE) |
| `httparse` | `1.10.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/httparse-1.10.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/httparse-1.10.1/LICENSE-MIT) |
| `httpdate` | `1.0.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/httpdate-1.0.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/httpdate-1.0.3/LICENSE-MIT) |
| `hybrid-array` | `0.4.15` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/hybrid-array-0.4.15/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/hybrid-array-0.4.15/LICENSE-MIT) |
| `hyper` | `1.12.0` | `MIT` | [`LICENSE`](dependencies/hyper-1.12.0/LICENSE) |
| `hyper-rustls` | `0.27.10` | `Apache-2.0 OR ISC OR MIT` | [`LICENSE-APACHE`](dependencies/hyper-rustls-0.27.10/LICENSE-APACHE), [`LICENSE-ISC`](dependencies/hyper-rustls-0.27.10/LICENSE-ISC), [`LICENSE-MIT`](dependencies/hyper-rustls-0.27.10/LICENSE-MIT) |
| `hyper-util` | `0.1.21` | `MIT` | [`LICENSE`](dependencies/hyper-util-0.1.21/LICENSE) |
| `iana-time-zone` | `0.1.65` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/iana-time-zone-0.1.65/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/iana-time-zone-0.1.65/LICENSE-MIT) |
| `iana-time-zone-haiku` | `0.1.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/iana-time-zone-haiku-0.1.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/iana-time-zone-haiku-0.1.2/LICENSE-MIT) |
| `icu_collections` | `2.3.0` | `Unicode-3.0` | [`LICENSE`](dependencies/icu_collections-2.3.0/LICENSE) |
| `icu_locale_core` | `2.3.0` | `Unicode-3.0` | [`LICENSE`](dependencies/icu_locale_core-2.3.0/LICENSE) |
| `icu_normalizer` | `2.3.0` | `Unicode-3.0` | [`LICENSE`](dependencies/icu_normalizer-2.3.0/LICENSE) |
| `icu_normalizer_data` | `2.3.0` | `Unicode-3.0` | [`LICENSE`](dependencies/icu_normalizer_data-2.3.0/LICENSE) |
| `icu_properties` | `2.3.0` | `Unicode-3.0` | [`LICENSE`](dependencies/icu_properties-2.3.0/LICENSE) |
| `icu_properties_data` | `2.3.0` | `Unicode-3.0` | [`LICENSE`](dependencies/icu_properties_data-2.3.0/LICENSE) |
| `icu_provider` | `2.3.1` | `Unicode-3.0` | [`LICENSE`](dependencies/icu_provider-2.3.1/LICENSE) |
| `idna` | `1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/idna-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/idna-1.1.0/LICENSE-MIT) |
| `idna_adapter` | `1.2.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/idna_adapter-1.2.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/idna_adapter-1.2.2/LICENSE-MIT) |
| `ignore` | `0.4.33` | `Unlicense OR MIT` | [`COPYING`](dependencies/ignore-0.4.33/COPYING), [`LICENSE-MIT`](dependencies/ignore-0.4.33/LICENSE-MIT) |
| `image` | `0.25.10` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/image-0.25.10/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/image-0.25.10/LICENSE-MIT) |
| `image-webp` | `0.2.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/image-webp-0.2.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/image-webp-0.2.4/LICENSE-MIT) |
| `imagesize` | `0.13.0` | `MIT` | [`LICENSE`](dependencies/imagesize-0.13.0/LICENSE) |
| `imagesize` | `0.14.0` | `MIT` | [`LICENSE`](dependencies/imagesize-0.14.0/LICENSE) |
| `imap-proto` | `0.16.7` | `MIT OR Apache-2.0` | **No regular license file found** |
| `imgref` | `1.12.3` | `CC0-1.0 OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/imgref-1.12.3/LICENSE-APACHE), [`LICENSE-CC0`](dependencies/imgref-1.12.3/LICENSE-CC0) |
| `indexmap` | `2.14.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/indexmap-2.14.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/indexmap-2.14.2/LICENSE-MIT) |
| `inotify` | `0.11.5` | `ISC` | [`LICENSE`](dependencies/inotify-0.11.5/LICENSE) |
| `inotify-sys` | `0.1.8` | `ISC` | [`LICENSE`](dependencies/inotify-sys-0.1.8/LICENSE) |
| `inout` | `0.1.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/inout-0.1.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/inout-0.1.4/LICENSE-MIT) |
| `instant` | `0.1.13` | `BSD-3-Clause` | [`LICENSE`](dependencies/instant-0.1.13/LICENSE) |
| `interpolate_name` | `0.2.4` | `MIT` | [`LICENSE`](dependencies/interpolate_name-0.2.4/LICENSE) |
| `inventory` | `0.3.25` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/inventory-0.3.25/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/inventory-0.3.25/LICENSE-MIT) |
| `io-surface` | `0.16.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/io-surface-0.16.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/io-surface-0.16.1/LICENSE-MIT) |
| `ipnet` | `2.12.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ipnet-2.12.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ipnet-2.12.2/LICENSE-MIT) |
| `is-docker` | `0.2.0` | `MIT` | [`LICENSE`](dependencies/is-docker-0.2.0/LICENSE) |
| `is-wsl` | `0.4.0` | `MIT` | [`LICENSE`](dependencies/is-wsl-0.4.0/LICENSE) |
| `itertools` | `0.11.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/itertools-0.11.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/itertools-0.11.0/LICENSE-MIT) |
| `itertools` | `0.13.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/itertools-0.13.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/itertools-0.13.0/LICENSE-MIT) |
| `itertools` | `0.14.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/itertools-0.14.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/itertools-0.14.0/LICENSE-MIT) |
| `itoa` | `1.0.18` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/itoa-1.0.18/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/itoa-1.0.18/LICENSE-MIT) |
| `jni-sys` | `0.3.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/jni-sys-0.3.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/jni-sys-0.3.1/LICENSE-MIT) |
| `jni-sys` | `0.4.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/jni-sys-0.4.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/jni-sys-0.4.1/LICENSE-MIT) |
| `jni-sys-macros` | `0.4.1` | `MIT OR Apache-2.0` | **No regular license file found** |
| `jobserver` | `0.1.35` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/jobserver-0.1.35/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/jobserver-0.1.35/LICENSE-MIT) |
| `js-sys` | `0.3.106` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/js-sys-0.3.106/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/js-sys-0.3.106/LICENSE-MIT) |
| `json5` | `0.4.1` | `ISC` | **No regular license file found** |
| `keyring` | `3.6.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/keyring-3.6.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/keyring-3.6.3/LICENSE-MIT) |
| `khronos-egl` | `6.0.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/khronos-egl-6.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/khronos-egl-6.0.0/LICENSE-MIT) |
| `khronos_api` | `3.1.0` | `Apache-2.0` | **No regular license file found** |
| `kqueue` | `1.2.1` | `MIT` | [`LICENSE`](dependencies/kqueue-1.2.1/LICENSE) |
| `kqueue-sys` | `1.1.2` | `MIT` | [`LICENSE`](dependencies/kqueue-sys-1.1.2/LICENSE) |
| `kurbo` | `0.11.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/kurbo-0.11.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/kurbo-0.11.3/LICENSE-MIT) |
| `kurbo` | `0.13.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/kurbo-0.13.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/kurbo-0.13.1/LICENSE-MIT) |
| `lazy_static` | `1.5.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/lazy_static-1.5.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/lazy_static-1.5.1/LICENSE-MIT) |
| `leak` | `0.1.2` | `Apache-2.0 OR MIT` | **No regular license file found** |
| `leaky-cow` | `0.1.1` | `MIT / Apache-2.0` | **No regular license file found** |
| `lebe` | `0.5.3` | `BSD-3-Clause` | [`LICENSE-BSD-3-Clause`](dependencies/lebe-0.5.3/LICENSE-BSD-3-Clause) |
| `lettre` | `0.11.23` | `MIT` | [`LICENSE`](dependencies/lettre-0.11.23/LICENSE) |
| `libbz2-rs-sys` | `0.2.5` | `bzip2-1.0.6` | [`LICENSE`](dependencies/libbz2-rs-sys-0.2.5/LICENSE) |
| `libc` | `0.2.190` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/libc-0.2.190/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/libc-0.2.190/LICENSE-MIT) |
| `libdbus-sys` | `0.2.7` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/libdbus-sys-0.2.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/libdbus-sys-0.2.7/LICENSE-MIT) |
| `libfuzzer-sys` | `0.4.13` | `(MIT OR Apache-2.0) AND NCSA` | [`LICENSE-APACHE`](dependencies/libfuzzer-sys-0.4.13/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/libfuzzer-sys-0.4.13/LICENSE-MIT) |
| `libloading` | `0.8.9` | `ISC` | [`LICENSE`](dependencies/libloading-0.8.9/LICENSE) |
| `libm` | `0.2.16` | `MIT` | [`LICENSE.txt`](dependencies/libm-0.2.16/LICENSE.txt) |
| `libredox` | `0.1.25` | `MIT` | [`LICENSE`](dependencies/libredox-0.1.25/LICENSE) |
| `libsqlite3-sys` | `0.30.1` | `MIT` | [`LICENSE`](dependencies/libsqlite3-sys-0.30.1/LICENSE) |
| `linebender_resource_handle` | `0.1.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/linebender_resource_handle-0.1.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/linebender_resource_handle-0.1.1/LICENSE-MIT) |
| `link-section` | `0.19.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/link-section-0.19.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/link-section-0.19.3/LICENSE-MIT) |
| `linktime-proc-macro` | `0.2.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/linktime-proc-macro-0.2.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/linktime-proc-macro-0.2.3/LICENSE-MIT) |
| `linux-raw-sys` | `0.12.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/linux-raw-sys-0.12.1/LICENSE-APACHE), [`LICENSE-Apache-2.0_WITH_LLVM-exception`](dependencies/linux-raw-sys-0.12.1/LICENSE-Apache-2.0_WITH_LLVM-exception), [`LICENSE-MIT`](dependencies/linux-raw-sys-0.12.1/LICENSE-MIT) |
| `litemap` | `0.8.3` | `Unicode-3.0` | [`LICENSE`](dependencies/litemap-0.8.3/LICENSE) |
| `litrs` | `1.0.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/litrs-1.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/litrs-1.0.0/LICENSE-MIT) |
| `locale_config` | `0.3.0` | `MIT` | [`LICENSE`](dependencies/locale_config-0.3.0/LICENSE) |
| `lock_api` | `0.4.14` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/lock_api-0.4.14/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/lock_api-0.4.14/LICENSE-MIT) |
| `log` | `0.4.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/log-0.4.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/log-0.4.34/LICENSE-MIT) |
| `loop9` | `0.1.5` | `MIT` | [`LICENSE`](dependencies/loop9-0.1.5/LICENSE) |
| `lru-slab` | `0.1.3` | `MIT OR Apache-2.0 OR Zlib` | [`LICENSE-APACHE`](dependencies/lru-slab-0.1.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/lru-slab-0.1.3/LICENSE-MIT), [`LICENSE-ZLIB`](dependencies/lru-slab-0.1.3/LICENSE-ZLIB) |
| `lsp-types` | `0.97.0` | `MIT` | [`LICENSE`](dependencies/lsp-types-0.97.0/LICENSE) |
| `lyon` | `1.0.19` | `MIT OR Apache-2.0` | **No regular license file found** |
| `lyon_algorithms` | `1.0.21` | `MIT OR Apache-2.0` | **No regular license file found** |
| `lyon_geom` | `1.0.19` | `MIT OR Apache-2.0` | **No regular license file found** |
| `lyon_path` | `1.0.19` | `MIT OR Apache-2.0` | **No regular license file found** |
| `lyon_tessellation` | `1.0.22` | `MIT OR Apache-2.0` | **No regular license file found** |
| `mac` | `0.1.1` | `MIT/Apache-2.0` | **No regular license file found** |
| `mac-notification-sys` | `0.6.15` | `MIT/Apache-2.0` | **No regular license file found** |
| `mach2` | `0.5.0` | `BSD-2-Clause OR MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/mach2-0.5.0/LICENSE-APACHE), [`LICENSE-BSD`](dependencies/mach2-0.5.0/LICENSE-BSD), [`LICENSE-MIT`](dependencies/mach2-0.5.0/LICENSE-MIT) |
| `mail-parser` | `0.9.4` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/mail-parser-0.9.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/mail-parser-0.9.4/LICENSE-MIT) |
| `malloc_buf` | `0.0.6` | `MIT` | **No regular license file found** |
| `maplit` | `1.0.2` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/maplit-1.0.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/maplit-1.0.2/LICENSE-MIT) |
| `markdown` | `1.0.0` | `MIT` | [`license`](dependencies/markdown-1.0.0/license) |
| `markup5ever` | `0.12.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/markup5ever-0.12.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/markup5ever-0.12.1/LICENSE-MIT) |
| `markup5ever` | `0.39.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/markup5ever-0.39.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/markup5ever-0.39.0/LICENSE-MIT) |
| `markup5ever` | `0.40.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/markup5ever-0.40.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/markup5ever-0.40.0/LICENSE-MIT) |
| `markup5ever_rcdom` | `0.3.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/markup5ever_rcdom-0.3.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/markup5ever_rcdom-0.3.0/LICENSE-MIT) |
| `markup5ever_rcdom` | `0.39.0+unofficial` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/markup5ever_rcdom-0.39.0+unofficial/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/markup5ever_rcdom-0.39.0+unofficial/LICENSE-MIT) |
| `matchers` | `0.2.0` | `MIT` | [`LICENSE`](dependencies/matchers-0.2.0/LICENSE) |
| `maybe-rayon` | `0.1.1` | `MIT` | [`LICENSE`](dependencies/maybe-rayon-0.1.1/LICENSE) |
| `md-5` | `0.10.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/md-5-0.10.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/md-5-0.10.6/LICENSE-MIT) |
| `memchr` | `2.8.3` | `Unlicense OR MIT` | [`COPYING`](dependencies/memchr-2.8.3/COPYING), [`LICENSE-MIT`](dependencies/memchr-2.8.3/LICENSE-MIT) |
| `memmap2` | `0.9.11` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/memmap2-0.9.11/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/memmap2-0.9.11/LICENSE-MIT) |
| `memoffset` | `0.9.1` | `MIT` | [`LICENSE`](dependencies/memoffset-0.9.1/LICENSE) |
| `metal` | `0.33.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/metal-0.33.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/metal-0.33.0/LICENSE-MIT) |
| `mime` | `0.3.17` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/mime-0.3.17/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/mime-0.3.17/LICENSE-MIT) |
| `mime_guess` | `2.0.5` | `MIT` | [`LICENSE`](dependencies/mime_guess-2.0.5/LICENSE) |
| `minimal-lexical` | `0.2.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/minimal-lexical-0.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/minimal-lexical-0.2.1/LICENSE-MIT), [`LICENSE.md`](dependencies/minimal-lexical-0.2.1/LICENSE.md) |
| `miniz_oxide` | `0.8.9` | `MIT OR Zlib OR Apache-2.0` | [`LICENSE`](dependencies/miniz_oxide-0.8.9/LICENSE), [`LICENSE-APACHE.md`](dependencies/miniz_oxide-0.8.9/LICENSE-APACHE.md), [`LICENSE-MIT.md`](dependencies/miniz_oxide-0.8.9/LICENSE-MIT.md), [`LICENSE-ZLIB.md`](dependencies/miniz_oxide-0.8.9/LICENSE-ZLIB.md) |
| `miniz_oxide` | `0.9.1` | `MIT OR Zlib OR Apache-2.0` | [`LICENSE`](dependencies/miniz_oxide-0.9.1/LICENSE), [`LICENSE-APACHE.md`](dependencies/miniz_oxide-0.9.1/LICENSE-APACHE.md), [`LICENSE-MIT.md`](dependencies/miniz_oxide-0.9.1/LICENSE-MIT.md), [`LICENSE-ZLIB.md`](dependencies/miniz_oxide-0.9.1/LICENSE-ZLIB.md) |
| `mio` | `1.2.4` | `MIT` | [`LICENSE`](dependencies/mio-1.2.4/LICENSE) |
| `moxcms` | `0.8.1` | `BSD-3-Clause OR Apache-2.0` | [`LICENSE-APACHE.md`](dependencies/moxcms-0.8.1/LICENSE-APACHE.md), [`LICENSE.md`](dependencies/moxcms-0.8.1/LICENSE.md) |
| `multiversion_no_op` | `1.0.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/multiversion_no_op-1.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/multiversion_no_op-1.0.0/LICENSE-MIT) |
| `naga` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/naga-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/naga-29.0.4/LICENSE.MIT) |
| `native-tls` | `0.2.18` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/native-tls-0.2.18/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/native-tls-0.2.18/LICENSE-MIT) |
| `ndk-sys` | `0.6.0+11769913` | `MIT OR Apache-2.0` | **No regular license file found** |
| `new_debug_unreachable` | `1.0.6` | `MIT` | [`LICENSE-MIT`](dependencies/new_debug_unreachable-1.0.6/LICENSE-MIT) |
| `nix` | `0.29.0` | `MIT` | [`LICENSE`](dependencies/nix-0.29.0/LICENSE) |
| `no_std_io2` | `0.9.4` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/no_std_io2-0.9.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/no_std_io2-0.9.4/LICENSE-MIT) |
| `nohash-hasher` | `0.2.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/nohash-hasher-0.2.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/nohash-hasher-0.2.0/LICENSE-MIT) |
| `nom` | `7.1.3` | `MIT` | [`LICENSE`](dependencies/nom-7.1.3/LICENSE) |
| `nom` | `8.0.0` | `MIT` | [`LICENSE`](dependencies/nom-8.0.0/LICENSE) |
| `noop_proc_macro` | `0.3.0` | `MIT` | [`LICENSE`](dependencies/noop_proc_macro-0.3.0/LICENSE) |
| `normpath` | `1.5.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/normpath-1.5.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/normpath-1.5.2/LICENSE-MIT), [`LICENSE-THIRD-PARTY`](dependencies/normpath-1.5.2/LICENSE-THIRD-PARTY) |
| `notify` | `8.2.0` | `CC0-1.0` | [`LICENSE-CC0`](dependencies/notify-8.2.0/LICENSE-CC0) |
| `notify-rust` | `4.18.2` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/notify-rust-4.18.2/LICENSE-Apache), [`LICENSE-MIT`](dependencies/notify-rust-4.18.2/LICENSE-MIT) |
| `notify-types` | `2.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/notify-types-2.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/notify-types-2.1.0/LICENSE-MIT) |
| `ntapi` | `0.4.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/ntapi-0.4.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ntapi-0.4.3/LICENSE-MIT) |
| `nu-ansi-term` | `0.50.3` | `MIT` | [`LICENSE`](dependencies/nu-ansi-term-0.50.3/LICENSE) |
| `num` | `0.4.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-0.4.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-0.4.3/LICENSE-MIT) |
| `num-bigint` | `0.4.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-bigint-0.4.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-bigint-0.4.8/LICENSE-MIT) |
| `num-bigint-dig` | `0.9.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-bigint-dig-0.9.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-bigint-dig-0.9.1/LICENSE-MIT) |
| `num-complex` | `0.4.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-complex-0.4.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-complex-0.4.6/LICENSE-MIT) |
| `num-conv` | `0.2.2` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/num-conv-0.2.2/LICENSE-Apache), [`LICENSE-MIT`](dependencies/num-conv-0.2.2/LICENSE-MIT) |
| `num-derive` | `0.4.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-derive-0.4.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-derive-0.4.2/LICENSE-MIT) |
| `num-integer` | `0.1.47` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-integer-0.1.47/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-integer-0.1.47/LICENSE-MIT) |
| `num-iter` | `0.1.46` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-iter-0.1.46/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-iter-0.1.46/LICENSE-MIT) |
| `num-rational` | `0.4.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-rational-0.4.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-rational-0.4.2/LICENSE-MIT) |
| `num-traits` | `0.2.19` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num-traits-0.2.19/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num-traits-0.2.19/LICENSE-MIT) |
| `num_cpus` | `1.17.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/num_cpus-1.17.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/num_cpus-1.17.0/LICENSE-MIT) |
| `objc` | `0.2.7` | `MIT` | [`LICENSE.txt`](dependencies/objc-0.2.7/LICENSE.txt) |
| `objc-foundation` | `0.1.1` | `MIT` | **No regular license file found** |
| `objc-sys` | `0.3.5` | `MIT` | **No regular license file found** |
| `objc2` | `0.5.3` | `MIT` | **No regular license file found** |
| `objc2` | `0.6.5` | `MIT` | **No regular license file found** |
| `objc2-app-kit` | `0.2.2` | `MIT` | **No regular license file found** |
| `objc2-app-kit` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-cloud-kit` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-audio` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-audio-types` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-data` | `0.2.2` | `MIT` | **No regular license file found** |
| `objc2-core-data` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-foundation` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-graphics` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-image` | `0.2.2` | `MIT` | **No regular license file found** |
| `objc2-core-image` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-location` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-media` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-text` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-core-video` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-encode` | `4.1.0` | `MIT` | **No regular license file found** |
| `objc2-foundation` | `0.2.2` | `MIT` | **No regular license file found** |
| `objc2-foundation` | `0.3.2` | `MIT` | **No regular license file found** |
| `objc2-io-surface` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-metal` | `0.2.2` | `MIT` | **No regular license file found** |
| `objc2-metal` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-quartz-core` | `0.2.2` | `MIT` | **No regular license file found** |
| `objc2-quartz-core` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-screen-capture-kit` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc2-user-notifications` | `0.3.2` | `Zlib OR Apache-2.0 OR MIT` | **No regular license file found** |
| `objc_exception` | `0.1.2` | `MIT` | **No regular license file found** |
| `objc_id` | `0.1.1` | `MIT` | **No regular license file found** |
| `object` | `0.37.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/object-0.37.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/object-0.37.3/LICENSE-MIT) |
| `object` | `0.39.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/object-0.39.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/object-0.39.1/LICENSE-MIT) |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/once_cell-1.21.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/once_cell-1.21.4/LICENSE-MIT) |
| `oo7` | `0.6.0` | `MIT` | [`LICENSE`](dependencies/oo7-0.6.0/LICENSE) |
| `open` | `5.4.4` | `MIT` | [`LICENSE.md`](dependencies/open-5.4.4/LICENSE.md) |
| `openssl` | `0.10.81` | `Apache-2.0` | [`LICENSE`](dependencies/openssl-0.10.81/LICENSE), [`LICENSE-APACHE`](dependencies/openssl-0.10.81/LICENSE-APACHE) |
| `openssl-macros` | `0.1.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/openssl-macros-0.1.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/openssl-macros-0.1.1/LICENSE-MIT) |
| `openssl-probe` | `0.2.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/openssl-probe-0.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/openssl-probe-0.2.1/LICENSE-MIT) |
| `openssl-sys` | `0.9.117` | `MIT` | [`LICENSE-MIT`](dependencies/openssl-sys-0.9.117/LICENSE-MIT) |
| `option-ext` | `0.2.0` | `MPL-2.0` | [`LICENSE.txt`](dependencies/option-ext-0.2.0/LICENSE.txt) |
| `ordered-float` | `5.5.0` | `MIT` | [`LICENSE-MIT`](dependencies/ordered-float-5.5.0/LICENSE-MIT) |
| `ordered-stream` | `0.2.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ordered-stream-0.2.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ordered-stream-0.2.0/LICENSE-MIT) |
| `parking` | `2.2.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/parking-2.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/parking-2.2.1/LICENSE-MIT), [`LICENSE-THIRD-PARTY`](dependencies/parking-2.2.1/LICENSE-THIRD-PARTY) |
| `parking_lot` | `0.12.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/parking_lot-0.12.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/parking_lot-0.12.5/LICENSE-MIT) |
| `parking_lot_core` | `0.9.12` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/parking_lot_core-0.9.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/parking_lot_core-0.9.12/LICENSE-MIT) |
| `paste` | `1.0.15` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/paste-1.0.15/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/paste-1.0.15/LICENSE-MIT) |
| `pastey` | `0.1.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pastey-0.1.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pastey-0.1.1/LICENSE-MIT) |
| `pathfinder_geometry` | `0.5.1` | `MIT/Apache-2.0` | **No regular license file found** |
| `pathfinder_simd` | `0.5.6` | `MIT OR Apache-2.0` | **No regular license file found** |
| `pbkdf2` | `0.12.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pbkdf2-0.12.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pbkdf2-0.12.2/LICENSE-MIT) |
| `percent-encoding` | `2.3.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/percent-encoding-2.3.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/percent-encoding-2.3.2/LICENSE-MIT) |
| `pest` | `2.9.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pest-2.9.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pest-2.9.2/LICENSE-MIT) |
| `pest_derive` | `2.9.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pest_derive-2.9.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pest_derive-2.9.2/LICENSE-MIT) |
| `pest_generator` | `2.9.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pest_generator-2.9.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pest_generator-2.9.2/LICENSE-MIT) |
| `pest_meta` | `2.9.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pest_meta-2.9.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pest_meta-2.9.2/LICENSE-MIT) |
| `phf` | `0.11.3` | `MIT` | [`LICENSE`](dependencies/phf-0.11.3/LICENSE) |
| `phf` | `0.13.1` | `MIT` | [`LICENSE`](dependencies/phf-0.13.1/LICENSE) |
| `phf` | `0.14.0` | `MIT` | [`LICENSE`](dependencies/phf-0.14.0/LICENSE) |
| `phf_codegen` | `0.11.3` | `MIT` | [`LICENSE`](dependencies/phf_codegen-0.11.3/LICENSE) |
| `phf_codegen` | `0.13.1` | `MIT` | [`LICENSE`](dependencies/phf_codegen-0.13.1/LICENSE) |
| `phf_codegen` | `0.14.0` | `MIT` | [`LICENSE`](dependencies/phf_codegen-0.14.0/LICENSE) |
| `phf_generator` | `0.11.3` | `MIT` | [`LICENSE`](dependencies/phf_generator-0.11.3/LICENSE) |
| `phf_generator` | `0.13.1` | `MIT` | [`LICENSE`](dependencies/phf_generator-0.13.1/LICENSE) |
| `phf_generator` | `0.14.0` | `MIT` | [`LICENSE`](dependencies/phf_generator-0.14.0/LICENSE) |
| `phf_macros` | `0.13.1` | `MIT` | [`LICENSE`](dependencies/phf_macros-0.13.1/LICENSE) |
| `phf_shared` | `0.11.3` | `MIT` | [`LICENSE`](dependencies/phf_shared-0.11.3/LICENSE) |
| `phf_shared` | `0.13.1` | `MIT` | [`LICENSE`](dependencies/phf_shared-0.13.1/LICENSE) |
| `phf_shared` | `0.14.0` | `MIT` | [`LICENSE`](dependencies/phf_shared-0.14.0/LICENSE) |
| `pico-args` | `0.5.0` | `MIT` | [`LICENSE`](dependencies/pico-args-0.5.0/LICENSE) |
| `pin-project` | `1.1.13` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/pin-project-1.1.13/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pin-project-1.1.13/LICENSE-MIT) |
| `pin-project-internal` | `1.1.13` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/pin-project-internal-1.1.13/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pin-project-internal-1.1.13/LICENSE-MIT) |
| `pin-project-lite` | `0.2.17` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/pin-project-lite-0.2.17/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pin-project-lite-0.2.17/LICENSE-MIT) |
| `pin-utils` | `0.1.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pin-utils-0.1.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pin-utils-0.1.1/LICENSE-MIT) |
| `piper` | `0.2.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/piper-0.2.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/piper-0.2.5/LICENSE-MIT) |
| `pkg-config` | `0.3.34` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/pkg-config-0.3.34/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pkg-config-0.3.34/LICENSE-MIT) |
| `plist` | `1.10.1` | `MIT` | **No regular license file found** |
| `png` | `0.17.16` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/png-0.17.16/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/png-0.17.16/LICENSE-MIT) |
| `png` | `0.18.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/png-0.18.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/png-0.18.1/LICENSE-MIT) |
| `polling` | `3.11.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/polling-3.11.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/polling-3.11.0/LICENSE-MIT) |
| `pollster` | `0.2.5` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/pollster-0.2.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pollster-0.2.5/LICENSE-MIT) |
| `pollster` | `0.4.0` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/pollster-0.4.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/pollster-0.4.0/LICENSE-MIT) |
| `polycool` | `0.4.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/polycool-0.4.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/polycool-0.4.0/LICENSE-MIT) |
| `portable-atomic` | `1.15.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/portable-atomic-1.15.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/portable-atomic-1.15.0/LICENSE-MIT) |
| `portable-atomic-util` | `0.2.8` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/portable-atomic-util-0.2.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/portable-atomic-util-0.2.8/LICENSE-MIT) |
| `postage` | `0.5.0` | `MIT` | [`LICENSE`](dependencies/postage-0.5.0/LICENSE) |
| `potential_utf` | `0.1.6` | `Unicode-3.0` | [`LICENSE`](dependencies/potential_utf-0.1.6/LICENSE) |
| `powerfmt` | `0.2.1` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/powerfmt-0.2.1/LICENSE-Apache), [`LICENSE-MIT`](dependencies/powerfmt-0.2.1/LICENSE-MIT) |
| `ppv-lite86` | `0.2.21` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ppv-lite86-0.2.21/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ppv-lite86-0.2.21/LICENSE-MIT) |
| `precomputed-hash` | `0.1.1` | `MIT` | [`LICENSE`](dependencies/precomputed-hash-0.1.1/LICENSE) |
| `presser` | `0.3.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/presser-0.3.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/presser-0.3.1/LICENSE-MIT) |
| `prettyplease` | `0.2.37` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/prettyplease-0.2.37/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/prettyplease-0.2.37/LICENSE-MIT) |
| `proc-macro-crate` | `3.5.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/proc-macro-crate-3.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/proc-macro-crate-3.5.0/LICENSE-MIT) |
| `proc-macro2` | `1.0.107` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/proc-macro2-1.0.107/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/proc-macro2-1.0.107/LICENSE-MIT) |
| `profiling` | `1.0.18` | `MIT OR Apache-2.0` | **No regular license file found** |
| `profiling-procmacros` | `1.0.18` | `MIT OR Apache-2.0` | **No regular license file found** |
| `psm` | `0.1.32` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/psm-0.1.32/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/psm-0.1.32/LICENSE-MIT) |
| `pulldown-cmark` | `0.13.4` | `MIT` | [`LICENSE`](dependencies/pulldown-cmark-0.13.4/LICENSE) |
| `pulldown-cmark-escape` | `0.11.0` | `MIT` | [`LICENSE`](dependencies/pulldown-cmark-escape-0.11.0/LICENSE) |
| `pulp` | `0.22.3` | `MIT` | [`LICENSE`](dependencies/pulp-0.22.3/LICENSE) |
| `pulp-wasm-simd-flag` | `0.1.1` | `MIT` | **No regular license file found** |
| `pxfm` | `0.1.30` | `BSD-3-Clause OR Apache-2.0` | [`LICENSE-APACHE.md`](dependencies/pxfm-0.1.30/LICENSE-APACHE.md), [`LICENSE.md`](dependencies/pxfm-0.1.30/LICENSE.md) |
| `qoi` | `0.4.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/qoi-0.4.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/qoi-0.4.1/LICENSE-MIT) |
| `quick-error` | `2.0.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/quick-error-2.0.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/quick-error-2.0.1/LICENSE-MIT) |
| `quick-xml` | `0.41.0` | `MIT` | [`LICENSE-MIT.md`](dependencies/quick-xml-0.41.0/LICENSE-MIT.md) |
| `quick-xml` | `0.42.0` | `MIT` | [`LICENSE-MIT.md`](dependencies/quick-xml-0.42.0/LICENSE-MIT.md) |
| `quinn` | `0.11.12` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/quinn-0.11.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/quinn-0.11.12/LICENSE-MIT) |
| `quinn-proto` | `0.11.19` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/quinn-proto-0.11.19/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/quinn-proto-0.11.19/LICENSE-MIT) |
| `quinn-udp` | `0.5.16` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/quinn-udp-0.5.16/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/quinn-udp-0.5.16/LICENSE-MIT) |
| `quote` | `1.0.47` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/quote-1.0.47/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/quote-1.0.47/LICENSE-MIT) |
| `quoted_printable` | `0.5.2` | `0BSD` | [`LICENSE`](dependencies/quoted_printable-0.5.2/LICENSE) |
| `r-efi` | `5.3.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | **No regular license file found** |
| `r-efi` | `6.0.0` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | **No regular license file found** |
| `rand` | `0.10.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand-0.10.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand-0.10.3/LICENSE-MIT) |
| `rand` | `0.8.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand-0.8.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand-0.8.8/LICENSE-MIT) |
| `rand` | `0.9.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand-0.9.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand-0.9.5/LICENSE-MIT) |
| `rand_chacha` | `0.3.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand_chacha-0.3.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand_chacha-0.3.1/LICENSE-MIT) |
| `rand_chacha` | `0.9.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand_chacha-0.9.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand_chacha-0.9.0/LICENSE-MIT) |
| `rand_core` | `0.10.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand_core-0.10.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand_core-0.10.1/LICENSE-MIT) |
| `rand_core` | `0.6.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand_core-0.6.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand_core-0.6.4/LICENSE-MIT) |
| `rand_core` | `0.9.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand_core-0.9.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand_core-0.9.5/LICENSE-MIT) |
| `rand_pcg` | `0.10.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rand_pcg-0.10.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rand_pcg-0.10.2/LICENSE-MIT) |
| `range-alloc` | `0.1.5` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/range-alloc-0.1.5/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/range-alloc-0.1.5/LICENSE.MIT) |
| `rangemap` | `1.8.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/rangemap-1.8.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rangemap-1.8.0/LICENSE-MIT) |
| `rav1e` | `0.8.1` | `BSD-2-Clause` | [`LICENSE`](dependencies/rav1e-0.8.1/LICENSE) |
| `ravif` | `0.13.0` | `BSD-3-Clause` | [`LICENSE`](dependencies/ravif-0.13.0/LICENSE) |
| `raw-cpuid` | `11.6.0` | `MIT` | [`LICENSE.md`](dependencies/raw-cpuid-11.6.0/LICENSE.md) |
| `raw-window-handle` | `0.6.2` | `MIT OR Apache-2.0 OR Zlib` | [`LICENSE-APACHE.md`](dependencies/raw-window-handle-0.6.2/LICENSE-APACHE.md), [`LICENSE-MIT.md`](dependencies/raw-window-handle-0.6.2/LICENSE-MIT.md), [`LICENSE-ZLIB.md`](dependencies/raw-window-handle-0.6.2/LICENSE-ZLIB.md) |
| `raw-window-metal` | `1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/raw-window-metal-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/raw-window-metal-1.1.0/LICENSE-MIT) |
| `rayon` | `1.12.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rayon-1.12.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rayon-1.12.0/LICENSE-MIT) |
| `rayon-core` | `1.13.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rayon-core-1.13.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rayon-core-1.13.0/LICENSE-MIT) |
| `read-fonts` | `0.37.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/read-fonts-0.37.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/read-fonts-0.37.0/LICENSE-MIT) |
| `read-fonts` | `0.41.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/read-fonts-0.41.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/read-fonts-0.41.0/LICENSE-MIT) |
| `reborrow` | `0.5.5` | `MIT` | [`LICENSE`](dependencies/reborrow-0.5.5/LICENSE) |
| `redox_syscall` | `0.5.18` | `MIT` | [`LICENSE`](dependencies/redox_syscall-0.5.18/LICENSE) |
| `redox_users` | `0.4.6` | `MIT` | [`LICENSE`](dependencies/redox_users-0.4.6/LICENSE) |
| `redox_users` | `0.5.3` | `MIT` | [`LICENSE`](dependencies/redox_users-0.5.3/LICENSE) |
| `ref-cast` | `1.0.27` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ref-cast-1.0.27/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ref-cast-1.0.27/LICENSE-MIT) |
| `ref-cast-impl` | `1.0.27` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ref-cast-impl-1.0.27/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ref-cast-impl-1.0.27/LICENSE-MIT) |
| `regex` | `1.13.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/regex-1.13.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/regex-1.13.1/LICENSE-MIT) |
| `regex-automata` | `0.4.18` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/regex-automata-0.4.18/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/regex-automata-0.4.18/LICENSE-MIT) |
| `regex-syntax` | `0.8.11` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/regex-syntax-0.8.11/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/regex-syntax-0.8.11/LICENSE-MIT) |
| `renderdoc-sys` | `1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/renderdoc-sys-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/renderdoc-sys-1.1.0/LICENSE-MIT) |
| `resvg` | `0.45.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/resvg-0.45.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/resvg-0.45.1/LICENSE-MIT) |
| `resvg` | `0.46.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/resvg-0.46.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/resvg-0.46.0/LICENSE-MIT) |
| `rfc2047-decoder` | `1.1.2` | `MIT` | [`LICENSE`](dependencies/rfc2047-decoder-1.1.2/LICENSE) |
| `rgb` | `0.8.53` | `MIT` | [`LICENSE`](dependencies/rgb-0.8.53/LICENSE) |
| `ring` | `0.17.14` | `Apache-2.0 AND ISC` | [`LICENSE`](dependencies/ring-0.17.14/LICENSE), [`LICENSE-BoringSSL`](dependencies/ring-0.17.14/LICENSE-BoringSSL), [`LICENSE-other-bits`](dependencies/ring-0.17.14/LICENSE-other-bits) |
| `ropey` | `2.0.0-beta.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ropey-2.0.0-beta.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ropey-2.0.0-beta.1/LICENSE-MIT) |
| `roxmltree` | `0.20.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/roxmltree-0.20.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/roxmltree-0.20.0/LICENSE-MIT) |
| `roxmltree` | `0.21.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/roxmltree-0.21.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/roxmltree-0.21.1/LICENSE-MIT) |
| `rusqlite` | `0.32.1` | `MIT` | [`LICENSE`](dependencies/rusqlite-0.32.1/LICENSE) |
| `rust-embed` | `8.13.0` | `MIT` | [`LICENSE`](dependencies/rust-embed-8.13.0/LICENSE) |
| `rust-embed-impl` | `8.13.0` | `MIT` | [`LICENSE`](dependencies/rust-embed-impl-8.13.0/LICENSE) |
| `rust-embed-utils` | `8.13.0` | `MIT` | [`LICENSE`](dependencies/rust-embed-utils-8.13.0/LICENSE) |
| `rust-i18n` | `4.2.4` | `MIT` | [`LICENSE`](dependencies/rust-i18n-4.2.4/LICENSE) |
| `rust-i18n-macro` | `4.2.4` | `MIT` | **No regular license file found** |
| `rust-i18n-support` | `4.2.4` | `MIT` | **No regular license file found** |
| `rustc-demangle` | `0.1.28` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/rustc-demangle-0.1.28/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rustc-demangle-0.1.28/LICENSE-MIT) |
| `rustc-hash` | `1.1.0` | `Apache-2.0/MIT` | [`LICENSE-APACHE`](dependencies/rustc-hash-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rustc-hash-1.1.0/LICENSE-MIT) |
| `rustc-hash` | `2.1.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/rustc-hash-2.1.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rustc-hash-2.1.3/LICENSE-MIT) |
| `rustc_version` | `0.4.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rustc_version-0.4.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rustc_version-0.4.1/LICENSE-MIT) |
| `rustix` | `1.1.5` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/rustix-1.1.5/LICENSE-APACHE), [`LICENSE-Apache-2.0_WITH_LLVM-exception`](dependencies/rustix-1.1.5/LICENSE-Apache-2.0_WITH_LLVM-exception), [`LICENSE-MIT`](dependencies/rustix-1.1.5/LICENSE-MIT) |
| `rustls` | `0.23.45` | `Apache-2.0 OR ISC OR MIT` | [`LICENSE-APACHE`](dependencies/rustls-0.23.45/LICENSE-APACHE), [`LICENSE-ISC`](dependencies/rustls-0.23.45/LICENSE-ISC), [`LICENSE-MIT`](dependencies/rustls-0.23.45/LICENSE-MIT) |
| `rustls-native-certs` | `0.8.4` | `Apache-2.0 OR ISC OR MIT` | [`LICENSE`](dependencies/rustls-native-certs-0.8.4/LICENSE), [`LICENSE-APACHE`](dependencies/rustls-native-certs-0.8.4/LICENSE-APACHE), [`LICENSE-ISC`](dependencies/rustls-native-certs-0.8.4/LICENSE-ISC), [`LICENSE-MIT`](dependencies/rustls-native-certs-0.8.4/LICENSE-MIT) |
| `rustls-pemfile` | `2.2.0` | `Apache-2.0 OR ISC OR MIT` | [`LICENSE`](dependencies/rustls-pemfile-2.2.0/LICENSE), [`LICENSE-APACHE`](dependencies/rustls-pemfile-2.2.0/LICENSE-APACHE), [`LICENSE-ISC`](dependencies/rustls-pemfile-2.2.0/LICENSE-ISC), [`LICENSE-MIT`](dependencies/rustls-pemfile-2.2.0/LICENSE-MIT) |
| `rustls-pki-types` | `1.15.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rustls-pki-types-1.15.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rustls-pki-types-1.15.1/LICENSE-MIT) |
| `rustls-webpki` | `0.103.15` | `ISC` | [`LICENSE`](dependencies/rustls-webpki-0.103.15/LICENSE) |
| `rustversion` | `1.0.23` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/rustversion-1.0.23/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/rustversion-1.0.23/LICENSE-MIT) |
| `rustybuzz` | `0.20.1` | `MIT` | [`LICENSE`](dependencies/rustybuzz-0.20.1/LICENSE) |
| `ryu` | `1.0.23` | `Apache-2.0 OR BSL-1.0` | [`LICENSE-APACHE`](dependencies/ryu-1.0.23/LICENSE-APACHE), [`LICENSE-BOOST`](dependencies/ryu-1.0.23/LICENSE-BOOST) |
| `same-file` | `1.0.6` | `Unlicense/MIT` | [`COPYING`](dependencies/same-file-1.0.6/COPYING), [`LICENSE-MIT`](dependencies/same-file-1.0.6/LICENSE-MIT) |
| `schannel` | `0.1.29` | `MIT` | [`LICENSE.md`](dependencies/schannel-0.1.29/LICENSE.md) |
| `schemars` | `1.2.2` | `MIT` | [`LICENSE`](dependencies/schemars-1.2.2/LICENSE) |
| `schemars_derive` | `1.2.2` | `MIT` | [`LICENSE`](dependencies/schemars_derive-1.2.2/LICENSE) |
| `scoped-tls` | `1.0.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/scoped-tls-1.0.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/scoped-tls-1.0.1/LICENSE-MIT) |
| `scopeguard` | `1.2.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/scopeguard-1.2.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/scopeguard-1.2.0/LICENSE-MIT) |
| `screencapturekit` | `0.2.8` | `MIT OR Apache-2.0` | **No regular license file found** |
| `screencapturekit-sys` | `0.2.8` | `MIT OR Apache-2.0` | **No regular license file found** |
| `seahash` | `4.1.0` | `MIT` | **No regular license file found** |
| `secret-service` | `4.0.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/secret-service-4.0.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/secret-service-4.0.0/LICENSE-MIT) |
| `security-framework` | `3.7.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/security-framework-3.7.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/security-framework-3.7.0/LICENSE-MIT) |
| `security-framework-sys` | `2.17.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/security-framework-sys-2.17.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/security-framework-sys-2.17.0/LICENSE-MIT) |
| `selectors` | `0.40.0` | `MPL-2.0` | **No regular license file found** |
| `self_cell` | `1.3.0` | `Apache-2.0 OR GPL-2.0-only` | [`LICENSE-APACHE`](dependencies/self_cell-1.3.0/LICENSE-APACHE), [`LICENSE-GPLv2`](dependencies/self_cell-1.3.0/LICENSE-GPLv2) |
| `semver` | `1.0.28` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/semver-1.0.28/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/semver-1.0.28/LICENSE-MIT) |
| `serde` | `1.0.229` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde-1.0.229/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde-1.0.229/LICENSE-MIT) |
| `serde-saphyr` | `1.3.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde-saphyr-1.3.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde-saphyr-1.3.0/LICENSE-MIT) |
| `serde_bytes` | `0.11.19` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_bytes-0.11.19/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_bytes-0.11.19/LICENSE-MIT) |
| `serde_core` | `1.0.229` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_core-1.0.229/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_core-1.0.229/LICENSE-MIT) |
| `serde_derive` | `1.0.229` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_derive-1.0.229/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_derive-1.0.229/LICENSE-MIT) |
| `serde_derive_internals` | `0.30.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_derive_internals-0.30.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_derive_internals-0.30.0/LICENSE-MIT) |
| `serde_fmt` | `1.1.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/serde_fmt-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_fmt-1.1.0/LICENSE-MIT) |
| `serde_json` | `1.0.151` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_json-1.0.151/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_json-1.0.151/LICENSE-MIT) |
| `serde_repr` | `0.1.21` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_repr-0.1.21/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_repr-0.1.21/LICENSE-MIT) |
| `serde_spanned` | `0.6.9` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_spanned-0.6.9/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_spanned-0.6.9/LICENSE-MIT) |
| `serde_spanned` | `1.1.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_spanned-1.1.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_spanned-1.1.2/LICENSE-MIT) |
| `serde_urlencoded` | `0.7.1` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/serde_urlencoded-0.7.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/serde_urlencoded-0.7.1/LICENSE-MIT) |
| `servo_arc` | `0.4.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/servo_arc-0.4.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/servo_arc-0.4.3/LICENSE-MIT) |
| `sha1` | `0.10.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/sha1-0.10.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sha1-0.10.7/LICENSE-MIT) |
| `sha1_smol` | `1.0.1` | `BSD-3-Clause` | [`LICENSE`](dependencies/sha1_smol-1.0.1/LICENSE) |
| `sha2` | `0.10.9` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/sha2-0.10.9/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sha2-0.10.9/LICENSE-MIT) |
| `sha2` | `0.11.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/sha2-0.11.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sha2-0.11.0/LICENSE-MIT) |
| `sharded-slab` | `0.1.7` | `MIT` | [`LICENSE`](dependencies/sharded-slab-0.1.7/LICENSE) |
| `shellexpand` | `3.1.2` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/shellexpand-3.1.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/shellexpand-3.1.2/LICENSE-MIT) |
| `shlex` | `1.3.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/shlex-1.3.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/shlex-1.3.0/LICENSE-MIT) |
| `shlex` | `2.0.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/shlex-2.0.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/shlex-2.0.1/LICENSE-MIT) |
| `signal-hook-registry` | `1.4.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/signal-hook-registry-1.4.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/signal-hook-registry-1.4.8/LICENSE-MIT) |
| `simd-adler32` | `0.3.10` | `MIT` | [`LICENSE.md`](dependencies/simd-adler32-0.3.10/LICENSE.md) |
| `simd_helpers` | `0.1.0` | `MIT` | **No regular license file found** |
| `simdutf8` | `0.1.5` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/simdutf8-0.1.5/LICENSE-Apache), [`LICENSE-MIT`](dependencies/simdutf8-0.1.5/LICENSE-MIT) |
| `simplecss` | `0.2.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/simplecss-0.2.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/simplecss-0.2.2/LICENSE-MIT) |
| `siphasher` | `1.0.4` | `MIT OR Apache-2.0` | [`COPYING`](dependencies/siphasher-1.0.4/COPYING), [`LICENSE-APACHE`](dependencies/siphasher-1.0.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/siphasher-1.0.4/LICENSE-MIT) |
| `skrifa` | `0.40.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/skrifa-0.40.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/skrifa-0.40.0/LICENSE-MIT) |
| `skrifa` | `0.44.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/skrifa-0.44.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/skrifa-0.44.0/LICENSE-MIT) |
| `slab` | `0.4.12` | `MIT` | [`LICENSE`](dependencies/slab-0.4.12/LICENSE) |
| `slotmap` | `1.1.1` | `Zlib` | [`LICENSE`](dependencies/slotmap-1.1.1/LICENSE) |
| `smallvec` | `1.16.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/smallvec-1.16.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/smallvec-1.16.2/LICENSE-MIT) |
| `smol` | `2.0.2` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/smol-2.0.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/smol-2.0.2/LICENSE-MIT) |
| `smol_str` | `0.3.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/smol_str-0.3.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/smol_str-0.3.6/LICENSE-MIT) |
| `socket2` | `0.6.5` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/socket2-0.6.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/socket2-0.6.5/LICENSE-MIT) |
| `spin` | `0.10.1` | `MIT` | [`LICENSE`](dependencies/spin-0.10.1/LICENSE) |
| `spin` | `0.9.9` | `MIT` | [`LICENSE`](dependencies/spin-0.9.9/LICENSE) |
| `spirv` | `0.4.0+sdk-1.4.341.0` | `Apache-2.0` | **No regular license file found** |
| `stable_deref_trait` | `1.2.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/stable_deref_trait-1.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/stable_deref_trait-1.2.1/LICENSE-MIT) |
| `stacker` | `0.1.25` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/stacker-0.1.25/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/stacker-0.1.25/LICENSE-MIT) |
| `static_assertions` | `1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/static_assertions-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/static_assertions-1.1.0/LICENSE-MIT) |
| `stop-token` | `0.7.0` | `MIT OR Apache-2.0` | **No regular license file found** |
| `str_indices` | `0.4.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/str_indices-0.4.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/str_indices-0.4.4/LICENSE-MIT) |
| `strict-num` | `0.1.1` | `MIT` | [`LICENSE`](dependencies/strict-num-0.1.1/LICENSE) |
| `string_cache` | `0.11.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/string_cache-0.11.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/string_cache-0.11.0/LICENSE-MIT) |
| `string_cache` | `0.8.9` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/string_cache-0.8.9/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/string_cache-0.8.9/LICENSE-MIT) |
| `string_cache` | `0.9.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/string_cache-0.9.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/string_cache-0.9.0/LICENSE-MIT) |
| `string_cache_codegen` | `0.11.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/string_cache_codegen-0.11.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/string_cache_codegen-0.11.2/LICENSE-MIT) |
| `string_cache_codegen` | `0.5.4` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/string_cache_codegen-0.5.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/string_cache_codegen-0.5.4/LICENSE-MIT) |
| `string_cache_codegen` | `0.6.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/string_cache_codegen-0.6.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/string_cache_codegen-0.6.1/LICENSE-MIT) |
| `strum` | `0.28.0` | `MIT` | [`LICENSE`](dependencies/strum-0.28.0/LICENSE) |
| `strum_macros` | `0.28.0` | `MIT` | [`LICENSE`](dependencies/strum_macros-0.28.0/LICENSE) |
| `subtle` | `2.6.1` | `BSD-3-Clause` | [`LICENSE`](dependencies/subtle-2.6.1/LICENSE) |
| `sval` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval-2.22.0/LICENSE-MIT) |
| `sval_buffer` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval_buffer-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval_buffer-2.22.0/LICENSE-MIT) |
| `sval_dynamic` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval_dynamic-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval_dynamic-2.22.0/LICENSE-MIT) |
| `sval_fmt` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval_fmt-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval_fmt-2.22.0/LICENSE-MIT) |
| `sval_json` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval_json-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval_json-2.22.0/LICENSE-MIT) |
| `sval_nested` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval_nested-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval_nested-2.22.0/LICENSE-MIT) |
| `sval_ref` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval_ref-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval_ref-2.22.0/LICENSE-MIT) |
| `sval_serde` | `2.22.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/sval_serde-2.22.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sval_serde-2.22.0/LICENSE-MIT) |
| `svg_fmt` | `0.4.5` | `MIT/Apache-2.0` | **No regular license file found** |
| `svgtypes` | `0.15.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/svgtypes-0.15.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/svgtypes-0.15.3/LICENSE-MIT) |
| `svgtypes` | `0.16.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/svgtypes-0.16.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/svgtypes-0.16.1/LICENSE-MIT) |
| `swash` | `0.2.10` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/swash-0.2.10/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/swash-0.2.10/LICENSE-MIT) |
| `syn` | `2.0.119` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/syn-2.0.119/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/syn-2.0.119/LICENSE-MIT) |
| `syn` | `3.0.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/syn-3.0.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/syn-3.0.6/LICENSE-MIT) |
| `sync_wrapper` | `1.0.2` | `Apache-2.0` | [`LICENSE`](dependencies/sync_wrapper-1.0.2/LICENSE) |
| `synstructure` | `0.14.0` | `MIT` | [`LICENSE`](dependencies/synstructure-0.14.0/LICENSE) |
| `sys-locale` | `0.3.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/sys-locale-0.3.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/sys-locale-0.3.2/LICENSE-MIT) |
| `sysinfo` | `0.31.4` | `MIT` | [`LICENSE`](dependencies/sysinfo-0.31.4/LICENSE) |
| `system-configuration` | `0.6.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/system-configuration-0.6.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/system-configuration-0.6.1/LICENSE-MIT) |
| `system-configuration-sys` | `0.6.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/system-configuration-sys-0.6.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/system-configuration-sys-0.6.0/LICENSE-MIT) |
| `system-deps` | `7.0.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/system-deps-7.0.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/system-deps-7.0.8/LICENSE-MIT) |
| `taffy` | `0.13.0` | `MIT` | **No regular license file found** |
| `tao-core-video-sys` | `0.2.0` | `MIT` | [`LICENSE`](dependencies/tao-core-video-sys-0.2.0/LICENSE) |
| `target-lexicon` | `0.13.5` | `Apache-2.0 WITH LLVM-exception` | [`LICENSE`](dependencies/target-lexicon-0.13.5/LICENSE) |
| `tauri-winrt-notification` | `0.8.1` | `MIT OR Apache-2.0` | [`LICENSE.spdx`](dependencies/tauri-winrt-notification-0.8.1/LICENSE.spdx), [`LICENSE_APACHE-2.0`](dependencies/tauri-winrt-notification-0.8.1/LICENSE_APACHE-2.0), [`LICENSE_MIT`](dependencies/tauri-winrt-notification-0.8.1/LICENSE_MIT) |
| `temp-dir` | `0.1.16` | `Apache-2.0` | [`LICENSE`](dependencies/temp-dir-0.1.16/LICENSE) |
| `tempfile` | `3.27.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/tempfile-3.27.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/tempfile-3.27.0/LICENSE-MIT) |
| `tendril` | `0.4.3` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/tendril-0.4.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/tendril-0.4.3/LICENSE-MIT) |
| `tendril` | `0.5.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/tendril-0.5.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/tendril-0.5.1/LICENSE-MIT) |
| `termcolor` | `1.4.1` | `Unlicense OR MIT` | [`COPYING`](dependencies/termcolor-1.4.1/COPYING), [`LICENSE-MIT`](dependencies/termcolor-1.4.1/LICENSE-MIT) |
| `thiserror` | `1.0.69` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/thiserror-1.0.69/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/thiserror-1.0.69/LICENSE-MIT) |
| `thiserror` | `2.0.21` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/thiserror-2.0.21/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/thiserror-2.0.21/LICENSE-MIT) |
| `thiserror-impl` | `1.0.69` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/thiserror-impl-1.0.69/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/thiserror-impl-1.0.69/LICENSE-MIT) |
| `thiserror-impl` | `2.0.21` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/thiserror-impl-2.0.21/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/thiserror-impl-2.0.21/LICENSE-MIT) |
| `thread_local` | `1.1.10` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/thread_local-1.1.10/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/thread_local-1.1.10/LICENSE-MIT) |
| `tiff` | `0.11.3` | `MIT` | [`LICENSE`](dependencies/tiff-0.11.3/LICENSE) |
| `time` | `0.3.55` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/time-0.3.55/LICENSE-Apache), [`LICENSE-MIT`](dependencies/time-0.3.55/LICENSE-MIT) |
| `time-core` | `0.1.9` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/time-core-0.1.9/LICENSE-Apache), [`LICENSE-MIT`](dependencies/time-core-0.1.9/LICENSE-MIT) |
| `time-macros` | `0.2.32` | `MIT OR Apache-2.0` | [`LICENSE-Apache`](dependencies/time-macros-0.2.32/LICENSE-Apache), [`LICENSE-MIT`](dependencies/time-macros-0.2.32/LICENSE-MIT) |
| `tiny-keccak` | `2.0.2` | `CC0-1.0` | [`LICENSE`](dependencies/tiny-keccak-2.0.2/LICENSE) |
| `tiny-skia` | `0.11.4` | `BSD-3-Clause` | [`LICENSE`](dependencies/tiny-skia-0.11.4/LICENSE) |
| `tiny-skia-path` | `0.11.4` | `BSD-3-Clause` | [`LICENSE`](dependencies/tiny-skia-path-0.11.4/LICENSE) |
| `tinystr` | `0.8.4` | `Unicode-3.0` | [`LICENSE`](dependencies/tinystr-0.8.4/LICENSE) |
| `tinyvec` | `1.13.3` | `Zlib OR Apache-2.0 OR MIT` | [`LICENSE-APACHE.md`](dependencies/tinyvec-1.13.3/LICENSE-APACHE.md), [`LICENSE-MIT.md`](dependencies/tinyvec-1.13.3/LICENSE-MIT.md), [`LICENSE-ZLIB.md`](dependencies/tinyvec-1.13.3/LICENSE-ZLIB.md) |
| `tokio` | `1.53.2` | `MIT` | [`LICENSE`](dependencies/tokio-1.53.2/LICENSE) |
| `tokio-macros` | `2.7.2` | `MIT` | [`LICENSE`](dependencies/tokio-macros-2.7.2/LICENSE) |
| `tokio-native-tls` | `0.3.1` | `MIT` | [`LICENSE`](dependencies/tokio-native-tls-0.3.1/LICENSE) |
| `tokio-rustls` | `0.26.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/tokio-rustls-0.26.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/tokio-rustls-0.26.6/LICENSE-MIT) |
| `tokio-socks` | `0.5.3` | `MIT` | [`LICENSE`](dependencies/tokio-socks-0.5.3/LICENSE) |
| `tokio-util` | `0.7.19` | `MIT` | [`LICENSE`](dependencies/tokio-util-0.7.19/LICENSE) |
| `toml` | `0.8.23` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml-0.8.23/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml-0.8.23/LICENSE-MIT) |
| `toml` | `1.1.7+spec-1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml-1.1.7+spec-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml-1.1.7+spec-1.1.0/LICENSE-MIT) |
| `toml_datetime` | `0.6.11` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml_datetime-0.6.11/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml_datetime-0.6.11/LICENSE-MIT) |
| `toml_datetime` | `1.1.2+spec-1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml_datetime-1.1.2+spec-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml_datetime-1.1.2+spec-1.1.0/LICENSE-MIT) |
| `toml_edit` | `0.22.27` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml_edit-0.22.27/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml_edit-0.22.27/LICENSE-MIT) |
| `toml_edit` | `0.25.16+spec-1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml_edit-0.25.16+spec-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml_edit-0.25.16+spec-1.1.0/LICENSE-MIT) |
| `toml_parser` | `1.1.4+spec-1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml_parser-1.1.4+spec-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml_parser-1.1.4+spec-1.1.0/LICENSE-MIT) |
| `toml_write` | `0.1.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml_write-0.1.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml_write-0.1.2/LICENSE-MIT) |
| `toml_writer` | `1.1.3+spec-1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/toml_writer-1.1.3+spec-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/toml_writer-1.1.3+spec-1.1.0/LICENSE-MIT) |
| `tower` | `0.5.3` | `MIT` | [`LICENSE`](dependencies/tower-0.5.3/LICENSE) |
| `tower-layer` | `0.3.3` | `MIT` | [`LICENSE`](dependencies/tower-layer-0.3.3/LICENSE) |
| `tower-service` | `0.3.3` | `MIT` | [`LICENSE`](dependencies/tower-service-0.3.3/LICENSE) |
| `tracing` | `0.1.44` | `MIT` | [`LICENSE`](dependencies/tracing-0.1.44/LICENSE) |
| `tracing-attributes` | `0.1.31` | `MIT` | [`LICENSE`](dependencies/tracing-attributes-0.1.31/LICENSE) |
| `tracing-core` | `0.1.36` | `MIT` | [`LICENSE`](dependencies/tracing-core-0.1.36/LICENSE) |
| `tracing-log` | `0.2.0` | `MIT` | [`LICENSE`](dependencies/tracing-log-0.2.0/LICENSE) |
| `tracing-subscriber` | `0.3.23` | `MIT` | [`LICENSE`](dependencies/tracing-subscriber-0.3.23/LICENSE) |
| `triomphe` | `0.1.16` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/triomphe-0.1.16/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/triomphe-0.1.16/LICENSE-MIT) |
| `try-lock` | `0.2.5` | `MIT` | [`LICENSE`](dependencies/try-lock-0.2.5/LICENSE) |
| `ttf-parser` | `0.25.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ttf-parser-0.25.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ttf-parser-0.25.1/LICENSE-MIT) |
| `typeid` | `1.0.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/typeid-1.0.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/typeid-1.0.3/LICENSE-MIT) |
| `typenum` | `1.20.1` | `MIT OR Apache-2.0` | [`LICENSE`](dependencies/typenum-1.20.1/LICENSE), [`LICENSE-APACHE`](dependencies/typenum-1.20.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/typenum-1.20.1/LICENSE-MIT) |
| `ucd-trie` | `0.1.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ucd-trie-0.1.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ucd-trie-0.1.7/LICENSE-MIT) |
| `uds_windows` | `1.2.1` | `MIT` | [`LICENSE`](dependencies/uds_windows-1.2.1/LICENSE) |
| `unicase` | `2.10.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicase-2.10.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicase-2.10.0/LICENSE-MIT) |
| `unicode-bidi` | `0.3.18` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-bidi-0.3.18/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-bidi-0.3.18/LICENSE-MIT) |
| `unicode-bidi-mirroring` | `0.4.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-bidi-mirroring-0.4.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-bidi-mirroring-0.4.0/LICENSE-MIT) |
| `unicode-ccc` | `0.4.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-ccc-0.4.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-ccc-0.4.0/LICENSE-MIT) |
| `unicode-id` | `0.3.7` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-id-0.3.7/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-id-0.3.7/LICENSE-MIT) |
| `unicode-ident` | `1.0.26` | `(MIT OR Apache-2.0) AND Unicode-3.0` | [`LICENSE-APACHE`](dependencies/unicode-ident-1.0.26/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-ident-1.0.26/LICENSE-MIT), [`LICENSE-UNICODE`](dependencies/unicode-ident-1.0.26/LICENSE-UNICODE) |
| `unicode-linebreak` | `0.1.5` | `Apache-2.0` | [`LICENSE`](dependencies/unicode-linebreak-0.1.5/LICENSE) |
| `unicode-properties` | `0.1.4` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-properties-0.1.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-properties-0.1.4/LICENSE-MIT) |
| `unicode-script` | `0.5.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-script-0.5.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-script-0.5.8/LICENSE-MIT) |
| `unicode-segmentation` | `1.13.3` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-segmentation-1.13.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-segmentation-1.13.3/LICENSE-MIT) |
| `unicode-vo` | `0.1.0` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-vo-0.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-vo-0.1.0/LICENSE-MIT) |
| `unicode-width` | `0.2.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-width-0.2.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-width-0.2.2/LICENSE-MIT) |
| `unicode-xid` | `0.2.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/unicode-xid-0.2.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/unicode-xid-0.2.6/LICENSE-MIT) |
| `untrusted` | `0.9.0` | `ISC` | [`LICENSE.txt`](dependencies/untrusted-0.9.0/LICENSE.txt) |
| `ureq` | `2.12.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/ureq-2.12.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/ureq-2.12.1/LICENSE-MIT) |
| `url` | `2.5.8` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/url-2.5.8/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/url-2.5.8/LICENSE-MIT) |
| `usvg` | `0.45.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/usvg-0.45.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/usvg-0.45.1/LICENSE-MIT) |
| `usvg` | `0.46.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/usvg-0.46.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/usvg-0.46.0/LICENSE-MIT) |
| `utf-8` | `0.7.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/utf-8-0.7.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/utf-8-0.7.6/LICENSE-MIT) |
| `utf8_iter` | `1.0.4` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/utf8_iter-1.0.4/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/utf8_iter-1.0.4/LICENSE-MIT) |
| `uuid` | `1.27.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/uuid-1.27.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/uuid-1.27.0/LICENSE-MIT) |
| `v_frame` | `0.3.9` | `BSD-2-Clause` | [`LICENSE`](dependencies/v_frame-0.3.9/LICENSE) |
| `valuable` | `0.1.1` | `MIT` | **No regular license file found** |
| `value-bag` | `1.14.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/value-bag-1.14.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/value-bag-1.14.1/LICENSE-MIT) |
| `value-bag-serde1` | `1.14.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/value-bag-serde1-1.14.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/value-bag-serde1-1.14.1/LICENSE-MIT) |
| `value-bag-sval2` | `1.14.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/value-bag-sval2-1.14.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/value-bag-sval2-1.14.1/LICENSE-MIT) |
| `vcpkg` | `0.2.15` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/vcpkg-0.2.15/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/vcpkg-0.2.15/LICENSE-MIT) |
| `version-compare` | `0.2.1` | `MIT` | [`LICENSE`](dependencies/version-compare-0.2.1/LICENSE) |
| `version_check` | `0.9.5` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/version_check-0.9.5/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/version_check-0.9.5/LICENSE-MIT) |
| `vswhom` | `0.1.0` | `MIT` | [`LICENSE`](dependencies/vswhom-0.1.0/LICENSE) |
| `vswhom-sys` | `0.1.3` | `MIT` | [`LICENSE`](dependencies/vswhom-sys-0.1.3/LICENSE) |
| `waker-fn` | `1.2.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/waker-fn-1.2.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/waker-fn-1.2.0/LICENSE-MIT) |
| `walkdir` | `2.5.0` | `Unlicense/MIT` | [`COPYING`](dependencies/walkdir-2.5.0/COPYING), [`LICENSE-MIT`](dependencies/walkdir-2.5.0/LICENSE-MIT) |
| `want` | `0.3.2` | `MIT` | [`LICENSE`](dependencies/want-0.3.2/LICENSE) |
| `wasi` | `0.11.1+wasi-snapshot-preview1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-APACHE), [`LICENSE-Apache-2.0_WITH_LLVM-exception`](dependencies/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-Apache-2.0_WITH_LLVM-exception), [`LICENSE-MIT`](dependencies/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-MIT) |
| `wasip2` | `1.0.4+wasi-0.2.12` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/wasip2-1.0.4+wasi-0.2.12/LICENSE-APACHE), [`LICENSE-Apache-2.0_WITH_LLVM-exception`](dependencies/wasip2-1.0.4+wasi-0.2.12/LICENSE-Apache-2.0_WITH_LLVM-exception), [`LICENSE-MIT`](dependencies/wasip2-1.0.4+wasi-0.2.12/LICENSE-MIT) |
| `wasm-bindgen` | `0.2.129` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/wasm-bindgen-0.2.129/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wasm-bindgen-0.2.129/LICENSE-MIT) |
| `wasm-bindgen-futures` | `0.4.79` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/wasm-bindgen-futures-0.4.79/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wasm-bindgen-futures-0.4.79/LICENSE-MIT) |
| `wasm-bindgen-macro` | `0.2.129` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/wasm-bindgen-macro-0.2.129/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wasm-bindgen-macro-0.2.129/LICENSE-MIT) |
| `wasm-bindgen-macro-support` | `0.2.129` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/wasm-bindgen-macro-support-0.2.129/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wasm-bindgen-macro-support-0.2.129/LICENSE-MIT) |
| `wasm-bindgen-shared` | `0.2.129` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/wasm-bindgen-shared-0.2.129/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wasm-bindgen-shared-0.2.129/LICENSE-MIT) |
| `wasm-streams` | `0.4.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/wasm-streams-0.4.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wasm-streams-0.4.2/LICENSE-MIT) |
| `wasm_thread` | `0.3.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/wasm_thread-0.3.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wasm_thread-0.3.3/LICENSE-MIT) |
| `wayland-backend` | `0.3.17` | `MIT` | [`LICENSE.txt`](dependencies/wayland-backend-0.3.17/LICENSE.txt) |
| `wayland-client` | `0.31.15` | `MIT` | [`LICENSE.txt`](dependencies/wayland-client-0.31.15/LICENSE.txt) |
| `wayland-cursor` | `0.31.14` | `MIT` | [`LICENSE.txt`](dependencies/wayland-cursor-0.31.14/LICENSE.txt) |
| `wayland-protocols` | `0.32.13` | `MIT` | [`LICENSE.txt`](dependencies/wayland-protocols-0.32.13/LICENSE.txt) |
| `wayland-protocols-plasma` | `0.3.12` | `MIT` | [`LICENSE.txt`](dependencies/wayland-protocols-plasma-0.3.12/LICENSE.txt) |
| `wayland-protocols-wlr` | `0.3.12` | `MIT` | [`LICENSE.txt`](dependencies/wayland-protocols-wlr-0.3.12/LICENSE.txt) |
| `wayland-scanner` | `0.31.11` | `MIT` | [`LICENSE.txt`](dependencies/wayland-scanner-0.31.11/LICENSE.txt) |
| `wayland-sys` | `0.31.11` | `MIT` | [`LICENSE.txt`](dependencies/wayland-sys-0.31.11/LICENSE.txt) |
| `web-sys` | `0.3.106` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/web-sys-0.3.106/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/web-sys-0.3.106/LICENSE-MIT) |
| `web-time` | `1.1.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/web-time-1.1.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/web-time-1.1.0/LICENSE-MIT) |
| `web_atoms` | `0.2.6` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/web_atoms-0.2.6/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/web_atoms-0.2.6/LICENSE-MIT) |
| `web_atoms` | `0.3.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/web_atoms-0.3.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/web_atoms-0.3.0/LICENSE-MIT) |
| `webpki-roots` | `0.26.11` | `CDLA-Permissive-2.0` | [`LICENSE`](dependencies/webpki-roots-0.26.11/LICENSE) |
| `webpki-roots` | `1.0.9` | `CDLA-Permissive-2.0` | [`LICENSE`](dependencies/webpki-roots-1.0.9/LICENSE) |
| `weezl` | `0.1.12` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/weezl-0.1.12/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/weezl-0.1.12/LICENSE-MIT) |
| `wgpu` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-29.0.4/LICENSE.MIT) |
| `wgpu-core` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-core-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-core-29.0.4/LICENSE.MIT) |
| `wgpu-core-deps-apple` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-core-deps-apple-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-core-deps-apple-29.0.4/LICENSE.MIT) |
| `wgpu-core-deps-emscripten` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-core-deps-emscripten-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-core-deps-emscripten-29.0.4/LICENSE.MIT) |
| `wgpu-core-deps-wasm` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-core-deps-wasm-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-core-deps-wasm-29.0.4/LICENSE.MIT) |
| `wgpu-core-deps-windows-linux-android` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-core-deps-windows-linux-android-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-core-deps-windows-linux-android-29.0.4/LICENSE.MIT) |
| `wgpu-hal` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-hal-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-hal-29.0.4/LICENSE.MIT) |
| `wgpu-naga-bridge` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-naga-bridge-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-naga-bridge-29.0.4/LICENSE.MIT) |
| `wgpu-types` | `29.0.4` | `MIT OR Apache-2.0` | [`LICENSE.APACHE`](dependencies/wgpu-types-29.0.4/LICENSE.APACHE), [`LICENSE.MIT`](dependencies/wgpu-types-29.0.4/LICENSE.MIT) |
| `which` | `8.0.6` | `MIT` | [`LICENSE.txt`](dependencies/which-8.0.6/LICENSE.txt) |
| `winapi` | `0.3.9` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/winapi-0.3.9/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/winapi-0.3.9/LICENSE-MIT) |
| `winapi-i686-pc-windows-gnu` | `0.4.0` | `MIT/Apache-2.0` | **No regular license file found** |
| `winapi-util` | `0.1.11` | `Unlicense OR MIT` | [`COPYING`](dependencies/winapi-util-0.1.11/COPYING), [`LICENSE-MIT`](dependencies/winapi-util-0.1.11/LICENSE-MIT) |
| `winapi-x86_64-pc-windows-gnu` | `0.4.0` | `MIT/Apache-2.0` | **No regular license file found** |
| `windows` | `0.57.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-0.57.0/license-apache-2.0), [`license-mit`](dependencies/windows-0.57.0/license-mit) |
| `windows` | `0.58.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-0.58.0/license-apache-2.0), [`license-mit`](dependencies/windows-0.58.0/license-mit) |
| `windows` | `0.61.3` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-0.61.3/license-apache-2.0), [`license-mit`](dependencies/windows-0.61.3/license-mit) |
| `windows` | `0.62.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-0.62.2/license-apache-2.0), [`license-mit`](dependencies/windows-0.62.2/license-mit) |
| `windows-capture` | `1.5.0` | `MIT` | **No regular license file found** |
| `windows-collections` | `0.2.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-collections-0.2.0/license-apache-2.0), [`license-mit`](dependencies/windows-collections-0.2.0/license-mit) |
| `windows-collections` | `0.3.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-collections-0.3.2/license-apache-2.0), [`license-mit`](dependencies/windows-collections-0.3.2/license-mit) |
| `windows-core` | `0.57.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-core-0.57.0/license-apache-2.0), [`license-mit`](dependencies/windows-core-0.57.0/license-mit) |
| `windows-core` | `0.58.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-core-0.58.0/license-apache-2.0), [`license-mit`](dependencies/windows-core-0.58.0/license-mit) |
| `windows-core` | `0.61.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-core-0.61.2/license-apache-2.0), [`license-mit`](dependencies/windows-core-0.61.2/license-mit) |
| `windows-core` | `0.62.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-core-0.62.2/license-apache-2.0), [`license-mit`](dependencies/windows-core-0.62.2/license-mit) |
| `windows-future` | `0.2.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-future-0.2.1/license-apache-2.0), [`license-mit`](dependencies/windows-future-0.2.1/license-mit) |
| `windows-future` | `0.3.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-future-0.3.2/license-apache-2.0), [`license-mit`](dependencies/windows-future-0.3.2/license-mit) |
| `windows-implement` | `0.57.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-implement-0.57.0/license-apache-2.0), [`license-mit`](dependencies/windows-implement-0.57.0/license-mit) |
| `windows-implement` | `0.58.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-implement-0.58.0/license-apache-2.0), [`license-mit`](dependencies/windows-implement-0.58.0/license-mit) |
| `windows-implement` | `0.60.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-implement-0.60.2/license-apache-2.0), [`license-mit`](dependencies/windows-implement-0.60.2/license-mit) |
| `windows-interface` | `0.57.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-interface-0.57.0/license-apache-2.0), [`license-mit`](dependencies/windows-interface-0.57.0/license-mit) |
| `windows-interface` | `0.58.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-interface-0.58.0/license-apache-2.0), [`license-mit`](dependencies/windows-interface-0.58.0/license-mit) |
| `windows-interface` | `0.59.3` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-interface-0.59.3/license-apache-2.0), [`license-mit`](dependencies/windows-interface-0.59.3/license-mit) |
| `windows-link` | `0.1.3` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-link-0.1.3/license-apache-2.0), [`license-mit`](dependencies/windows-link-0.1.3/license-mit) |
| `windows-link` | `0.2.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-link-0.2.1/license-apache-2.0), [`license-mit`](dependencies/windows-link-0.2.1/license-mit) |
| `windows-numerics` | `0.2.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-numerics-0.2.0/license-apache-2.0), [`license-mit`](dependencies/windows-numerics-0.2.0/license-mit) |
| `windows-numerics` | `0.3.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-numerics-0.3.1/license-apache-2.0), [`license-mit`](dependencies/windows-numerics-0.3.1/license-mit) |
| `windows-registry` | `0.4.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-registry-0.4.0/license-apache-2.0), [`license-mit`](dependencies/windows-registry-0.4.0/license-mit) |
| `windows-registry` | `0.6.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-registry-0.6.1/license-apache-2.0), [`license-mit`](dependencies/windows-registry-0.6.1/license-mit) |
| `windows-result` | `0.1.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-result-0.1.2/license-apache-2.0), [`license-mit`](dependencies/windows-result-0.1.2/license-mit) |
| `windows-result` | `0.2.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-result-0.2.0/license-apache-2.0), [`license-mit`](dependencies/windows-result-0.2.0/license-mit) |
| `windows-result` | `0.3.4` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-result-0.3.4/license-apache-2.0), [`license-mit`](dependencies/windows-result-0.3.4/license-mit) |
| `windows-result` | `0.4.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-result-0.4.1/license-apache-2.0), [`license-mit`](dependencies/windows-result-0.4.1/license-mit) |
| `windows-strings` | `0.1.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-strings-0.1.0/license-apache-2.0), [`license-mit`](dependencies/windows-strings-0.1.0/license-mit) |
| `windows-strings` | `0.3.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-strings-0.3.1/license-apache-2.0), [`license-mit`](dependencies/windows-strings-0.3.1/license-mit) |
| `windows-strings` | `0.4.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-strings-0.4.2/license-apache-2.0), [`license-mit`](dependencies/windows-strings-0.4.2/license-mit) |
| `windows-strings` | `0.5.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-strings-0.5.1/license-apache-2.0), [`license-mit`](dependencies/windows-strings-0.5.1/license-mit) |
| `windows-sys` | `0.48.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-sys-0.48.0/license-apache-2.0), [`license-mit`](dependencies/windows-sys-0.48.0/license-mit) |
| `windows-sys` | `0.52.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-sys-0.52.0/license-apache-2.0), [`license-mit`](dependencies/windows-sys-0.52.0/license-mit) |
| `windows-sys` | `0.59.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-sys-0.59.0/license-apache-2.0), [`license-mit`](dependencies/windows-sys-0.59.0/license-mit) |
| `windows-sys` | `0.60.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-sys-0.60.2/license-apache-2.0), [`license-mit`](dependencies/windows-sys-0.60.2/license-mit) |
| `windows-sys` | `0.61.2` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-sys-0.61.2/license-apache-2.0), [`license-mit`](dependencies/windows-sys-0.61.2/license-mit) |
| `windows-targets` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-targets-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows-targets-0.48.5/license-mit) |
| `windows-targets` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-targets-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows-targets-0.52.6/license-mit) |
| `windows-targets` | `0.53.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-targets-0.53.5/license-apache-2.0), [`license-mit`](dependencies/windows-targets-0.53.5/license-mit) |
| `windows-threading` | `0.1.0` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-threading-0.1.0/license-apache-2.0), [`license-mit`](dependencies/windows-threading-0.1.0/license-mit) |
| `windows-threading` | `0.2.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-threading-0.2.1/license-apache-2.0), [`license-mit`](dependencies/windows-threading-0.2.1/license-mit) |
| `windows-version` | `0.1.7` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows-version-0.1.7/license-apache-2.0), [`license-mit`](dependencies/windows-version-0.1.7/license-mit) |
| `windows_aarch64_gnullvm` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_aarch64_gnullvm-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows_aarch64_gnullvm-0.48.5/license-mit) |
| `windows_aarch64_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_aarch64_gnullvm-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_aarch64_gnullvm-0.52.6/license-mit) |
| `windows_aarch64_gnullvm` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_aarch64_gnullvm-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_aarch64_gnullvm-0.53.1/license-mit) |
| `windows_aarch64_msvc` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_aarch64_msvc-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows_aarch64_msvc-0.48.5/license-mit) |
| `windows_aarch64_msvc` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_aarch64_msvc-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_aarch64_msvc-0.52.6/license-mit) |
| `windows_aarch64_msvc` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_aarch64_msvc-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_aarch64_msvc-0.53.1/license-mit) |
| `windows_i686_gnu` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_gnu-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows_i686_gnu-0.48.5/license-mit) |
| `windows_i686_gnu` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_gnu-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_i686_gnu-0.52.6/license-mit) |
| `windows_i686_gnu` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_gnu-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_i686_gnu-0.53.1/license-mit) |
| `windows_i686_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_gnullvm-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_i686_gnullvm-0.52.6/license-mit) |
| `windows_i686_gnullvm` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_gnullvm-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_i686_gnullvm-0.53.1/license-mit) |
| `windows_i686_msvc` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_msvc-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows_i686_msvc-0.48.5/license-mit) |
| `windows_i686_msvc` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_msvc-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_i686_msvc-0.52.6/license-mit) |
| `windows_i686_msvc` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_i686_msvc-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_i686_msvc-0.53.1/license-mit) |
| `windows_x86_64_gnu` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_gnu-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_gnu-0.48.5/license-mit) |
| `windows_x86_64_gnu` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_gnu-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_gnu-0.52.6/license-mit) |
| `windows_x86_64_gnu` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_gnu-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_gnu-0.53.1/license-mit) |
| `windows_x86_64_gnullvm` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_gnullvm-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_gnullvm-0.48.5/license-mit) |
| `windows_x86_64_gnullvm` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_gnullvm-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_gnullvm-0.52.6/license-mit) |
| `windows_x86_64_gnullvm` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_gnullvm-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_gnullvm-0.53.1/license-mit) |
| `windows_x86_64_msvc` | `0.48.5` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_msvc-0.48.5/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_msvc-0.48.5/license-mit) |
| `windows_x86_64_msvc` | `0.52.6` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_msvc-0.52.6/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_msvc-0.52.6/license-mit) |
| `windows_x86_64_msvc` | `0.53.1` | `MIT OR Apache-2.0` | [`license-apache-2.0`](dependencies/windows_x86_64_msvc-0.53.1/license-apache-2.0), [`license-mit`](dependencies/windows_x86_64_msvc-0.53.1/license-mit) |
| `winnow` | `0.7.15` | `MIT` | [`LICENSE-MIT`](dependencies/winnow-0.7.15/LICENSE-MIT) |
| `winnow` | `1.0.4` | `MIT` | [`LICENSE-MIT`](dependencies/winnow-1.0.4/LICENSE-MIT) |
| `winreg` | `0.56.0` | `MIT` | [`LICENSE`](dependencies/winreg-0.56.0/LICENSE) |
| `wio` | `0.2.2` | `MIT/Apache-2.0` | [`LICENSE-APACHE`](dependencies/wio-0.2.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/wio-0.2.2/LICENSE-MIT) |
| `wit-bindgen` | `0.57.1` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/wit-bindgen-0.57.1/LICENSE-APACHE), [`LICENSE-Apache-2.0_WITH_LLVM-exception`](dependencies/wit-bindgen-0.57.1/LICENSE-Apache-2.0_WITH_LLVM-exception), [`LICENSE-MIT`](dependencies/wit-bindgen-0.57.1/LICENSE-MIT) |
| `writeable` | `0.6.4` | `Unicode-3.0` | [`LICENSE`](dependencies/writeable-0.6.4/LICENSE) |
| `x11` | `2.21.0` | `MIT` | [`LICENSE-MIT`](dependencies/x11-2.21.0/LICENSE-MIT) |
| `x11rb` | `0.13.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/x11rb-0.13.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/x11rb-0.13.2/LICENSE-MIT) |
| `x11rb-protocol` | `0.13.2` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/x11rb-protocol-0.13.2/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/x11rb-protocol-0.13.2/LICENSE-MIT) |
| `xcb` | `1.7.1` | `MIT` | [`LICENSE`](dependencies/xcb-1.7.1/LICENSE) |
| `xcursor` | `0.3.11` | `MIT` | [`LICENSE`](dependencies/xcursor-0.3.11/LICENSE) |
| `xdg-home` | `1.3.0` | `MIT` | [`LICENSE-MIT`](dependencies/xdg-home-1.3.0/LICENSE-MIT) |
| `xim-ctext` | `0.3.0` | `MIT` | **No regular license file found** |
| `xim-parser` | `0.2.2` | `MIT` | **No regular license file found** |
| `xkbcommon` | `0.8.0` | `MIT` | [`LICENSE`](dependencies/xkbcommon-0.8.0/LICENSE) |
| `xkeysym` | `0.2.1` | `MIT OR Apache-2.0 OR Zlib` | [`LICENSE-APACHE`](dependencies/xkeysym-0.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/xkeysym-0.2.1/LICENSE-MIT), [`LICENSE-ZLIB`](dependencies/xkeysym-0.2.1/LICENSE-ZLIB) |
| `xml-rs` | `0.8.29` | `MIT` | [`LICENSE`](dependencies/xml-rs-0.8.29/LICENSE) |
| `xml5ever` | `0.18.1` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/xml5ever-0.18.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/xml5ever-0.18.1/LICENSE-MIT) |
| `xml5ever` | `0.39.0` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/xml5ever-0.39.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/xml5ever-0.39.0/LICENSE-MIT) |
| `xmlwriter` | `0.1.0` | `MIT` | [`LICENSE`](dependencies/xmlwriter-0.1.0/LICENSE) |
| `y4m` | `0.8.0` | `MIT` | [`LICENSE`](dependencies/y4m-0.8.0/LICENSE) |
| `yazi` | `0.2.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/yazi-0.2.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/yazi-0.2.1/LICENSE-MIT) |
| `yeslogic-fontconfig-sys` | `6.0.1` | `MIT` | [`LICENSE`](dependencies/yeslogic-fontconfig-sys-6.0.1/LICENSE) |
| `yoke` | `0.8.3` | `Unicode-3.0` | [`LICENSE`](dependencies/yoke-0.8.3/LICENSE) |
| `yoke-derive` | `0.8.4` | `Unicode-3.0` | [`LICENSE`](dependencies/yoke-derive-0.8.4/LICENSE) |
| `zbus` | `4.4.0` | `MIT` | [`LICENSE`](dependencies/zbus-4.4.0/LICENSE) |
| `zbus` | `5.19.0` | `MIT` | [`LICENSE`](dependencies/zbus-5.19.0/LICENSE) |
| `zbus-lockstep` | `0.5.2` | `MIT` | [`LICENSE-MIT`](dependencies/zbus-lockstep-0.5.2/LICENSE-MIT) |
| `zbus-lockstep-macros` | `0.5.2` | `MIT` | [`LICENSE-MIT`](dependencies/zbus-lockstep-macros-0.5.2/LICENSE-MIT) |
| `zbus_macros` | `4.4.0` | `MIT` | [`LICENSE`](dependencies/zbus_macros-4.4.0/LICENSE) |
| `zbus_macros` | `5.19.0` | `MIT` | [`LICENSE`](dependencies/zbus_macros-5.19.0/LICENSE) |
| `zbus_names` | `3.0.0` | `MIT` | [`LICENSE`](dependencies/zbus_names-3.0.0/LICENSE) |
| `zbus_names` | `4.3.4` | `MIT` | [`LICENSE`](dependencies/zbus_names-4.3.4/LICENSE) |
| `zbus_xml` | `5.2.1` | `MIT` | [`LICENSE`](dependencies/zbus_xml-5.2.1/LICENSE) |
| `zcheapstr` | `1.1.0` | `MIT` | [`LICENSE`](dependencies/zcheapstr-1.1.0/LICENSE) |
| `zed-font-kit` | `0.14.1-zed` | `MIT OR Apache-2.0` | [`LICENSE-APACHE`](dependencies/zed-font-kit-0.14.1-zed/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/zed-font-kit-0.14.1-zed/LICENSE-MIT) |
| `zed-scap` | `0.0.8-zed` | `MIT` | [`LICENSE`](dependencies/zed-scap-0.0.8-zed/LICENSE) |
| `zed-xim` | `0.4.0-zed` | `MIT` | [`LICENSE`](dependencies/zed-xim-0.4.0-zed/LICENSE) |
| `zeno` | `0.3.3` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/zeno-0.3.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/zeno-0.3.3/LICENSE-MIT) |
| `zerocopy` | `0.8.62` | `BSD-2-Clause OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/zerocopy-0.8.62/LICENSE-APACHE), [`LICENSE-BSD`](dependencies/zerocopy-0.8.62/LICENSE-BSD), [`LICENSE-MIT`](dependencies/zerocopy-0.8.62/LICENSE-MIT) |
| `zerocopy-derive` | `0.8.62` | `BSD-2-Clause OR Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/zerocopy-derive-0.8.62/LICENSE-APACHE), [`LICENSE-BSD`](dependencies/zerocopy-derive-0.8.62/LICENSE-BSD), [`LICENSE-MIT`](dependencies/zerocopy-derive-0.8.62/LICENSE-MIT) |
| `zerofrom` | `0.1.8` | `Unicode-3.0` | [`LICENSE`](dependencies/zerofrom-0.1.8/LICENSE) |
| `zerofrom-derive` | `0.1.8` | `Unicode-3.0` | [`LICENSE`](dependencies/zerofrom-derive-0.1.8/LICENSE) |
| `zeroize` | `1.9.1` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/zeroize-1.9.1/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/zeroize-1.9.1/LICENSE-MIT) |
| `zeroize_derive` | `1.5.0` | `Apache-2.0 OR MIT` | [`LICENSE-APACHE`](dependencies/zeroize_derive-1.5.0/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/zeroize_derive-1.5.0/LICENSE-MIT) |
| `zerotrie` | `0.2.5` | `Unicode-3.0` | [`LICENSE`](dependencies/zerotrie-0.2.5/LICENSE) |
| `zerovec` | `0.11.8` | `Unicode-3.0` | [`LICENSE`](dependencies/zerovec-0.11.8/LICENSE) |
| `zerovec-derive` | `0.11.6` | `Unicode-3.0` | [`LICENSE`](dependencies/zerovec-derive-0.11.6/LICENSE) |
| `zlib-rs` | `0.6.8` | `Zlib` | [`LICENSE`](dependencies/zlib-rs-0.6.8/LICENSE) |
| `zmij` | `1.0.23` | `MIT` | [`LICENSE-MIT`](dependencies/zmij-1.0.23/LICENSE-MIT) |
| `zune-core` | `0.4.12` | `MIT OR Apache-2.0 OR Zlib` | **No regular license file found** |
| `zune-core` | `0.5.3` | `MIT OR Apache-2.0 OR Zlib` | [`LICENSE-APACHE`](dependencies/zune-core-0.5.3/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/zune-core-0.5.3/LICENSE-MIT), [`LICENSE-ZLIB`](dependencies/zune-core-0.5.3/LICENSE-ZLIB) |
| `zune-inflate` | `0.2.54` | `MIT OR Apache-2.0 OR Zlib` | **No regular license file found** |
| `zune-jpeg` | `0.4.21` | `MIT OR Apache-2.0 OR Zlib` | **No regular license file found** |
| `zune-jpeg` | `0.5.15` | `MIT OR Apache-2.0 OR Zlib` | [`LICENSE-APACHE`](dependencies/zune-jpeg-0.5.15/LICENSE-APACHE), [`LICENSE-MIT`](dependencies/zune-jpeg-0.5.15/LICENSE-MIT), [`LICENSE-ZLIB`](dependencies/zune-jpeg-0.5.15/LICENSE-ZLIB) |
| `zvariant` | `4.2.0` | `MIT` | [`LICENSE`](dependencies/zvariant-4.2.0/LICENSE) |
| `zvariant` | `5.15.0` | `MIT` | [`LICENSE`](dependencies/zvariant-5.15.0/LICENSE) |
| `zvariant_derive` | `4.2.0` | `MIT` | [`LICENSE`](dependencies/zvariant_derive-4.2.0/LICENSE) |
| `zvariant_derive` | `5.15.0` | `MIT` | [`LICENSE`](dependencies/zvariant_derive-5.15.0/LICENSE) |
| `zvariant_utils` | `2.1.0` | `MIT` | [`LICENSE`](dependencies/zvariant_utils-2.1.0/LICENSE) |
| `zvariant_utils` | `4.2.0` | `MIT` | [`LICENSE`](dependencies/zvariant_utils-4.2.0/LICENSE) |

## Missing or skipped package license files

The following package roots had license candidates that were missing, unsafe, outside the package root, or over a size/count limit. Cargo's declarations are reported above; review these entries before distribution.

- `accesskit@0.24.1` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `accesskit_atspi_common@0.19.1` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `accesskit_consumer@0.38.0` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `accesskit_macos@0.26.3` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `accesskit_unix@0.22.1` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `accesskit_windows@0.34.0` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `block@0.1.6` (`MIT`): no package-root license text found (Cargo declares MIT)
- `block2@0.5.1` (`MIT`): no package-root license text found (Cargo declares MIT)
- `block2@0.6.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `css-inline@0.21.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `dispatch@0.2.0` (`MIT`): no package-root license text found (Cargo declares MIT)
- `dispatch2@0.3.1` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `dwrote@0.11.5` (`MPL-2.0`): no package-root license text found (Cargo declares MPL-2.0)
- `gl_generator@0.14.0` (`Apache-2.0`): no package-root license text found (Cargo declares Apache-2.0)
- `gpu-descriptor@0.3.2` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `gpu-descriptor-types@0.2.0` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `harfrust@0.5.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `hexf-parse@0.2.1` (`CC0-1.0`): no package-root license text found (Cargo declares CC0-1.0)
- `imap-proto@0.16.7` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `jni-sys-macros@0.4.1` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `json5@0.4.1` (`ISC`): no package-root license text found (Cargo declares ISC)
- `khronos_api@3.1.0` (`Apache-2.0`): no package-root license text found (Cargo declares Apache-2.0)
- `leak@0.1.2` (`Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Apache-2.0 OR MIT)
- `leaky-cow@0.1.1` (`MIT / Apache-2.0`): no package-root license text found (Cargo declares MIT / Apache-2.0)
- `lyon@1.0.19` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `lyon_algorithms@1.0.21` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `lyon_geom@1.0.19` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `lyon_path@1.0.19` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `lyon_tessellation@1.0.22` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `mac@0.1.1` (`MIT/Apache-2.0`): no package-root license text found (Cargo declares MIT/Apache-2.0)
- `mac-notification-sys@0.6.15` (`MIT/Apache-2.0`): no package-root license text found (Cargo declares MIT/Apache-2.0)
- `malloc_buf@0.0.6` (`MIT`): no package-root license text found (Cargo declares MIT)
- `ndk-sys@0.6.0+11769913` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `objc-foundation@0.1.1` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc-sys@0.3.5` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2@0.5.3` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2@0.6.5` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-app-kit@0.2.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-app-kit@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-cloud-kit@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-audio@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-audio-types@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-data@0.2.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-core-data@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-foundation@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-graphics@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-image@0.2.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-core-image@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-location@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-media@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-text@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-core-video@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-encode@4.1.0` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-foundation@0.2.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-foundation@0.3.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-io-surface@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-metal@0.2.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-metal@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-quartz-core@0.2.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc2-quartz-core@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-screen-capture-kit@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc2-user-notifications@0.3.2` (`Zlib OR Apache-2.0 OR MIT`): no package-root license text found (Cargo declares Zlib OR Apache-2.0 OR MIT)
- `objc_exception@0.1.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `objc_id@0.1.1` (`MIT`): no package-root license text found (Cargo declares MIT)
- `pathfinder_geometry@0.5.1` (`MIT/Apache-2.0`): no package-root license text found (Cargo declares MIT/Apache-2.0)
- `pathfinder_simd@0.5.6` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `plist@1.10.1` (`MIT`): no package-root license text found (Cargo declares MIT)
- `profiling@1.0.18` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `profiling-procmacros@1.0.18` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `pulp-wasm-simd-flag@0.1.1` (`MIT`): no package-root license text found (Cargo declares MIT)
- `r-efi@5.3.0` (`MIT OR Apache-2.0 OR LGPL-2.1-or-later`): no package-root license text found (Cargo declares MIT OR Apache-2.0 OR LGPL-2.1-or-later)
- `r-efi@6.0.0` (`MIT OR Apache-2.0 OR LGPL-2.1-or-later`): no package-root license text found (Cargo declares MIT OR Apache-2.0 OR LGPL-2.1-or-later)
- `rust-i18n-macro@4.2.4` (`MIT`): no package-root license text found (Cargo declares MIT)
- `rust-i18n-support@4.2.4` (`MIT`): no package-root license text found (Cargo declares MIT)
- `screencapturekit@0.2.8` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `screencapturekit-sys@0.2.8` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `seahash@4.1.0` (`MIT`): no package-root license text found (Cargo declares MIT)
- `selectors@0.40.0` (`MPL-2.0`): no package-root license text found (Cargo declares MPL-2.0)
- `simd_helpers@0.1.0` (`MIT`): no package-root license text found (Cargo declares MIT)
- `spirv@0.4.0+sdk-1.4.341.0` (`Apache-2.0`): no package-root license text found (Cargo declares Apache-2.0)
- `stop-token@0.7.0` (`MIT OR Apache-2.0`): no package-root license text found (Cargo declares MIT OR Apache-2.0)
- `svg_fmt@0.4.5` (`MIT/Apache-2.0`): no package-root license text found (Cargo declares MIT/Apache-2.0)
- `taffy@0.13.0` (`MIT`): no package-root license text found (Cargo declares MIT)
- `valuable@0.1.1` (`MIT`): no package-root license text found (Cargo declares MIT)
- `winapi-i686-pc-windows-gnu@0.4.0` (`MIT/Apache-2.0`): no package-root license text found (Cargo declares MIT/Apache-2.0)
- `winapi-x86_64-pc-windows-gnu@0.4.0` (`MIT/Apache-2.0`): no package-root license text found (Cargo declares MIT/Apache-2.0)
- `windows-capture@1.5.0` (`MIT`): no package-root license text found (Cargo declares MIT)
- `xim-ctext@0.3.0` (`MIT`): no package-root license text found (Cargo declares MIT)
- `xim-parser@0.2.2` (`MIT`): no package-root license text found (Cargo declares MIT)
- `zune-core@0.4.12` (`MIT OR Apache-2.0 OR Zlib`): no package-root license text found (Cargo declares MIT OR Apache-2.0 OR Zlib)
- `zune-inflate@0.2.54` (`MIT OR Apache-2.0 OR Zlib`): no package-root license text found (Cargo declares MIT OR Apache-2.0 OR Zlib)
- `zune-jpeg@0.4.21` (`MIT OR Apache-2.0 OR Zlib`): no package-root license text found (Cargo declares MIT OR Apache-2.0 OR Zlib)

Dependency source roots are read-only Cargo registry or Git checkouts. The generator copies
bounded regular files named `LICENSE*`, `COPYING*`, or `NOTICE*`, plus any Cargo-declared
`license_file` that resolves inside the package root. GPUI Kit's missing Apache file comes
from the matching official upstream tag; its URL and hash are recorded in `inventory.json`.
