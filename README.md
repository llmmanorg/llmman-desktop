# llmman-desktop

[llmman](https://github.com/llmmanorg/llmman)'s [web UI](https://github.com/llmmanorg/llmman/blob/main/docs/webui.md)
in a native window. Chat, pull models, generate images, video and audio, and
the Shell tab: everything `llmman serve` shows at `http://127.0.0.1:17434/`,
without a browser tab.

Builds on llmman rather than re-implementing any of it. Each package ships
the llmman release binary for its platform next to the app. On launch:

- if a daemon already answers on `127.0.0.1:17434` (one the CLI started,
  say), the window uses it and leaves it running when closed;
- otherwise the app runs the `llmman serve` next to it (or else the one on
  `PATH`) and shows its progress. On a first run that includes fetching
  llama.cpp, which can take minutes. Closing the window stops the daemon.
  A daemon that exits shows its log and a *Retry* button. The log is at
  `llmman-desktop-serve.log` in the system temp directory.

Links to other sites open in the system browser; downloads (*Export chats*,
generated media) go to the Downloads folder.

## Install

From the [latest release](https://github.com/llmmanorg/llmman-desktop/releases/latest),
for the same five platforms as llmman:

| Platform | Asset |
| --- | --- |
| macOS, Apple Silicon | `llmman-desktop-aarch64-apple-darwin.zip` (`llmman.app`) |
| Linux x86_64 | `llmman-desktop-x86_64-unknown-linux-gnu.tar.gz` |
| Linux aarch64 | `llmman-desktop-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86_64 | `llmman-desktop-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `llmman-desktop-aarch64-pc-windows-msvc.zip` |

`checksums.txt` in the same release covers them all.

- **macOS:** unzip and move `llmman.app` to Applications. The app is
  ad-hoc signed, not notarized, so the first launch needs right-click → *Open*
  (or `xattr -dr com.apple.quarantine llmman.app`).
- **Linux:** extract and run `llmman-desktop/llmman-desktop`. It needs
  WebKitGTK 4.1 and GTK 3 (`libwebkit2gtk-4.1-0` on Debian/Ubuntu,
  `webkit2gtk4.1` on Fedora).
- **Windows:** extract and run `llmman-desktop\llmman-desktop.exe`. It uses
  the WebView2 runtime, which Windows 10 and 11 already have.

Keep `llmman-desktop` and `llmman` together: the app runs the `llmman` next
to it.

## Building

Rust (stable), plus WebKitGTK and GTK development headers on Linux
(`libwebkit2gtk-4.1-dev libgtk-3-dev`):

```sh
cargo build --release
./target/release/llmman-desktop   # uses the llmman on PATH
```

`packaging/package.sh <target> <version> <llmman-tag>` bundles a release build
with that llmman release into `dist/`, as CI does. As in llmman, every commit
on `main` is a release, versioned `MAJOR.MINOR.<commit count>`
(`packaging/version.sh`).
