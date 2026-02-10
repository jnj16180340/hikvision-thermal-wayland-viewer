Vendored SDK layout is platform-scoped:

- `linux/x86_64/hik_sdk`

Add other platforms as sibling paths, for example:

- `linux/aarch64/hik_sdk`
- `macos/arm64/hik_sdk`

Select the active SDK bundle using `HIK_SDK_ROOT` or `--sdk-root`.
