# 第三方作品 / Third-party notices

LeafStarter 自己的代码按 Apache-2.0 发布（见 `LICENSE`、`NOTICE`）。仓库里还包含下面这些第三方作品，按它们各自的许可使用。

LeafStarter's own code is licensed under Apache-2.0 (see `LICENSE` and `NOTICE`). This repository also contains the third-party works listed below, used under their own licenses.

编译出来的程序还带着 370 多个 Rust 依赖，它们的清单和许可原文由 `scripts/licenses.mjs`（cargo-about）生成在 [THIRD-PARTY-LICENSES.txt](THIRD-PARTY-LICENSES.txt)，随安装包和便携版一起发，程序的「关于」里也能看。
The compiled program also includes 370+ Rust crates; their list and license texts are generated into THIRD-PARTY-LICENSES.txt.

## Lucide 图标 / Lucide icons

- 用在哪里：`src/icons.js`、`proto/icons.js`、`site/public/demo/icons.js` 里的图标路径数据
- 来源：<https://lucide.dev>（<https://github.com/lucide-icons/lucide>）
- 许可：ISC；其中从 Feather 继承来的图标另按 MIT。原文如下：

```text
ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

---

The following Lucide icons are derived from the Feather project:

airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle, arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left, arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right, arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left, chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right, chevrons-up, circle, clipboard, clock, code, columns, command, compass, corner-down-left, corner-down-right, corner-left-down, corner-left-up, corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair, database, divide-circle, divide-square, dollar-sign, download, external-link, feather, frown, hash, headphones, help-circle, info, italic, key, layout, life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh, minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon, more-horizontal, more-vertical, move, music, navigation-2, navigation, octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio, rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square, table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type, upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out

The MIT License (MIT) (for the icons listed above)

Copyright (c) 2013-present Cole Bemis

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

## 编译后的程序 / Compiled builds

打包出来的安装包和便携版还包含 Rust 依赖（大多是 MIT、Apache-2.0，也有少量 MPL-2.0、BSD、ISC、Zlib、Unicode-3.0、CDLA-Permissive-2.0）。发布时由构建脚本自动生成完整的许可清单，放在安装目录里，并在「关于」页里能看到。

Installers and portable builds also contain Rust dependencies (mostly MIT and Apache-2.0, with a few under MPL-2.0, BSD, ISC, Zlib, Unicode-3.0 and CDLA-Permissive-2.0). Release builds ship a generated list of all of them, also shown on the About page.
