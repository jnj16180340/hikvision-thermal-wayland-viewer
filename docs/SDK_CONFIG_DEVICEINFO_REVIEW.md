# HCNetSDK Configuration and Device-Info Review

Date: 2026-02-10

## Scope

This review is scoped to the current project goal:

- Rust application
- Wayland-only local rendering
- Live preview from a Hikvision-compatible thermal camera
- No PTZ control, playback, NVR management, access-control workflows, or cloud workflows

## Source Files Reviewed

- `EN-HCNetSDKV6.1.9.4_build20220412_linux64/incEn/HCNetSDK.h`
- `EN-HCNetSDKV6.1.9.4_build20220412_linux64/incEn/plaympeg4.h`
- `Research/NOTES/ISAPI_System_deviceInfo`
- `Research/NOTES/ISAPI_System_deviceinfo_capabilities`
- `Research/NOTES/ISAPI_Streaming_channels.xml`
- `Research/NOTES/ISAPI_Image_channels.xml`
- `Research/NOTES/ISAPI_System_vide_inputs_channels.xml`

## SDK Surface Size (Header-Level)

Quick size indicators from `HCNetSDK.h`:

- `NET_DVR_API ... NET_DVR_*` exported functions: ~835
- `NET_DVR_GET_*` command macros: ~1191
- `NET_DVR_SET_*` command macros: ~810
- `NET_DVR_GET/SET` names containing `THERM`/`TEMP`: ~71

Practical meaning:

- HCNetSDK is very broad and device-family-wide.
- Only a small subset is needed for this viewer.

## Device Info and Configuration Surfaces Exposed by SDK

### 1. Login-Time Device Info

Key structures and APIs:

- `NET_DVR_USER_LOGIN_INFO` (`HCNetSDK.h:31858`)
- `NET_DVR_Login_V40` (`HCNetSDK.h:50338`)
- `NET_DVR_DEVICEINFO_V30` (`HCNetSDK.h:13448`)
- `NET_DVR_DEVICEINFO_V40` (`HCNetSDK.h:13476`)

What it gives:

- Channel counts (`byChanNum`, `byIPChanNum`, `byStartChan`, `byStartDChan`)
- Protocol support hints (`byMainProto`, `bySubProto`)
- Bitfield support flags (`bySupport*`) including whether XML-style transfer is supported (`bySupport4 & 0x02`)
- Login security state (`bySupportLock`, `dwSurplusLockTime`, password-level fields, login mode)

Relevance to this viewer: `High`

### 2. Generic Device Config (Get/Set by Command ID)

Core interfaces:

- `NET_DVR_GetDVRConfig` (`HCNetSDK.h:50699`)
- `NET_DVR_SetDVRConfig` (`HCNetSDK.h:50700`)

Representative command IDs:

- `NET_DVR_GET_DEVICECFG` / `SET_DEVICECFG` (`HCNetSDK.h:2045`)
- `NET_DVR_GET_NETCFG` / `SET_NETCFG` (`HCNetSDK.h:2047`)
- `NET_DVR_GET_PICCFG` / `SET_PICCFG` (`HCNetSDK.h:2049`)
- `NET_DVR_GET_COMPRESSCFG` / `SET_COMPRESSCFG` (`HCNetSDK.h:2051`)
- `NET_DVR_GET_RECORDCFG` / `SET_RECORDCFG` (`HCNetSDK.h:2053`)
- `NET_DVR_GET_USERCFG` / `SET_USERCFG` (`HCNetSDK.h:2069`)
- `NET_DVR_GET_IPPARACFG_V40` (`HCNetSDK.h:2362`)

Relevance to this viewer: `Medium`

- Useful for discovery and tuning.
- Not required for baseline live view once login/preview works.

### 3. Capability Query APIs

Capability interfaces:

- `NET_DVR_GetDeviceAbility` (`HCNetSDK.h:50744`)
- `NET_DVR_GetSTDAbility` (`HCNetSDK.h:51265`)

Ability type constants:

- `DEVICE_ABILITY_INFO` (`HCNetSDK.h:4994`)
- `DEVICE_NETWORK_ABILITY` (`HCNetSDK.h:4979`)
- `STREAM_ABILITY` (`HCNetSDK.h:4995`)
- `SYSTEM_MANAGEMENT_ABILITY` (`HCNetSDK.h:4996`)

Capability command examples:

- `NET_DVR_GET_STREAMING_CAPABILITIES` (`HCNetSDK.h:3530`)
- `NET_DVR_GET_PREVIEW_MODE_CAPABILITIES` (`HCNetSDK.h:3640`)
- `NET_DVR_GET_SYSTEM_CAPABILITIES` (`HCNetSDK.h:4490`)
- `NET_DVR_GET_THERMAL_CAPABILITIES` (`HCNetSDK.h:3460`)

Relevance to this viewer: `High` for robust auto-configuration

### 4. XML/ISAPI-Style Generic Path

Interfaces and buffers:

- `NET_DVR_STDXMLConfig` (`HCNetSDK.h:51267`)
- `NET_DVR_XML_CONFIG_INPUT` (`HCNetSDK.h:39417`)
- `NET_DVR_XML_CONFIG_OUTPUT` (`HCNetSDK.h:39431`)
- `NET_DVR_STD_CONFIG` (`HCNetSDK.h:39344`)
- `NET_DVR_STD_ABILITY` (`HCNetSDK.h:39359`)

Relevance to this viewer: `Medium`

- Useful for future features and fine-grained capability introspection.
- Not needed for current frame path.

### 5. Live Preview and Stream Transport

Core preview API and structure:

- `NET_DVR_RealPlay_V40` (`HCNetSDK.h:50359`)
- `NET_DVR_PREVIEWINFO` (`HCNetSDK.h:13545`)

Important fields for tuning:

- `dwStreamType` (main/sub/etc stream selection)
- `dwLinkMode` (TCP/UDP/RTP variants)
- `byProtoType` (private/RTSP/SRTP)
- `dwDisplayBufNum`
- `byRecvMetaData`

Related channel/stream topology structures:

- `NET_DVR_IPPARACFG_V40` (`HCNetSDK.h:11059`)
- `NET_DVR_STREAM_MODE` (`HCNetSDK.h:11050`)
- `NET_DVR_IPCHANINFO_V40` (`HCNetSDK.h:10973`)

Relevance to this viewer: `High`

### 6. PlayCtrl Decoder Surface (Separate C Library)

Key decode path APIs from `plaympeg4.h`:

- `PlayM4_GetPort` (`plaympeg4.h:458`)
- `PlayM4_OpenStream` (`plaympeg4.h:327`)
- `PlayM4_InputData` (`plaympeg4.h:328`)
- `PlayM4_SetDecCallBackMend` (`plaympeg4.h:451`)
- `PlayM4_Play` / `PlayM4_Stop` (`plaympeg4.h:315`, `plaympeg4.h:316`)

Frame structure:

- `FRAME_INFO` (`plaympeg4.h:194`)

Relevance to this viewer: `High`

### 7. Thermal/Thermometry Config and Events

Thermal command IDs include:

- `NET_DVR_GET/SET_THERMOMETRY_BASICPARAM` (`HCNetSDK.h:3446`, `HCNetSDK.h:3447`)
- `NET_DVR_GET/SET_THERMOMETRY_PRESETINFO` (`HCNetSDK.h:3449`, `HCNetSDK.h:3450`)
- `NET_DVR_GET/SET_THERMOMETRY_ALARMRULE` (`HCNetSDK.h:3452`, `HCNetSDK.h:3453`)
- `NET_DVR_GET/SET_THERMOMETRY_MODE` (`HCNetSDK.h:4451`, `HCNetSDK.h:4452`)
- `NET_DVR_GET_THERMAL_CAPABILITIES` (`HCNetSDK.h:3460`)

Thermometry structures:

- `NET_DVR_THERMOMETRY_BASICPARAM` (`HCNetSDK.h:48174`)
- `NET_DVR_THERMOMETRY_PRESETINFO` (`HCNetSDK.h:48244`)
- `NET_DVR_REALTIME_THERMOMETRY_COND` (`HCNetSDK.h:48297`)
- `NET_DVR_THERMOMETRY_UPLOAD` (`HCNetSDK.h:48325`)
- `NET_DVR_THERMOMETRY_ALARM` (`HCNetSDK.h:48360`)

Thermal alarm event codes:

- `COMM_THERMOMETRY_DIFF_ALARM` (`HCNetSDK.h:5219`)
- `COMM_THERMOMETRY_ALARM` (`HCNetSDK.h:5220`)
- `COMM_FACE_THERMOMETRY_ALARM` (`HCNetSDK.h:5198`)

Relevance to this viewer:

- `Low` for baseline video rendering
- `High` if we add temperature overlays/rules/alerts/analytics

## Bench Device Snapshot (from Existing Notes)

From `Research/NOTES/ISAPI_System_deviceInfo`:

- Model: `AGM Taipan TM10-256`
- Device type: `IPCamera`
- Firmware: `V5.5.52` (`build 221202`)
- Encoder version: `V7.3`

From `Research/NOTES/ISAPI_Streaming_channels.xml`:

- Streaming channels present: `101`, `102`
- Video codec reported: `H.264`
- Declared packet types: `PS` and `RTP`
- Transport/control includes `RTSP` and TCP-based unicast

From `Research/NOTES/ISAPI_Image_channels.xml` and `...vide_inputs...`:

- Video input channel `1`
- Base image controls exposed (brightness/contrast/saturation/noise reduction/focus)

## Relevance Matrix for This Project

`Must use now`:

- SDK init path and TLS/OpenSSL path config (`NET_DVR_SetSDKInitCfg`)
- Login/logout and device-info retrieval (`NET_DVR_Login_V40`, `NET_DVR_DEVICEINFO_V40`)
- Preview setup (`NET_DVR_PREVIEWINFO`, `NET_DVR_RealPlay_V40`, `NET_DVR_StopRealPlay`)
- Decode path via PlayCtrl callback (`PlayM4_*` stream open/input/decode callback)
- Error handling and reconnect knobs (`NET_DVR_GetLastError`, `NET_DVR_SetConnectTime`, `NET_DVR_SetReconnect`)

`Useful next`:

- Capability probes before selecting stream/link/protocol (`NET_DVR_GetDeviceAbility`, `NET_DVR_GET_STREAMING_CAPABILITIES`, `NET_DVR_GET_PREVIEW_MODE_CAPABILITIES`)
- Channel topology discovery (`NET_DVR_GET_IPPARACFG_V40`)
- Thermal capability discovery (`NET_DVR_GET_THERMAL_CAPABILITIES`)

`Likely irrelevant for current viewer`:

- PTZ control (`NET_DVR_PTZControl*`)
- Playback/file search/download (`NET_DVR_FindFile*`, `NET_DVR_PlayBackByTime*`, `NET_DVR_GetFileByTime*`)
- Disk format/upgrade paths (`NET_DVR_FormatDisk*`, `NET_DVR_Upgrade*`)
- Voice intercom/broadcast (`NET_DVR_StartVoiceCom*`)
- Access control/card/face libraries, matrix/video-wall management, and alarmhost administration

## Recommended Minimal Introspection Flow

1. Login with `NET_DVR_Login_V40` and record `NET_DVR_DEVICEINFO_V40`.
2. Query `NET_DVR_GetDeviceAbility` for at least:
   - `DEVICE_ABILITY_INFO`
   - `DEVICE_NETWORK_ABILITY`
   - `STREAM_ABILITY`
3. Query `NET_DVR_GetDVRConfig` for `NET_DVR_GET_IPPARACFG_V40` when channel mapping is unclear.
4. Start preview with conservative defaults (`stream_type=0`, `link_mode=0`, private protocol), then adapt based on capability result.
5. Defer thermometry/alert configuration until temperature metadata or alarming is a stated requirement.
