# `tstick`

This is a cross-platform tool to automate the management of Telegram emojis and stickers.

# Demo

Here is a quick demo that displays how `tstick` can be used to generate a video emoji.

![](https://user-images.githubusercontent.com/36276403/229662583-649da937-c4cd-4d0d-ba36-b67f939afe5a.gif)


Specify the input file and any additional options to `ffmpeg`, and `tstick` will find the best quality options (CRF) to fit into Telegram's emoji/sticker file size limits. It automatically resizes the image to fit into 100x100 or 512x512 for emoji and for sticker respectively according to the source video's largest side.

# Install

You can download the `tstick` binary from our [Github Releases](https://github.com/Veetaha/tstick/releases).

# Build

To build it you need to have the [rust toolchain](https://www.rust-lang.org/tools/install) installed.

Use `cargo` to build from sources (add `--release` flag to build with optimizations):

```
cargo build
```

The output binary will be available at `target/(debug|release)/tstick[.exe]` depending
on the build mode and the operating system (`.exe` is added on Windows).

To build and run the application the following command can be used:

```
cargo run -- --help
```

You can pass options to the application after the first `--`.

# Usage

See the `--help` output of the tool, which should guide you on what commands are available.

```py
tstick --help
```
```
A tool that automates the management of telegram stickers and emojis

Usage: tstick <COMMAND>

Commands:
  video  Generate telegram emoji or sticker from a video using ffmpeg
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```py
tstick video --help
```
```
Generate telegram emoji or sticker from a video using ffmpeg

The output file will be put into the same directory where the input file is located, but with name `emoji.webm` or `sticker.webm` by default.

This command implements the two-pass method described in the following docs: <https://trac.ffmpeg.org/wiki/Encode/VP9>

Usage: tstick video [OPTIONS] <--emoji|--sticker> [FFMPEG_ARGS]...

Arguments:
  [FFMPEG_ARGS]...
          Additional arguments that will be passed to ffmpeg between the input and output args. Beware that they may break the internal logic of generating the `ffmpeg` command. For example, if you need additional video filter use `--filter` flag instead

Options:
      --emoji
          Generate an emoji WEBM file

      --sticker
          Generate a sticker WEBM file

  -i, --input <INPUT>
          Path to the input media file(s) or directory(ies) containing media files to be processed.

          If the input is a directory, all files inside of it will be processed. Make sure there are no other files in the directory other than the ones to generate emoji/stickers for.

  -o, --output <OUTPUT>
          Path to the output directory where the generated emoji/stickers will be put. The output files will be named after the input files using the following pattern:

          `{input_file_name}.emoji.webm` or `{input_file_name}.sticker.webm`

          If this options is not specified, the output files will be put into the same directories where the input files are located, even if they are in different directories.

          Make sure all input file names are unique, otherwise there will be conflicts when writing to the output directory.

      --overwrite
          Overwrite the output files if they already exist, without asking for confirmation

      --publisher <PUBLISHER>
          Set the `publisher` metadata of the generated emoji/sticker WEBM file. It is recommended to set this to the URL of the Telegram channel or other resource where emojis/stickers are promoted. This helps with keeping the source of the emoji/sticker file even if it's copied to another pack by someone else

      --begin <BEGIN>
          The time from which the video will be cut.

          The total video duration must not exceed 3 seconds.

      --end <END>
          The time to which the video will be cut.

          The total video duration must not exceed 3 seconds.

      --filter <FILTER>
          The value of the video filter flag that will be passed to ffmpeg before rescaling it to the needed size

      --concurrency <CONCURRENCY>
          Maximum number of inputs to be proceesed in parallel

          [default: {PLATFORM_SPECIFIC}]

  -h, --help
          Print help (see a summary with '-h')
```
