# Discovery Capabilities Summary

Source report:

- `docs/discovery_reports/discovery_10_15_12_1_8000_1770688029141.json`

High-level probe results:

- `75` probes total
- `11` succeeded
- `64` failed
- Most ISAPI failures are explicit `notSupport` (`45`), plus `methodNotAllowed` (`2`) and one `deviceError`

## Supported

- Core ISAPI surfaces are present:
  - `/ISAPI/System/deviceInfo`
  - `/ISAPI/Streaming/channels`
  - `/ISAPI/Image/channels`
  - `/ISAPI/System/Video/inputs/channels`
- Streaming capabilities exposed:
  - stream IDs include `101` and `102`
  - transports include `RTSP`, `HTTP`, `SHTTP`
  - unicast (`RTP/TCP`) and multicast are enabled
- Imaging controls on channel `1` are exposed:
  - focus style options: `AUTO, MANUAL, SEMIAUTOMATIC`
  - noise reduction modes: `close, general, advanced`
  - dehaze endpoint works
  - digital zoom options: `1x, 2x, 4x, 8x`
  - palettes include `WhiteHot, BlackHot, RedHot, Fusion1`
  - regional image enhancement modes are exposed
- Thermal intelligence endpoint is present on channel `1`:
  - `/ISAPI/Thermal/channels/1/thermIntell`
  - capability shows only `intellType=fireDetection`

## Not Supported / Failing

- Thermometry API surface appears unavailable:
  - `/ISAPI/Thermal/channels/{1,2}/thermometry/*` returns `notSupport`
  - thermal PIP/manual thermometry endpoints return `notSupport`
- Channel `2` imaging/thermal endpoints mostly return `notSupport`
- SDK-side capability/config calls mostly failed:
  - many `NET_DVR_* capability` calls => SDK error `1000`
  - thermometry config calls => SDK error `17`
  - many thermal ISAPI-backed failures map to SDK error `23`
- `whiteBalance` on channel `1` returns `deviceError` (not cleanly supported)

## Practical Takeaway

- This unit supports streaming + image-mode control + thermal fire-detection intelligence
- It does not expose general thermometry control/data APIs through the tested Hikvision ISAPI/SDK surfaces on the current firmware/configuration
