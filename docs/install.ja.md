[English](install.md) | **日本語**

# インストール

## 前提パッケージ

```bash
# Ubuntu/Debian
sudo apt install poppler-utils libopencv-dev clang libclang-dev

# macOS (Homebrew)
brew install poppler opencv pkg-config

# Fedora/RHEL
sudo dnf install poppler-utils opencv-devel clang clang-devel
```

**対応する OpenCV のバージョンは 4.x（4.11 と 4.13 で確認済み）．** OpenCV は動的リンク
なので，CLI をインストールする前に [OpenCV のリンク](#opencv-のリンク) を読むこと．

## ライブラリとして使う

```bash
cargo add rsrpp
```

OpenCV 無しでビルドする場合（表領域の検出が無くなる —
[`table-detection` フィーチャ](#table-detection-フィーチャ) を参照）:

```bash
cargo add rsrpp --no-default-features
```

## CLI ツールとして使う

```bash
cargo install rsrpp-cli
```

## OpenCV のリンク

RSRPP は OpenCV を**動的に**リンクする．macOS では記録される依存関係が OpenCV の soname
（`libopencv_core.413.dylib`）を含むため，Homebrew で OpenCV を上げるとインストール済み
バイナリが要求するライブラリが消え，CLI は `main()` に入る前に死ぬ:

```
dyld: Library not loaded: /opt/homebrew/opt/opencv/lib/libopencv_gapi.411.dylib
  Referenced from: ~/.cargo/bin/rsrpp
```

これは RSRPP の内部で起きたクラッシュではない．先に動的リンカが失敗するので，RSRPP のコードは
1 行も走らない（したがってこちらのエラーメッセージも出ない）．対処は 3 通りある．

**1. OpenCV を上げたら入れ直す**（いちばん簡単）:

```bash
cargo install rsrpp-cli --force
```

**2. RSRPP が実際に使う OpenCV モジュールだけをリンクする．** 既定では `opencv` crate は
`pkg-config --libs opencv4` が報告するものをすべてリンクする — Homebrew では 56 個で，
`gapi`・`dnn`・`cuda*` のように RSRPP が一切呼ばないものまで含まれる．リンク対象を実際に
使う 3 つに絞れば，アップグレードで壊れうるライブラリの数が減る:

```bash
OPENCV_LINK_LIBS=opencv_core,opencv_imgproc,opencv_imgcodecs \
OPENCV_LINK_PATHS=+ OPENCV_INCLUDE_PATHS=+ \
  cargo install rsrpp-cli --force
```

（`+` は「自動検出したパスに追記する」という意味なので，Homebrew でも apt でも dnf でも
そのまま使える．）OpenCV のメジャーバージョンが上がったときに入れ直す必要は残るが，ビルドは
速くなり，露出するライブラリは 56 個ではなく 3 個になる．

**3. OpenCV をまったく使わずにビルドする** — OpenCV のアップグレードの影響を受けない:

```bash
cargo install rsrpp-cli --no-default-features
```

## `table-detection` フィーチャ

OpenCV を引き込むのは `table-detection`（既定で有効）だけである．これは各ページの画像に
Hough 変換をかけて直線を見つけ，表の領域を特定して，その中身が本文に混ざらないようにする．

`--no-default-features` を付けると，バイナリには **OpenCV へのリンクが一切無くなる** —
macOS なら `otool -L $(which rsrpp) | grep opencv`，Linux なら
`ldd $(which rsrpp) | grep opencv` で確認でき，何も表示されないはずである．引き換えに表の
領域は検出されなくなるので，罫線付きの表の中のテキストは除外されず，周囲の本文の節の一部として
出力される．

## ソースからビルドする

```bash
git clone https://github.com/akitenkrad/rsrpp.git
cd rsrpp
# 上記の前提パッケージ（poppler・OpenCV・pkg-config）を入れてから:
makers build
makers nextest
```
