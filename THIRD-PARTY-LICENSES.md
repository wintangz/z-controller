# Third-Party Licenses

This file lists the licenses and copyright notices for third-party components bundled with or linked into `z-controller`.

## Bundled Components

### SDL3 (zlib License)

Source: https://github.com/libsdl-org/SDL (vendored from `vendor/sdl3-win64/LICENSE.txt`)

```text
Copyright (C) 1997-2026 Sam Lantinga <slouken@libsdl.org>
  
This software is provided 'as-is', without any express or implied
warranty.  In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:
  
1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required. 
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.
```

### SDL_GameControllerDB (gamecontrollerdb) (zlib License)

Source: https://github.com/mdqinc/SDL_GameControllerDB (LICENSE)

```text
Copyright (C) 1997-2025 Sam Lantinga <slouken@libsdl.org>
  
This software is provided 'as-is', without any express or implied
warranty.  In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:
  
1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required. 
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.
```

### ViGEmBus (BSD-3-Clause)

Bundled installer: `assets/drivers/ViGEmBus_Setup.exe` (v1.22.0, Copyright (C) 2023 Nefarius Software Solutions e.U.)
Source: https://github.com/nefarius/ViGEmBus (LICENSE)

```text
BSD 3-Clause License

Copyright (c) 2016-2020, Nefarius Software Solutions e.U.
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### HidHide (MIT License)

Bundled installer: `assets/drivers/HidHide_Setup.exe` (v1.5.230, Copyright (C) 2024 Nefarius Software Solutions e.U.)
Source: https://github.com/nefarius/HidHide (LICENSE)

```text
MIT License

Copyright (c) 2020 Eric Korff de Gidts
Copyright (c) 2021-2024 Benjamin Höglinger-Stelzer

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## Rust Crate Dependencies

Generated from `Cargo.lock` and `cargo metadata` (66 third-party crates; the 4 workspace crates `z-controller`, `z-core`, `z-platform-windows`, and `z-sdl3` are licensed under `MIT`, totaling 70 packages in `Cargo.lock`).

| Crate | Version | License |
| --- | --- | --- |
| `ab_glyph` | `0.2.32` | Apache-2.0 |
| `ab_glyph_rasterizer` | `0.1.10` | Apache-2.0 |
| `ahash` | `0.8.13` | MIT OR Apache-2.0 |
| `bitflags` | `2.13.2` | MIT OR Apache-2.0 |
| `bumpalo` | `3.20.4` | MIT OR Apache-2.0 |
| `cfg-if` | `1.0.5` | MIT OR Apache-2.0 |
| `ecolor` | `0.29.1` | MIT OR Apache-2.0 |
| `egui` | `0.29.1` | MIT OR Apache-2.0 |
| `emath` | `0.29.1` | MIT OR Apache-2.0 |
| `epaint` | `0.29.1` | MIT OR Apache-2.0 |
| `epaint_default_fonts` | `0.29.1` | (MIT OR Apache-2.0) AND OFL-1.1 AND LicenseRef-UFL-1.0 |
| `equivalent` | `1.0.3` | Apache-2.0 OR MIT |
| `getrandom` | `0.4.3` | MIT OR Apache-2.0 |
| `hashbrown` | `0.17.2` | MIT OR Apache-2.0 |
| `indexmap` | `2.14.2` | Apache-2.0 OR MIT |
| `itoa` | `1.0.19` | MIT OR Apache-2.0 |
| `js-sys` | `0.3.107` | MIT OR Apache-2.0 |
| `libc` | `0.2.191` | MIT OR Apache-2.0 |
| `lock_api` | `0.4.14` | MIT OR Apache-2.0 |
| `memchr` | `2.8.3` | Unlicense OR MIT |
| `nohash-hasher` | `0.2.0` | Apache-2.0 OR MIT |
| `once_cell` | `1.21.4` | MIT OR Apache-2.0 |
| `owned_ttf_parser` | `0.25.1` | Apache-2.0 |
| `parking_lot` | `0.12.3` | MIT OR Apache-2.0 |
| `parking_lot_core` | `0.9.10` | MIT OR Apache-2.0 |
| `proc-macro2` | `1.0.107` | MIT OR Apache-2.0 |
| `quote` | `1.0.47` | MIT OR Apache-2.0 |
| `r-efi` | `6.0.0` | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| `redox_syscall` | `0.5.18` | MIT |
| `rustversion` | `1.0.23` | MIT OR Apache-2.0 |
| `scopeguard` | `1.2.0` | MIT OR Apache-2.0 |
| `serde` | `1.0.229` | MIT OR Apache-2.0 |
| `serde_core` | `1.0.229` | MIT OR Apache-2.0 |
| `serde_derive` | `1.0.229` | MIT OR Apache-2.0 |
| `serde_json` | `1.0.154` | MIT OR Apache-2.0 |
| `serde_spanned` | `0.6.9` | MIT OR Apache-2.0 |
| `smallvec` | `1.16.6` | MIT OR Apache-2.0 |
| `syn` | `2.0.119` | MIT OR Apache-2.0 |
| `syn` | `3.0.10` | MIT OR Apache-2.0 |
| `thiserror` | `2.0.21` | MIT OR Apache-2.0 |
| `thiserror-impl` | `2.0.21` | MIT OR Apache-2.0 |
| `toml` | `0.8.23` | MIT OR Apache-2.0 |
| `toml_datetime` | `0.6.11` | MIT OR Apache-2.0 |
| `toml_edit` | `0.22.27` | MIT OR Apache-2.0 |
| `toml_write` | `0.1.2` | MIT OR Apache-2.0 |
| `ttf-parser` | `0.25.1` | MIT OR Apache-2.0 |
| `unicode-ident` | `1.0.27` | (MIT OR Apache-2.0) AND Unicode-3.0 |
| `version_check` | `0.9.5` | MIT/Apache-2.0 |
| `wasm-bindgen` | `0.2.130` | MIT OR Apache-2.0 |
| `wasm-bindgen-macro` | `0.2.130` | MIT OR Apache-2.0 |
| `wasm-bindgen-macro-support` | `0.2.130` | MIT OR Apache-2.0 |
| `wasm-bindgen-shared` | `0.2.130` | MIT OR Apache-2.0 |
| `windows-sys` | `0.59.0` | MIT OR Apache-2.0 |
| `windows-targets` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_aarch64_gnullvm` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_aarch64_msvc` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_i686_gnu` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_i686_gnullvm` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_i686_msvc` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_x86_64_gnu` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_x86_64_gnullvm` | `0.52.6` | MIT OR Apache-2.0 |
| `windows_x86_64_msvc` | `0.52.6` | MIT OR Apache-2.0 |
| `winnow` | `0.7.15` | MIT |
| `zerocopy` | `0.8.65` | BSD-2-Clause OR Apache-2.0 OR MIT |
| `zerocopy-derive` | `0.8.65` | BSD-2-Clause OR Apache-2.0 OR MIT |
| `zmij` | `1.0.23` | MIT |
