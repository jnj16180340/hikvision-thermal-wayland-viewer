# Progress Checkpoint

Date: 2026-02-11

## Progress

- Confirmed current branch state after reverting latency-profile changes.
- Added unit tests in `src/main.rs` for core pure helpers used by the viewer and discovery logic:
  - RTSP path/host normalization helpers.
  - XML value extraction helpers.
  - XML tag replacement helpers.
  - ISAPI response-status validation helper.
  - String/error parsing helpers.
  - SDK/PlayM4 error-name table lookups.
  - Targeted discovery sweep path generation.
- Verified test suite locally with `cargo test`.
  - Result: `11 passed; 0 failed`.

## Open Questions

- Metadata on preview stream:
  - Do we want to explicitly probe `/ISAPI/System/capabilities` for `DeviceCap.SysCap.isSupportMetadata` and surface that in discovery output?
  - If metadata is supported, should we add a CLI flag to enable `NET_DVR_PREVIEWINFO.byRecvMetaData` and log callback packet types beyond `NET_DVR_STREAMDATA`?
- Telemetry strategy:
  - For this firmware/device, thermometry ISAPI routes currently report `notSupport`.
  - Should future telemetry support be scoped to OSD extraction only, or include alternate SDK alarm/event channels when available?
