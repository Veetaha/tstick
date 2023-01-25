# `tstick`

This is a cross-platform tool to automate the management of Telegram emojis and stickers.

# Demo

Here is a quick demo that displays how `tstick` can be used to generate a video emoji and a sticker.

Specify the input file and any additional options to `ffmpeg`, and `tstick` will do its best to find the best quality options (CRF) to fit into Telegram's emoji/sticker file size limits

![](https://user-images.githubusercontent.com/36276403/214474683-9e0566cb-86ba-48e8-b486-234a4547e5f4.gif)

# Install

You can download the `tstick` binary from our [Github Releases](https://github.com/Veetaha/tstick/releases).

# Build

To build it you need to have the [rust toolchain] installed.

Use `cargo` to build from sources (add `--release` flag to build with optimizations):

```
cargo build
```

The output binary will be available at `target/(debug|release)/tstick[.exe]` depending
on the build mode and the operating system (`.exe` is added on Windows).

# Usage

Build the tool and run it with the `--help` flag like this:

```
cargo run -- --help
```

You can pass any arguments to the too after the `cargo run --`.
See the commands and agruments available in the help message and go from there.

[rust toolchain]: https://www.rust-lang.org/tools/install
