# `tstick`

This is a cross-platform tool to automate the management of Telegram emojis and stickers.

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
