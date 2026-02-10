# Hikvision Thermal Wayland Viewer

Minimal Hikvision thermal camera viewer implemented in Rust with:

- `HCNetSDK` (`NET_DVR_Login_V40`, `NET_DVR_RealPlay_V40`)
- `PlayCtrl` decode callback (`PlayM4_SetDecCallBackMend`)
- Wayland-only windowing/rendering (`winit` Wayland feature + `softbuffer`)

## Fedora Dependencies (Wayland only)

```bash
sudo dnf install -y \
  pkgconf-pkg-config \
  wayland-devel wayland-protocols-devel libxkbcommon-devel \
  mesa-libEGL-devel mesa-libgbm-devel vulkan-loader-devel
```

Rust toolchain:

```bash
rustup toolchain install stable
```

## Run

From this repository root:

```bash
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1
```

Useful options:

```bash
HIK_PASSWORD='abcd1234' cargo run -- --help
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --link-mode 8
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --stream-type 1
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --sdk-root /path/to/another/sdk
```

Binary/package name:

```bash
hikvision-thermal-wayland-viewer
```

## Device Note

For an **AGM Taipan `TM10-256`** on this test bench, use:

- `IP`: `10.15.12.1`
- `username`: `admin`
- `password`: `abcd1234`

## Notes

- This target is Wayland-only at compile time (`winit` `default-features = false`, `features = ["wayland"]`).
- The project vendors the SDK under `vendor/linux/x86_64/hik_sdk`.
- Default `--sdk-root` is `<crate>/vendor/linux/x86_64/hik_sdk` (set in code); override with `--sdk-root` if needed.
- At startup, the binary re-execs itself once with `LD_LIBRARY_PATH` pointing to the vendored SDK so dynamic dependencies resolve under plain `cargo run`.
- The binary also preloads required native dependencies from vendored absolute paths before loading HCNetSDK/PlayCtrl.
- Cargo runtime env defaults are in `.cargo/config.toml`:
  - `HIK_SDK_ROOT` -> `<crate>/vendor/linux/x86_64/hik_sdk`
- Vendor layout is platform-scoped to support future targets:
  - `vendor/linux/x86_64/hik_sdk`
  - Add additional SDK bundles under sibling directories (for example `vendor/linux/aarch64/...`) and point `HIK_SDK_ROOT` or `--sdk-root` to the desired one.
- Press `Esc`, `q`, or close the window to stop the viewer.
