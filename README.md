# `tstick`

This is a cross-platform tool to automate the management of Telegram emojis and stickers.

# Demo

Here is a quick demo that displays how `tstick` can be used to generate a video emoji and a sticker.

Specify the input file and any additional options to `ffmpeg`, and `tstick` will find the best quality options (CRF) to fit into Telegram's emoji/sticker file size limits

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

See the `--help` output of the tool, which should guide you on what commands are available.

```py
tstick --help
```
```
A tool that automates the management of telegram stickers and emojis

Usage: tstick <COMMAND>

Commands:
  video  Generate telegram emoji and sticker from a video using ffmpeg
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```py
tstick video --help
```
```
Generate telegram emoji and sticker from a video using ffmpeg

The output files will be put into the same directory where the input file is located, but with names `emoji.webm` and `sticker.webm` respectively.

This command implements the two-pass method described in the following docs: <https://trac.ffmpeg.org/wiki/Encode/VP9>

Usage: tstick video [OPTIONS] --input <INPUT> [FFMPEG_ARGS]...

Arguments:
  [FFMPEG_ARGS]...
          Arguments that will be passed to ffmpeg between the input and output args

Options:
  -i, --input <INPUT>
          Path to the input file to convert into a sticker and emoji

      --no-emoji
          Don't generate an emoji

      --no-sticker
          Don't generate a sticker

      --start-crf <START_CRF>
          Set a custom CRF value to start search for the most optimal one from

          [default: 18]

  -h, --help
          Print help (see a summary with '-h')
```
