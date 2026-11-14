# Hikvision Thermal Wayland Viewer

Minimal Hikvision thermal camera viewer implemented in Rust with:

- `HCNetSDK` (`NET_DVR_Login_V40`, `NET_DVR_RealPlay_V40`)
- `PlayCtrl` decode callback (`PlayM4_SetDecCallBackMend`)
- Wayland-only windowing/rendering (`winit` Wayland feature + `softbuffer`)
- Headless GStreamer `appsrc` RTSP publishing mode

## Fedora Dependencies (Wayland only)

```bash
sudo dnf install -y \
  pkgconf-pkg-config \
  wayland-devel wayland-protocols-devel libxkbcommon-devel \
  mesa-libEGL-devel mesa-libgbm-devel vulkan-loader-devel \
  gstreamer1-devel gstreamer1-plugins-base-devel gstreamer1-rtsp-server-devel \
  gstreamer1-plugins-good gstreamer1-plugins-bad-free \
  gstreamer1-plugins-ugly-free
```

Rust toolchain:

```bash
rustup toolchain install stable
```

## Connect to device wifi

By default, the password is the serial number. So, network `Wlan-L31278659` has password `L31278659`

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
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --discover
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --discover --discover-report-dir runtime_logs/discovery_reports
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --focus-style SEMIAUTOMATIC --noise-reduction-mode general --digital-zoom 2x --palette WhiteHot --image-mode standard --scene normal
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --osd-date-time-enabled false --osd-date-display-enabled false --osd-channel-name-enabled false --osd-battery-enabled false
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --osd-snapshot
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --osd-snapshot --osd-date-time-enabled false
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --headless-publish-rtsp
HIK_PASSWORD='abcd1234' cargo run -- --ip 10.15.12.1 --headless-publish-rtsp --publish-rtsp-address 0.0.0.0 --publish-rtsp-port 8554 --publish-rtsp-path /hik
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
- Discovery is gated by `--discover`. When enabled, the app prints an SDK/ISAPI introspection summary and writes a JSON report under `runtime_logs/discovery_reports` by default.
- Use `--discover-report-dir` to override the report output directory.
- Supported image-control CLI options (current ISAPI surface) are:
  - `--focus-style` (`AUTO`, `MANUAL`, `SEMIAUTOMATIC`)
  - `--noise-reduction-mode` (`close`, `general`, `advanced`)
  - `--digital-zoom` (`1x`, `2x`, `4x`, `8x`)
  - `--palette` (`WhiteHot`, `BlackHot`, `RedHot`, `Fusion1`)
  - `--image-mode` (`standard`, `indoor`, `outdoor`, `dimLight`)
  - `--scene` (free-form string; accepted values are device/firmware-specific)
- Supported OSD-control CLI options (grouped under `OSD Controls` in `--help`) are:
  - `--osd-date-time-enabled` (`true|false`)
  - `--osd-date-display-enabled` (`true|false`)
  - `--osd-channel-name-enabled` (`true|false`)
  - `--osd-text-enabled` (`true|false`, applies to all existing text overlay entries)
  - `--osd-battery-enabled` (`true|false`)
  - `--osd-device-status-battery-enabled` (`true|false`)
  - `--osd-device-status-charging-enabled` (`true|false`)
  - `--osd-device-status-bluetooth-enabled` (`true|false`)
  - `--osd-device-status-dial-enabled` (`true|false`)
  - `--osd-ptz-zoom-label` (`2sec|5sec|10sec|alwaysclose|alwaysopen`)
- OSD snapshot options (grouped under `OSD Snapshot` in `--help`) are:
  - `--osd-snapshot` (capture OSD endpoint snapshots before and after applying controls)
  - `--osd-snapshot-dir` (default `runtime_logs/osd_snapshots`)
- These controls are currently known to work on image channel `1` on the Taipan test unit.
- Vendor layout is platform-scoped to support future targets:
  - `vendor/linux/x86_64/hik_sdk`
  - Add additional SDK bundles under sibling directories (for example `vendor/linux/aarch64/...`) and point `HIK_SDK_ROOT` or `--sdk-root` to the desired one.
- Press `Esc`, `q`, or close the window to stop the viewer.
- Headless publishing options (grouped under `Headless Publish` in `--help`) are:
  - `--headless-publish-rtsp` (enable headless mode; no Wayland window created)
  - `--publish-rtsp-address` (default `0.0.0.0`)
  - `--publish-rtsp-port` (default `8554`)
  - `--publish-rtsp-path` (default `/hik`)
  - `--publish-fps` (default `25`)
  - `--publish-bitrate-kbps` (default `2048`)
  - `--publish-wait-for-first-frame-ms` (default `10000`)
- When headless RTSP mode is enabled, connect clients to:
  - `rtsp://<publisher-host>:<publish-rtsp-port><publish-rtsp-path>`
  - If bind address is `0.0.0.0`, do not use `0.0.0.0` in VLC; use `127.0.0.1` (local) or the host LAN IP.

## Container

Headless RTSP publisher image (Fedora base, vendored x86_64 SDK baked in):

```bash
podman build -t hik-bridge .
podman run --rm -e HIK_PASSWORD='abcd1234' -p 8554:8554 hik-bridge \
  --ip 10.15.12.1 --headless-publish-rtsp
```

The stream is then at `rtsp://localhost:8554/hik`. Used by the NVR stack in the parent MotionCamera repo.
