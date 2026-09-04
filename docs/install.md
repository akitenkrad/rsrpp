**English** | [日本語](install.ja.md)

# Installation

## Prerequisites

```bash
# Ubuntu/Debian
sudo apt install poppler-utils libopencv-dev clang libclang-dev

# macOS (Homebrew)
brew install poppler opencv pkg-config

# Fedora/RHEL
sudo dnf install poppler-utils opencv-devel clang clang-devel
```

**Supported OpenCV versions: 4.x (tested against 4.11 and 4.13).** OpenCV is linked
dynamically, so read [OpenCV linking](#opencv-linking) before installing the CLI.

## As a library

```bash
cargo add rsrpp
```

To build without OpenCV (no table-region detection — see
[the `table-detection` feature](#the-table-detection-feature)):

```bash
cargo add rsrpp --no-default-features
```

## As a CLI tool

```bash
cargo install rsrpp-cli
```

## OpenCV linking

RSRPP links OpenCV **dynamically**. On macOS the recorded dependency carries the
OpenCV soname (`libopencv_core.413.dylib`), so a Homebrew OpenCV upgrade removes the
library the installed binary asks for and the CLI dies before `main()`:

```
dyld: Library not loaded: /opt/homebrew/opt/opencv/lib/libopencv_gapi.411.dylib
  Referenced from: ~/.cargo/bin/rsrpp
```

This is not a crash inside RSRPP — the dynamic loader fails first, so no RSRPP code
(and no error message of ours) can run. There are three ways to deal with it:

**1. Reinstall after an OpenCV upgrade** (simplest):

```bash
cargo install rsrpp-cli --force
```

**2. Link only the OpenCV modules RSRPP uses.** By default the `opencv` crate links
everything `pkg-config --libs opencv4` reports — 56 libraries on Homebrew, including
`gapi`, `dnn` and `cuda*`, none of which RSRPP calls. Restricting the link set to the
three that are actually used cuts the number of libraries that an upgrade can break:

```bash
OPENCV_LINK_LIBS=opencv_core,opencv_imgproc,opencv_imgcodecs \
OPENCV_LINK_PATHS=+ OPENCV_INCLUDE_PATHS=+ \
  cargo install rsrpp-cli --force
```

(`+` means "append to the auto-detected paths", so this stays portable across
Homebrew, apt and dnf.) You still need to reinstall on a major OpenCV bump, but the
build is faster and the exposure is 3 libraries instead of 56.

**3. Build without OpenCV entirely** — immune to OpenCV upgrades:

```bash
cargo install rsrpp-cli --no-default-features
```

## The `table-detection` feature

`table-detection` (enabled by default) is the only thing that pulls in OpenCV. It
runs a Hough-line transform over each page image to find table regions so their
contents are kept out of the body text.

With `--no-default-features` the binary has **no OpenCV linkage at all** — verify with
`otool -L $(which rsrpp) | grep opencv` (macOS) or `ldd $(which rsrpp) | grep opencv`
(Linux), which should print nothing. The trade-off: table regions are no longer
detected, so text inside ruled tables is emitted as part of the surrounding body
section instead of being excluded.

## Building from source

```bash
git clone https://github.com/akitenkrad/rsrpp.git
cd rsrpp
# Install the prerequisites above (poppler, OpenCV, pkg-config), then:
makers build
makers nextest
```
