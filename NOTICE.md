# Notices

Pixel Pet is Copyright (c) 2026 Shreeniwas (Shreeniwas1) and Veer Pratap Singh (VeerDev-hub). The source code is licensed under the GNU General
Public License v3.0 or later (see `LICENSE`). The pixel art is licensed under CC BY 4.0 (see
`LICENSE-ART.md`).

Pixel Pet is original work. All bundled content (`data/facts.txt` and the pixel art) was written
or drawn for this project. All
dependencies below are MIT or Apache-2.0, which are compatible with the GPL.

Character packs that you import are not part of Pixel Pet. Whoever makes or shares one answers for the
rights to what is in it.

Rust crates used: `windows-sys` (MIT OR Apache-2.0), `ruzstd` (MIT, text below). On Linux also
`x11rb` (MIT OR Apache-2.0) and `fontdue` (MIT OR Apache-2.0 OR Zlib), on macOS `objc2` (MIT) and `fontdue`.

The Linux and macOS versions contain the font **Fira Mono**, Copyright (c) 2012-2013 The Mozilla
Corporation and Telefonica S.A., under the SIL Open Font License 1.1 (`assets/fonts/FiraMono-OFL.txt`).

## AI token usage

The way Codex CLI session logs are turned into token counts (cumulative totals become deltas,
cached tokens are a subset of input, reasoning tokens a subset of output, out-of-order snapshots
are skipped) follows the approach of **tokscale** by Junho Yeo, <https://github.com/junhoyeo/tokscale>.
The OpenCode part (which tables and JSON fields hold the numbers: `message` for v1, `session_message` for v2) was learned from tokscale too. No tokscale source file is included; `src/tokens.rs` and `src/opencode.rs` are our own compact rewrites. OpenCode's database is read through the `winsqlite3.dll` that ships with Windows, so no database library is bundled. tokscale is
MIT licensed and its notice is reproduced here as that licence asks:

```
MIT License

Copyright (c) 2025 Junho Yeo

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## ruzstd

Used to read Codex's compressed session logs.

```
MIT License

Copyright (c) 2019 Moritz Borcherding

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
