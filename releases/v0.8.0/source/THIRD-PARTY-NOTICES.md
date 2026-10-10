# 第三方许可证声明

FindFlow 以 MIT 许可证发布（见 `LICENSE`）。本软件使用了下列开源组件，各组件版权归其作者所有，
并按各自的许可证授权。分发 FindFlow 时请一并保留本声明。各许可证全文见对应项目仓库或其发布包。

> 本清单由 `cargo tree`（Windows 目标、正常依赖）与 `package-lock.json`（生产依赖）生成，
> 版本以 FindFlow 当前发布版本为准。其中 MPL-2.0 许可的组件（如 `cssparser`、`selectors`、`dtoa-short`、
> `option-ext`）仅要求对这些文件本身的修改保持开源，FindFlow 未修改它们，其源码可在对应仓库获取。
> 双重许可的组件（如 `r-efi`：MIT OR Apache-2.0 OR LGPL-2.1-or-later）在此选用其中宽松的 MIT / Apache-2.0。

## Rust 依赖（537 项）

| 名称 | 版本 | 许可证 | 仓库 |
| --- | --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | https://github.com/oyvindln/adler2 |
| adler32 | 1.2.0 | Zlib | https://github.com/remram44/adler32-rs |
| adobe-cmap-parser | 0.4.1 | MIT | https://github.com/jrmuizel/adobe-cmap-parser |
| ahash | 0.8.12 | MIT OR Apache-2.0 | https://github.com/tkaitchuck/ahash |
| aho-corasick | 1.1.5 | Unlicense OR MIT | https://github.com/BurntSushi/aho-corasick |
| aho-corasick | 1.1.5 | Unlicense OR MIT | https://github.com/BurntSushi/aho-corasick (*) |
| alloc-no-stdlib | 3.0.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| alloc-stdlib | 0.3.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| alloc-stdlib | 0.3.0 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib (*) |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 | https://github.com/zakarumych/allocator-api2 |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | https://github.com/dtolnay/anyhow |
| arc-swap | 1.9.2 | MIT OR Apache-2.0 | https://github.com/vorner/arc-swap |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | https://github.com/bluss/arrayvec |
| async-trait | 0.1.92 | MIT OR Apache-2.0 | https://github.com/dtolnay/async-trait |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/atomic-waker |
| auto-launch | 0.5.0 | MIT | https://github.com/zzzgydi/auto-launch.git |
| base64 | 0.22.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| base64 | 0.23.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| bit-set | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-set |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-vec |
| bitflags | 1.3.2 | MIT/Apache-2.0 | https://github.com/bitflags/bitflags |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags (*) |
| bitpacking | 0.9.3 | MIT | https://github.com/quickwit-oss/bitpacking |
| bitpacking | 0.9.3 | MIT | https://github.com/quickwit-oss/bitpacking (*) |
| blake3 | 1.8.7 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | https://github.com/BLAKE3-team/BLAKE3 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| brotli | 9.0.0 | BSD-3-Clause AND MIT | https://github.com/dropbox/rust-brotli |
| brotli | 9.0.0 | BSD-3-Clause AND MIT | https://github.com/dropbox/rust-brotli (*) |
| brotli-decompressor | 6.0.1 | BSD-3-Clause/MIT | https://github.com/dropbox/rust-brotli-decompressor |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | https://github.com/fitzgen/bumpalo |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/bytemuck |
| byteorder | 1.5.0 | Unlicense OR MIT | https://github.com/BurntSushi/byteorder |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | https://github.com/image-rs/byteorder-lite |
| bytes | 1.12.1 | MIT | https://github.com/tokio-rs/bytes |
| camino | 1.2.6 | MIT OR Apache-2.0 | https://github.com/camino-rs/camino |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 | https://github.com/rust-lang/cargo |
| cargo_metadata | 0.19.2 | MIT | https://github.com/oli-obk/cargo_metadata |
| cedarwood | 0.4.6 | BSD-2-Clause | https://github.com/MnO2/cedarwood |
| census | 0.4.2 | MIT | https://github.com/quickwit-inc/census |
| cfb | 0.14.0 | MIT | https://github.com/mdsteele/rust-cfb |
| cfb | 0.7.3 | MIT | https://github.com/mdsteele/rust-cfb |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if |
| chrono | 0.4.45 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono |
| clipboard-win | 5.4.1 | BSL-1.0 | https://github.com/DoumanAsh/clipboard-win |
| color_quant | 1.1.0 | MIT | https://github.com/image-rs/color_quant.git |
| constant_time_eq | 0.4.2 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://github.com/cesarb/constant_time_eq |
| cookie | 0.18.2 | MIT OR Apache-2.0 | https://github.com/SergioBenitez/cookie-rs |
| cookie | 0.18.2 | MIT OR Apache-2.0 | https://github.com/SergioBenitez/cookie-rs (*) |
| core_detect | 1.0.0 | MIT/Apache-2.0 | https://github.com/thomcc/core_detect |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast (*) |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam (*) |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crunchy | 0.2.4 | MIT | https://github.com/eira-fransham/crunchy |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| cssparser | 0.37.0 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| cssparser | 0.37.0 | MPL-2.0 | https://github.com/servo/rust-cssparser (*) |
| cssparser-macros | 0.7.1 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| ctor | 1.0.13 | Apache-2.0 OR MIT | https://github.com/mmastrac/linktime |
| darling | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_core | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| darling_core | 0.24.1 | MIT | https://github.com/TedDriggs/darling (*) |
| darling_macro | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| dary_heap | 0.3.9 | MIT OR Apache-2.0 | https://github.com/hanmertens/dary_heap |
| deranged | 0.5.8 | MIT OR Apache-2.0 | https://github.com/jhpratt/deranged |
| derive_more | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| derive_more-impl | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| digest | 0.10.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| dirs | 7.0.0 | MIT OR Apache-2.0 | https://codeberg.org/dirs/dirs-rs |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 | https://github.com/dirs-dev/dirs-sys-rs |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | https://github.com/yaahc/displaydoc |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | https://github.com/yaahc/displaydoc (*) |
| dom_query | 0.28.0 | MIT | https://github.com/niklak/dom_query |
| downcast-rs | 1.2.1 | MIT/Apache-2.0 | https://github.com/marcianx/downcast-rs |
| dpi | 0.1.2 | Apache-2.0 AND MIT | https://github.com/rust-windowing/winit |
| dpi | 0.1.2 | Apache-2.0 AND MIT | https://github.com/rust-windowing/winit (*) |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | https://github.com/dtolnay/dtoa |
| dtoa-short | 0.3.5 | MPL-2.0 | https://github.com/upsuper/dtoa-short |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://gitlab.com/kornelski/dunce |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 | https://github.com/dtolnay/dyn-clone |
| either | 1.18.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/either |
| encoding_rs | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause | https://github.com/hsivonen/encoding_rs |
| encoding_rs | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause | https://github.com/hsivonen/encoding_rs (*) |
| equivalent | 1.0.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/equivalent |
| erased-serde | 0.4.10 | MIT OR Apache-2.0 | https://github.com/dtolnay/erased-serde |
| error-code | 3.4.0 | BSL-1.0 | https://github.com/DoumanAsh/error-code |
| euclid | 0.20.14 | MIT / Apache-2.0 | https://github.com/servo/euclid |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | https://github.com/sfackler/rust-fallible-iterator |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | https://github.com/sfackler/fallible-streaming-iterator |
| fastdivide | 0.4.2 | zlib-acknowledgement OR MIT | https://github.com/fulmicoton/fastdivide |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/fastrand |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | https://github.com/image-rs/fdeflate |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | https://github.com/image-rs/fdeflate (*) |
| filetime | 0.2.29 | MIT/Apache-2.0 | https://github.com/alexcrichton/filetime |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/flate2-rs |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/flate2-rs (*) |
| fnv | 1.0.7 | Apache-2.0 / MIT | https://github.com/servo/rust-fnv |
| foldhash | 0.1.5 | Zlib | https://github.com/orlp/foldhash |
| foldhash | 0.2.0 | Zlib | https://github.com/orlp/foldhash |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url (*) |
| fs4 | 0.8.4 | MIT OR Apache-2.0 | https://github.com/al8n/fs4-rs |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs (*) |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs (*) |
| fxhash | 0.2.1 | Apache-2.0/MIT | https://github.com/cbreeden/fxhash |
| generic-array | 0.14.7 | MIT | https://github.com/fizyk20/generic-array.git |
| generic-array | 0.14.7 | MIT | https://github.com/fizyk20/generic-array.git (*) |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom (*) |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom (*) |
| gif | 0.14.2 | MIT OR Apache-2.0 | https://github.com/image-rs/image-gif |
| glob | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/glob |
| global-hotkey | 0.8.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/global-hotkey |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.14.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashlink | 0.9.1 | MIT OR Apache-2.0 | https://github.com/kyren/hashlink |
| heck | 0.5.0 | MIT OR Apache-2.0 | https://github.com/withoutboats/heck |
| html5ever | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| htmlescape | 0.3.1 | Apache-2.0 / MIT / MPL-2.0 | https://github.com/veddan/rust-htmlescape |
| http | 1.5.0 | MIT OR Apache-2.0 | https://github.com/hyperium/http |
| http | 1.5.0 | MIT OR Apache-2.0 | https://github.com/hyperium/http (*) |
| http-body | 1.1.0 | MIT | https://github.com/hyperium/http-body |
| http-body | 1.1.0 | MIT | https://github.com/hyperium/http-body (*) |
| http-body-util | 0.1.5 | MIT | https://github.com/hyperium/http-body |
| http-body-util | 0.1.5 | MIT | https://github.com/hyperium/http-body (*) |
| httparse | 1.10.1 | MIT OR Apache-2.0 | https://github.com/seanmonstar/httparse |
| hyper | 1.11.1 | MIT | https://github.com/hyperium/hyper |
| hyper | 1.11.1 | MIT | https://github.com/hyperium/hyper (*) |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/hyper-rustls |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/hyper-rustls (*) |
| hyper-util | 0.1.20 | MIT | https://github.com/hyperium/hyper-util |
| hyper-util | 0.1.20 | MIT | https://github.com/hyperium/hyper-util (*) |
| ico | 0.5.0 | MIT | https://github.com/mdsteele/rust-ico |
| icu_collections | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_collections | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| icu_locale_core | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_locale_core | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| icu_normalizer | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| icu_properties_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_provider | 2.3.1 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_provider | 2.3.1 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| ident_case | 1.0.1 | MIT/Apache-2.0 | https://github.com/TedDriggs/ident_case |
| idna | 1.1.0 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | https://github.com/hsivonen/idna_adapter |
| image | 0.25.10 | MIT OR Apache-2.0 | https://github.com/image-rs/image |
| image-webp | 0.2.4 | MIT OR Apache-2.0 | https://github.com/image-rs/image-webp |
| include-flate | 0.3.4 | Apache-2.0 | https://github.com/SOF3/include-flate.git |
| include-flate-codegen | 0.3.4 | Apache-2.0 | https://github.com/SOF3/include-flate.git |
| include-flate-compress | 0.3.4 | Apache-2.0 | https://github.com/SOF3/include-flate.git |
| indexmap | 1.9.3 | Apache-2.0 OR MIT | https://github.com/bluss/indexmap |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap (*) |
| infer | 0.19.0 | MIT | https://github.com/bojand/infer |
| infer | 0.22.0 | MIT | https://github.com/bojand/infer |
| instant | 0.1.13 | BSD-3-Clause | https://github.com/sebcrozet/instant |
| instant | 0.1.13 | BSD-3-Clause | https://github.com/sebcrozet/instant (*) |
| ipnet | 2.12.2 | MIT OR Apache-2.0 | https://github.com/krisprice/ipnet |
| itertools | 0.12.1 | MIT OR Apache-2.0 | https://github.com/rust-itertools/itertools |
| itertools | 0.12.1 | MIT OR Apache-2.0 | https://github.com/rust-itertools/itertools (*) |
| itoa | 1.0.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/itoa |
| jieba-macros | 0.7.1 | MIT | https://github.com/messense/jieba-rs |
| jieba-rs | 0.7.4 | MIT | https://github.com/messense/jieba-rs |
| json-patch | 4.2.0 | MIT/Apache-2.0 | https://github.com/idubrov/json-patch |
| json-patch | 4.2.0 | MIT/Apache-2.0 | https://github.com/idubrov/json-patch (*) |
| jsonptr | 0.7.1 | MIT OR Apache-2.0 | https://github.com/chanced/jsonptr |
| keyboard-types | 0.7.0 | MIT OR Apache-2.0 | https://github.com/pyfisch/keyboard-types |
| keyboard-types | 0.8.3 | MIT OR Apache-2.0 | https://github.com/rust-windowing/keyboard-types |
| lazy_static | 1.5.0 | MIT OR Apache-2.0 | https://github.com/rust-lang-nursery/lazy-static.rs |
| levenshtein_automata | 0.2.1 | MIT | https://github.com/tantivy-search/levenshtein-automata |
| libc | 0.2.189 | MIT OR Apache-2.0 | https://github.com/rust-lang/libc |
| libflate | 2.3.2 | MIT | https://github.com/sile/libflate |
| libflate_lz77 | 2.3.0 | MIT | https://github.com/sile/libflate |
| libm | 0.2.16 | MIT | https://github.com/rust-lang/compiler-builtins |
| libsqlite3-sys | 0.30.1 | MIT | https://github.com/rusqlite/rusqlite |
| litemap | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| log | 0.4.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| lopdf | 0.34.0 | MIT | https://github.com/J-F-Liu/lopdf.git |
| lru | 0.12.5 | MIT | https://github.com/jeromefroe/lru-rs.git |
| lz4_flex | 0.11.6 | MIT | https://github.com/pseitz/lz4_flex |
| markup5ever | 0.39.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| matchers | 0.2.0 | MIT | https://github.com/hawkw/matchers |
| md-5 | 0.10.6 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| measure_time | 0.8.3 | MIT | https://github.com/PSeitz/rust_measure_time |
| memchr | 2.8.3 | Unlicense OR MIT | https://github.com/BurntSushi/memchr |
| memmap2 | 0.9.11 | MIT OR Apache-2.0 | https://github.com/RazrFalcon/memmap2-rs |
| mime | 0.3.17 | MIT OR Apache-2.0 | https://github.com/hyperium/mime |
| minimal-lexical | 0.2.1 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/minimal-lexical |
| minisign-verify | 0.2.5 | MIT | https://github.com/jedisct1/rust-minisign-verify |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide (*) |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| mio | 1.2.3 | MIT | https://github.com/tokio-rs/mio |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | https://github.com/awxkee/moxcms.git |
| muda | 0.20.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/muda |
| muda | 0.20.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/muda (*) |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT | https://github.com/hsivonen/multiversion_no_op |
| murmurhash32 | 0.3.1 | MIT | https://github.com/quickwit-inc/murmurhash32 |
| new_debug_unreachable | 1.0.6 | MIT | https://github.com/mbrubeck/rust-debug-unreachable |
| no_std_io2 | 0.9.4 | Apache-2.0 OR MIT | https://github.com/wcampbell0x2a/no-std-io2 |
| no_std_io2 | 0.9.4 | Apache-2.0 OR MIT | https://github.com/wcampbell0x2a/no-std-io2 (*) |
| nom | 7.1.3 | MIT | https://github.com/Geal/nom |
| nom | 7.1.3 | MIT | https://github.com/Geal/nom (*) |
| notify | 7.0.0 | CC0-1.0 | https://github.com/notify-rs/notify.git |
| notify-types | 1.0.1 | MIT OR Apache-2.0 | https://github.com/notify-rs/notify.git |
| nu-ansi-term | 0.50.3 | MIT | https://github.com/nushell/nu-ansi-term |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | https://github.com/jhpratt/num-conv |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits (*) |
| num_cpus | 1.17.0 | MIT OR Apache-2.0 | https://github.com/seanmonstar/num_cpus |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell |
| oneshot | 0.1.13 | MIT OR Apache-2.0 | https://github.com/faern/oneshot |
| open | 5.4.4 | MIT | https://github.com/Byron/open-rs |
| option-ext | 0.2.0 | MPL-2.0 | https://github.com/soc/option-ext.git |
| ownedbytes | 0.7.0 | MIT | https://github.com/quickwit-oss/tantivy |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| pdf-extract | 0.7.12 | MIT | https://github.com/jrmuizel/pdf-extract |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| phf | 0.11.3 | MIT | https://github.com/rust-phf/rust-phf |
| phf | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf (*) |
| phf_codegen | 0.11.3 | MIT | https://github.com/rust-phf/rust-phf |
| phf_generator | 0.11.3 | MIT | https://github.com/rust-phf/rust-phf |
| phf_generator | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_macros | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_macros | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf (*) |
| phf_shared | 0.11.3 | MIT | https://github.com/rust-phf/rust-phf |
| phf_shared | 0.11.3 | MIT | https://github.com/rust-phf/rust-phf (*) |
| phf_shared | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| phf_shared | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf (*) |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project-lite |
| pinyin | 0.10.0 | MIT | https://github.com/mozillazg/rust-pinyin |
| plist | 1.10.1 | MIT | https://github.com/ebarnard/rust-plist/ |
| png | 0.17.16 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| png | 0.18.1 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| pom | 1.1.0 | MIT | https://github.com/J-F-Liu/pom.git |
| postscript | 0.14.1 | Apache-2.0/MIT | https://github.com/bodoni/postscript |
| potential_utf | 0.1.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| powerfmt | 0.2.0 | MIT OR Apache-2.0 | https://github.com/jhpratt/powerfmt |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | https://github.com/cryptocorrosion/cryptocorrosion |
| precomputed-hash | 0.1.1 | MIT | https://github.com/emilio/precomputed-hash |
| proc-macro-error-attr3 | 3.1.1 | MIT OR Apache-2.0 | https://github.com/gamma0987/proc-macro-error3 |
| proc-macro-error3 | 3.1.1 | MIT OR Apache-2.0 | https://github.com/gamma0987/proc-macro-error3 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 (*) |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | https://github.com/awxkee/pxfm |
| quick-error | 2.0.1 | MIT/Apache-2.0 | http://github.com/tailhook/quick-error |
| quick-xml | 0.42.0 | MIT | https://github.com/tafia/quick-xml |
| quote | 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote |
| quote | 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote (*) |
| rand | 0.8.8 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | https://github.com/rust-random/rand (*) |
| rand_distr | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rangemap | 1.8.0 | MIT/Apache-2.0 | https://github.com/jeffparsons/rangemap |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/raw-window-handle |
| rayon | 1.12.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/rayon |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/rayon |
| regex | 1.13.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex | 1.13.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex (*) |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex (*) |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| reqwest | 0.13.5 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| rfd | 0.16.0 | MIT | https://github.com/PolyMeilex/rfd |
| ring | 0.17.14 | Apache-2.0 AND ISC | https://github.com/briansmith/ring |
| ring | 0.17.14 | Apache-2.0 AND ISC | https://github.com/briansmith/ring (*) |
| rle-decode-fast | 1.0.3 | MIT OR Apache-2.0 | https://github.com/WanzenBug/rle-decode-helper |
| rusqlite | 0.32.1 | MIT | https://github.com/rusqlite/rusqlite |
| rust-stemmers | 1.2.0 | MIT/BSD-3-Clause | https://github.com/CurrySoftware/rust-stemmers |
| rustc-hash | 1.1.0 | Apache-2.0/MIT | https://github.com/rust-lang-nursery/rustc-hash |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | https://github.com/rust-lang/rustc-hash |
| rustls | 0.23.45 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/rustls |
| rustls | 0.23.45 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/rustls (*) |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | https://github.com/rustls/pki-types |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | https://github.com/rustls/pki-types (*) |
| rustls-platform-verifier | 0.7.0 | MIT OR Apache-2.0 | https://github.com/rustls/rustls-platform-verifier |
| rustls-webpki | 0.103.15 | ISC | https://github.com/rustls/webpki |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | https://github.com/dtolnay/rustversion |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | https://github.com/dtolnay/ryu |
| same-file | 1.0.6 | Unlicense/MIT | https://github.com/BurntSushi/same-file |
| schemars | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| schemars_derive | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | https://github.com/bluss/scopeguard |
| selectors | 0.38.0 | MPL-2.0 | https://github.com/servo/stylo |
| semver | 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver |
| semver | 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver (*) |
| serde | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde (*) |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-untagged |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde (*) |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | https://github.com/serde-rs/json (*) |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-repr |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-repr (*) |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 | https://github.com/nox/serde_urlencoded |
| serde_with | 3.23.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serde_with_macros | 3.23.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| serde_with_macros | 3.23.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ (*) |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 | https://github.com/servo/stylo |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| sharded-slab | 0.1.7 | MIT | https://github.com/hawkw/sharded-slab |
| simd-adler32 | 0.3.10 | MIT | https://github.com/mcountryman/simd-adler32 |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | https://github.com/rusticstuff/simdutf8 |
| siphasher | 1.0.3 | MIT/Apache-2.0 | https://github.com/jedisct1/rust-siphash |
| sketches-ddsketch | 0.2.2 | Apache-2.0 | https://github.com/mheffner/rust-sketches-ddsketch |
| slab | 0.4.12 | MIT | https://github.com/tokio-rs/slab |
| smallvec | 1.16.1 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/socket2 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/socket2 (*) |
| softbuffer | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-windowing/softbuffer |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | https://github.com/storyyeller/stable_deref_trait |
| string_cache | 0.9.0 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| strsim | 0.11.1 | MIT | https://github.com/rapidfuzz/strsim-rs |
| subtle | 2.6.1 | BSD-3-Clause | https://github.com/dalek-cryptography/subtle |
| symlink | 0.1.0 | MIT/Apache-2.0 | https://gitlab.com/chris-morgan/symlink |
| syn | 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| syn | 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn (*) |
| syn | 3.0.6 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| syn | 3.0.6 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn (*) |
| sync_wrapper | 1.0.2 | Apache-2.0 | https://github.com/Actyx/sync_wrapper |
| sync_wrapper | 1.0.2 | Apache-2.0 | https://github.com/Actyx/sync_wrapper (*) |
| synstructure | 0.14.0 | MIT | https://github.com/mystor/synstructure |
| synstructure | 0.14.0 | MIT | https://github.com/mystor/synstructure (*) |
| tantivy | 0.22.1 | MIT | https://github.com/quickwit-oss/tantivy |
| tantivy-bitpacker | 0.6.0 | MIT | https://github.com/quickwit-oss/tantivy |
| tantivy-bitpacker | 0.6.0 | MIT | https://github.com/quickwit-oss/tantivy (*) |
| tantivy-columnar | 0.3.0 | MIT | https://github.com/quickwit-oss/tantivy |
| tantivy-common | 0.7.0 | MIT | https://github.com/quickwit-oss/tantivy |
| tantivy-common | 0.7.0 | MIT | https://github.com/quickwit-oss/tantivy (*) |
| tantivy-fst | 0.5.0 | Unlicense/MIT | https://github.com/quickwit-inc/fst |
| tantivy-fst | 0.5.0 | Unlicense/MIT | https://github.com/quickwit-inc/fst (*) |
| tantivy-query-grammar | 0.22.0 | MIT | https://github.com/quickwit-oss/tantivy |
| tantivy-sstable | 0.3.0 | MIT | https://github.com/quickwit-oss/tantivy |
| tantivy-stacker | 0.3.0 | MIT | https://github.com/quickwit-oss/tantivy |
| tantivy-stacker | 0.3.0 | MIT | https://github.com/quickwit-oss/tantivy (*) |
| tantivy-tokenizer-api | 0.3.0 | MIT | https://github.com/quickwit-oss/tantivy |
| tao | 0.37.1 | Apache-2.0 | https://github.com/tauri-apps/tao |
| tauri | 2.12.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri | 2.12.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri (*) |
| tauri-codegen | 2.7.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-macros | 2.7.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin-autostart | 2.6.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-dialog | 2.7.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-fs | 2.5.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-global-shortcut | 2.3.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-opener | 2.5.5 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-single-instance | 2.4.5 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-updater | 2.12.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-runtime | 2.12.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-runtime | 2.12.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri (*) |
| tauri-runtime-wry | 2.12.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-utils | 2.10.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-utils | 2.10.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri (*) |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | https://github.com/Stebalien/tempfile |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | https://github.com/Stebalien/tempfile (*) |
| tendril | 0.5.1 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| tendril | 0.5.1 | MIT OR Apache-2.0 | https://github.com/servo/html5ever (*) |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror (*) |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror (*) |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror (*) |
| thread_local | 1.1.10 | MIT OR Apache-2.0 | https://github.com/Amanieu/thread_local-rs |
| time | 0.3.55 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time | 0.3.55 | MIT OR Apache-2.0 | https://github.com/time-rs/time (*) |
| time-core | 0.1.9 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| tinystr | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/tinyvec |
| tokio | 1.53.1 | MIT | https://github.com/tokio-rs/tokio |
| tokio | 1.53.1 | MIT | https://github.com/tokio-rs/tokio (*) |
| tokio-rustls | 0.26.5 | MIT OR Apache-2.0 | https://github.com/rustls/tokio-rustls |
| tokio-rustls | 0.26.5 | MIT OR Apache-2.0 | https://github.com/rustls/tokio-rustls (*) |
| tokio-util | 0.7.19 | MIT | https://github.com/tokio-rs/tokio |
| toml | 1.1.6+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| tower | 0.5.3 | MIT | https://github.com/tower-rs/tower |
| tower | 0.5.3 | MIT | https://github.com/tower-rs/tower (*) |
| tower-http | 0.6.11 | MIT | https://github.com/tower-rs/tower-http |
| tower-http | 0.6.11 | MIT | https://github.com/tower-rs/tower-http (*) |
| tower-layer | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tower-service | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| tracing | 0.1.44 | MIT | https://github.com/tokio-rs/tracing |
| tracing | 0.1.44 | MIT | https://github.com/tokio-rs/tracing (*) |
| tracing-appender | 0.2.5 | MIT | https://github.com/tokio-rs/tracing |
| tracing-attributes | 0.1.31 | MIT | https://github.com/tokio-rs/tracing |
| tracing-core | 0.1.36 | MIT | https://github.com/tokio-rs/tracing |
| tracing-core | 0.1.36 | MIT | https://github.com/tokio-rs/tracing (*) |
| tracing-log | 0.2.0 | MIT | https://github.com/tokio-rs/tracing |
| tracing-subscriber | 0.3.23 | MIT | https://github.com/tokio-rs/tracing |
| tracing-subscriber | 0.3.23 | MIT | https://github.com/tokio-rs/tracing (*) |
| trash | 5.2.9 | MIT | https://github.com/ArturKovacs/trash |
| tray-icon | 0.25.1 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tray-icon |
| try-lock | 0.2.5 | MIT | https://github.com/seanmonstar/try-lock |
| type1-encoding-parser | 0.1.1 | MIT | https://github.com/jrmuizel/type1-encoding-parser |
| typeid | 1.0.3 | MIT OR Apache-2.0 | https://github.com/dtolnay/typeid |
| typenum | 1.20.1 | MIT OR Apache-2.0 | https://github.com/paholg/typenum |
| unicode-ident | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident |
| unicode-normalization | 0.1.25 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-normalization |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-segmentation |
| untrusted | 0.9.0 | ISC | https://github.com/briansmith/untrusted |
| url | 2.5.8 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| url | 2.5.8 | MIT OR Apache-2.0 | https://github.com/servo/rust-url (*) |
| urlpattern | 0.6.0 | MIT | https://github.com/denoland/rust-urlpattern |
| utf8-ranges | 1.0.5 | Unlicense/MIT | https://github.com/BurntSushi/utf8-ranges |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | https://github.com/hsivonen/utf8_iter |
| uuid | 1.26.1 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| uuid | 1.26.1 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid (*) |
| walkdir | 2.5.0 | Unlicense/MIT | https://github.com/BurntSushi/walkdir |
| walkdir | 2.5.0 | Unlicense/MIT | https://github.com/BurntSushi/walkdir (*) |
| want | 0.3.1 | MIT | https://github.com/seanmonstar/want |
| web-time | 1.1.0 | MIT OR Apache-2.0 | https://github.com/daxpedda/web-time |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| webpki-roots | 1.0.9 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots |
| webpki-roots | 1.0.9 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots (*) |
| webview2-com | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com | 0.39.1 | MIT | https://github.com/wravery/webview2-rs (*) |
| webview2-com-macros | 0.8.1 | MIT | https://github.com/wravery/webview2-rs |
| webview2-com-sys | 0.39.1 | MIT | https://github.com/wravery/webview2-rs |
| weezl | 0.1.12 | MIT OR Apache-2.0 | https://github.com/image-rs/weezl |
| winapi | 0.3.9 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| winapi-util | 0.1.11 | Unlicense OR MIT | https://github.com/BurntSushi/winapi-util |
| winapi-util | 0.1.11 | Unlicense OR MIT | https://github.com/BurntSushi/winapi-util (*) |
| window-vibrancy | 0.8.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri-plugin-vibrancy |
| windows | 0.58.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows | 0.61.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-collections | 0.2.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core | 0.58.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-future | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-future | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-implement | 0.58.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-interface | 0.58.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-link | 0.1.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-numerics | 0.2.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-registry | 0.6.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result | 0.2.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result | 0.2.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-result | 0.3.4 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-strings | 0.1.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings | 0.4.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-threading | 0.1.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-version | 0.1.7 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-version | 0.1.7 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs (*) |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| winnow | 1.0.4 | MIT | https://github.com/winnow-rs/winnow |
| winreg | 0.10.1 | MIT | https://github.com/gentoo90/winreg-rs |
| writeable | 0.6.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| wry | 0.57.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/wry |
| yoke | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| yoke | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| yoke-derive | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| yoke-derive | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| zerocopy | 0.8.58 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| zerofrom | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerofrom | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| zeroize | 1.9.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| zerotrie | 0.2.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerotrie | 0.2.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| zerovec | 0.11.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec | 0.11.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| zerovec-derive | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec-derive | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x (*) |
| zip | 2.4.2 | MIT | https://github.com/zip-rs/zip2.git |
| zip | 4.6.1 | MIT | https://github.com/zip-rs/zip2.git |
| zmij | 1.0.23 | MIT | https://github.com/dtolnay/zmij |
| zopfli | 0.8.3 | Apache-2.0 | https://github.com/zopfli-rs/zopfli |
| zstd | 0.13.3 | MIT | https://github.com/gyscos/zstd-rs |
| zstd | 0.13.3 | MIT | https://github.com/gyscos/zstd-rs (*) |
| zstd-safe | 7.3.0 | BSD-3-Clause | https://github.com/gyscos/zstd-rs |
| zstd-sys | 2.1.0+zstd.1.5.7 | BSD-3-Clause | https://github.com/gyscos/zstd-rs |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib | https://github.com/etemesi254/zune-image |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib | https://github.com/etemesi254/zune-image/tree/dev/crates/zune-jpeg |

## npm 生产依赖（8 项）

| 名称 | 版本 | 许可证 | 仓库 |
| --- | --- | --- | --- |
| @tauri-apps/api | 2.12.0 | Apache-2.0 OR MIT | git+https://github.com/tauri-apps/tauri.git |
| @tauri-apps/plugin-autostart | 2.6.0 | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| @tauri-apps/plugin-dialog | 2.7.3 | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| js-tokens | 4.0.0 | MIT | lydell/js-tokens |
| loose-envify | 1.4.0 | MIT | git://github.com/zertosh/loose-envify.git |
| react | 18.3.1 | MIT | https://github.com/facebook/react.git |
| react-dom | 18.3.1 | MIT | https://github.com/facebook/react.git |
| scheduler | 0.23.2 | MIT | https://github.com/facebook/react.git |
