# UnstuckSpotify

A small cross-platform GUI app (Rust + egui) that fixes a Spotify hang/crash by:

- **Step 1 — Clear cache (safe):** quits Spotify, deletes its cache, keeps your login and playlists.
- **Step 2 — Full reset:** quits Spotify, deletes app data, preferences, and caches. You'll be logged out; playlists re-sync automatically from the cloud.

## Download prebuilt binaries (no Rust needed)

1. Go to the [Releases page](../../releases) of this repo.
2. Download the file for your platform:
   - **macOS (Apple Silicon):** `UnstuckSpotify-macos-arm64.tar.gz`
   - **macOS (Intel):** `UnstuckSpotify-macos-intel.tar.gz`
   - **Windows:** `UnstuckSpotify-windows-x64.zip`
   - **Linux:** `UnstuckSpotify-linux-x64.tar.gz`
3. Extract and run:
   - **macOS:** `tar -xzf UnstuckSpotify-macos-arm64.tar.gz && ./unstuck_spotify`
   - **Windows:** unzip, then double-click `unstuck_spotify.exe`
   - **Linux:** `tar -xzf UnstuckSpotify-linux-x64.tar.gz && ./unstuck_spotify`

Each download has a `.sha256` checksum file to verify integrity.

> **macOS note:** the binary is not notarized, so macOS may block it. Right-click
> the app → Open, or run `xattr -d com.apple.quarantine ./unstuck_spotify` once.

## Building from source

You need [Rust](https://rustup.rs) installed.

```bash
cargo build --release
# binary is at target/release/unstuck_spotify
```

Run it:

```bash
./target/release/unstuck_spotify        # macOS / Linux
.\target\release\unstuck_spotify.exe    # Windows
```

The binary is fully self-contained — no runtime dependencies, no installer.

### Per-platform build notes

| Platform | Command | Extra dependencies |
|---|---|---|
| **macOS** (arm64 or Intel) | `cargo build --release` | None — Xcode CLT (`xcode-select --install`) |
| **Windows** | `cargo build --release` | MSVC Build Tools (installed with Visual Studio) |
| **Windows** (from Linux/macOS) | `cargo build --release --target x86_64-pc-windows-gnu` | mingw-w64 (`brew install mingw-w64` / `apt install mingw-w64`) |
| **Linux** | `cargo build --release` | GTK dev packages: `sudo apt install libgtk-3-dev libxkbcommon-dev libx11-dev libwayland-dev` |

### Building for other architectures (optional)

```bash
# macOS Intel binary from an Apple Silicon Mac (or vice versa)
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Linux x64
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu
```

## Releasing a new version

Releases are automated with GitHub Actions. When you push a tag, the workflow
builds macOS (arm64 + Intel), Windows, and Linux binaries and attaches them to
a GitHub Release automatically:

```bash
git tag v1.0.0
git push origin v1.0.0
```

The workflow lives at `.github/workflows/release.yml`.

## Notes

- Step 1 always runs first if you want to try the safe fix. Step 2 is only needed if Step 1 doesn't help.
- Playlists are stored in Spotify's cloud, so they're never lost — Step 2 only removes local data.
- If Spotify still hangs after Step 2, the app itself is corrupted — re-download it from https://www.spotify.com/download.
- Paths used per OS:
  - **macOS:** `~/Library/Caches/com.spotify.client`, `~/Library/Application Support/Spotify`, `~/Library/Preferences/com.spotify.client.plist`
  - **Windows:** `%APPDATA%\Spotify`, `%LOCALAPPDATA%\Spotify`
  - **Linux:** `~/.config/spotify`, `~/.cache/spotify`
