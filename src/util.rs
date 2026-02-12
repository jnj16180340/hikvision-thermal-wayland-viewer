use std::ffi::c_char;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};

use crate::ffi::{Dword, NetDvrThermometryCond};

pub(crate) fn net_dvr_error_name(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("NET_DVR_NOERROR"),
        1 => Some("NET_DVR_PASSWORD_ERROR"),
        2 => Some("NET_DVR_NOENOUGHPRI"),
        3 => Some("NET_DVR_NOINIT"),
        4 => Some("NET_DVR_CHANNEL_ERROR"),
        5 => Some("NET_DVR_OVER_MAXLINK"),
        6 => Some("NET_DVR_VERSIONNOMATCH"),
        7 => Some("NET_DVR_NETWORK_FAIL_CONNECT"),
        8 => Some("NET_DVR_NETWORK_SEND_ERROR"),
        9 => Some("NET_DVR_NETWORK_RECV_ERROR"),
        10 => Some("NET_DVR_NETWORK_RECV_TIMEOUT"),
        11 => Some("NET_DVR_NETWORK_ERRORDATA"),
        12 => Some("NET_DVR_ORDER_ERROR"),
        13 => Some("NET_DVR_OPERNOPERMIT"),
        14 => Some("NET_DVR_COMMANDTIMEOUT"),
        15 => Some("NET_DVR_ERRORSERIALPORT"),
        16 => Some("NET_DVR_ERRORALARMPORT"),
        17 => Some("NET_DVR_PARAMETER_ERROR"),
        18 => Some("NET_DVR_CHAN_EXCEPTION"),
        19 => Some("NET_DVR_NODISK"),
        20 => Some("NET_DVR_ERRORDISKNUM"),
        21 => Some("NET_DVR_DISK_FULL"),
        22 => Some("NET_DVR_DISK_ERROR"),
        23 => Some("NET_DVR_NOSUPPORT"),
        24 => Some("NET_DVR_BUSY"),
        25 => Some("NET_DVR_MODIFY_FAIL"),
        26 => Some("NET_DVR_PASSWORD_FORMAT_ERROR"),
        27 => Some("NET_DVR_DISK_FORMATING"),
        28 => Some("NET_DVR_DVRNORESOURCE"),
        29 => Some("NET_DVR_DVROPRATEFAILED"),
        30 => Some("NET_DVR_OPENHOSTSOUND_FAIL"),
        31 => Some("NET_DVR_DVRVOICEOPENED"),
        32 => Some("NET_DVR_TIMEINPUTERROR"),
        33 => Some("NET_DVR_NOSPECFILE"),
        34 => Some("NET_DVR_CREATEFILE_ERROR"),
        35 => Some("NET_DVR_FILEOPENFAIL"),
        36 => Some("NET_DVR_OPERNOTFINISH"),
        37 => Some("NET_DVR_GETPLAYTIMEFAIL"),
        38 => Some("NET_DVR_PLAYFAIL"),
        39 => Some("NET_DVR_FILEFORMAT_ERROR"),
        40 => Some("NET_DVR_DIR_ERROR"),
        41 => Some("NET_DVR_ALLOC_RESOURCE_ERROR"),
        42 => Some("NET_DVR_AUDIO_MODE_ERROR"),
        43 => Some("NET_DVR_NOENOUGH_BUF"),
        44 => Some("NET_DVR_CREATESOCKET_ERROR"),
        45 => Some("NET_DVR_SETSOCKET_ERROR"),
        46 => Some("NET_DVR_MAX_NUM"),
        47 => Some("NET_DVR_USERNOTEXIST"),
        48 => Some("NET_DVR_WRITEFLASHERROR"),
        49 => Some("NET_DVR_UPGRADEFAIL"),
        50 => Some("NET_DVR_CARDHAVEINIT"),
        51 => Some("NET_DVR_PLAYERFAILED"),
        52 => Some("NET_DVR_MAX_USERNUM"),
        53 => Some("NET_DVR_GETLOCALIPANDMACFAIL"),
        54 => Some("NET_DVR_NOENCODEING"),
        55 => Some("NET_DVR_IPMISMATCH"),
        56 => Some("NET_DVR_MACMISMATCH"),
        57 => Some("NET_DVR_UPGRADELANGMISMATCH"),
        58 => Some("NET_DVR_MAX_PLAYERPORT"),
        59 => Some("NET_DVR_NOSPACEBACKUP"),
        60 => Some("NET_DVR_NODEVICEBACKUP"),
        61 => Some("NET_DVR_PICTURE_BITS_ERROR"),
        62 => Some("NET_DVR_PICTURE_DIMENSION_ERROR"),
        63 => Some("NET_DVR_PICTURE_SIZ_ERROR"),
        64 => Some("NET_DVR_LOADPLAYERSDKFAILED"),
        65 => Some("NET_DVR_LOADPLAYERSDKPROC_ERROR"),
        66 => Some("NET_DVR_LOADDSSDKFAILED"),
        67 => Some("NET_DVR_LOADDSSDKPROC_ERROR"),
        68 => Some("NET_DVR_DSSDK_ERROR"),
        69 => Some("NET_DVR_VOICEMONOPOLIZE"),
        70 => Some("NET_DVR_JOINMULTICASTFAILED"),
        71 => Some("NET_DVR_CREATEDIR_ERROR"),
        72 => Some("NET_DVR_BINDSOCKET_ERROR"),
        73 => Some("NET_DVR_SOCKETCLOSE_ERROR"),
        74 => Some("NET_DVR_USERID_ISUSING"),
        75 => Some("NET_DVR_SOCKETLISTEN_ERROR"),
        76 => Some("NET_DVR_PROGRAM_EXCEPTION"),
        77 => Some("NET_DVR_WRITEFILE_FAILED"),
        78 => Some("NET_DVR_FORMAT_READONLY"),
        79 => Some("NET_DVR_WITHSAMEUSERNAME"),
        80 => Some("NET_DVR_DEVICETYPE_ERROR"),
        81 => Some("NET_DVR_LANGUAGE_ERROR"),
        82 => Some("NET_DVR_PARAVERSION_ERROR"),
        83 => Some("NET_DVR_IPCHAN_NOTALIVE"),
        84 => Some("NET_DVR_RTSP_SDK_ERROR"),
        85 => Some("NET_DVR_CONVERT_SDK_ERROR"),
        86 => Some("NET_DVR_IPC_COUNT_OVERFLOW"),
        87 => Some("NET_DVR_MAX_ADD_NUM"),
        88 => Some("NET_DVR_PARAMMODE_ERROR"),
        89 => Some("NET_DVR_CODESPITTER_OFFLINE"),
        90 => Some("NET_DVR_BACKUP_COPYING"),
        91 => Some("NET_DVR_CHAN_NOTSUPPORT"),
        92 => Some("NET_DVR_CALLINEINVALID"),
        93 => Some("NET_DVR_CALCANCELCONFLICT"),
        94 => Some("NET_DVR_CALPOINTOUTRANGE"),
        95 => Some("NET_DVR_FILTERRECTINVALID"),
        96 => Some("NET_DVR_DDNS_DEVOFFLINE"),
        97 => Some("NET_DVR_DDNS_INTER_ERROR"),
        100 => Some("NET_DVR_INTERCOM_SDK_ERROR"),
        147 => Some("NET_DVR_CERTIFICATE_FILE_ERROR"),
        148 => Some("NET_DVR_LOAD_SSL_LIB_ERROR"),
        156 => Some("NET_DVR_LOAD_LIBEAY32_DLL_ERROR"),
        157 => Some("NET_DVR_LOAD_SSLEAY32_DLL_ERROR"),
        250 => Some("NET_DVR_ERROR_DEVICE_NOT_ACTIVATED"),
        251 => Some("NET_DVR_ERROR_RISK_PASSWORD"),
        252 => Some("NET_DVR_ERROR_DEVICE_HAS_ACTIVATED"),
        883 => Some("NET_DVR_THERMOMETRY_REGION_OVERSTEP_ERROR"),
        897 => Some("NET_DVR_NO_TEMP_SENSOR_ERROR"),
        898 => Some("NET_DVR_PUPIL_DISTANCE_OVERSIZE_ERROR"),
        1000 => Some("XML_ABILITY_NOTSUPPORT"),
        1001 => Some("XML_ANALYZE_NOENOUGH_BUF"),
        1002 => Some("XML_ANALYZE_FIND_LOCALXML_ERROR"),
        1003 => Some("XML_ANALYZE_LOAD_LOCALXML_ERROR"),
        1004 => Some("XML_NANLYZE_DVR_DATA_FORMAT_ERROR"),
        1005 => Some("XML_ANALYZE_TYPE_ERROR"),
        1006 => Some("XML_ANALYZE_XML_NODE_ERROR"),
        1007 => Some("XML_INPUT_PARAM_ERROR"),
        1008 => Some("NET_DVR_ERR_RETURNED_XML_DATA"),
        _ => None,
    }
}

pub(crate) fn format_sdk_error(code: u32) -> String {
    match net_dvr_error_name(code) {
        Some(name) => format!("{code} ({name})"),
        None => code.to_string(),
    }
}

pub(crate) fn playm4_error_name(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("PLAYM4_NOERROR"),
        1 => Some("PLAYM4_PARA_OVER"),
        2 => Some("PLAYM4_ORDER_ERROR"),
        3 => Some("PLAYM4_TIMER_ERROR"),
        4 => Some("PLAYM4_DEC_VIDEO_ERROR"),
        5 => Some("PLAYM4_DEC_AUDIO_ERROR"),
        6 => Some("PLAYM4_ALLOC_MEMORY_ERROR"),
        7 => Some("PLAYM4_OPEN_FILE_ERROR"),
        8 => Some("PLAYM4_CREATE_OBJ_ERROR"),
        11 => Some("PLAYM4_BUF_OVER"),
        12 => Some("PLAYM4_CREATE_SOUND_ERROR"),
        13 => Some("PLAYM4_SET_VOLUME_ERROR"),
        14 => Some("PLAYM4_SUPPORT_FILE_ONLY"),
        15 => Some("PLAYM4_SUPPORT_STREAM_ONLY"),
        16 => Some("PLAYM4_SYS_NOT_SUPPORT"),
        17 => Some("PLAYM4_FILEHEADER_UNKNOWN"),
        18 => Some("PLAYM4_VERSION_INCORRECT"),
        19 => Some("PLAYM4_INIT_DECODER_ERROR"),
        20 => Some("PLAYM4_CHECK_FILE_ERROR"),
        21 => Some("PLAYM4_INIT_TIMER_ERROR"),
        22 => Some("PLAYM4_BLT_ERROR"),
        24 => Some("PLAYM4_OPEN_FILE_ERROR_MULTI"),
        25 => Some("PLAYM4_OPEN_FILE_ERROR_VIDEO"),
        26 => Some("PLAYM4_JPEG_COMPRESS_ERROR"),
        27 => Some("PLAYM4_EXTRACT_NOT_SUPPORT"),
        28 => Some("PLAYM4_EXTRACT_DATA_ERROR"),
        29 => Some("PLAYM4_SECRET_KEY_ERROR"),
        30 => Some("PLAYM4_DECODE_KEYFRAME_ERROR"),
        31 => Some("PLAYM4_NEED_MORE_DATA"),
        32 => Some("PLAYM4_INVALID_PORT"),
        33 => Some("PLAYM4_NOT_FIND"),
        34 => Some("PLAYM4_NEED_LARGER_BUFFER"),
        99 => Some("PLAYM4_FAIL_UNKNOWN"),
        _ => None,
    }
}

pub(crate) fn format_playm4_error(code: u32) -> String {
    match playm4_error_name(code) {
        Some(name) => format!("{code} ({name})"),
        None => code.to_string(),
    }
}

pub(crate) fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

pub(crate) fn bool_u8_text(v: u8) -> &'static str {
    match v {
        0 => "no",
        1 => "yes",
        _ => "unknown",
    }
}

pub(crate) fn bool_xml_value(v: bool) -> &'static str {
    if v { "true" } else { "false" }
}

pub(crate) fn c_byte_array_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

pub(crate) fn stream_proto_name(v: u8) -> &'static str {
    match v {
        0 => "private",
        1 => "rtsp",
        2 => "private+rtsp",
        _ => "unknown",
    }
}

pub(crate) fn thermometry_mode_name(v: u8) -> &'static str {
    match v {
        0 => "normal",
        1 => "expert",
        _ => "unknown",
    }
}

pub(crate) fn thermometry_roi_mode_name(v: u8) -> &'static str {
    match v {
        0 => "reserved",
        1 => "close",
        2 => "open",
        _ => "unknown",
    }
}

pub(crate) fn thermometry_unit_name(v: u8) -> &'static str {
    match v {
        0 => "C",
        1 => "F",
        2 => "K",
        _ => "unknown",
    }
}

pub(crate) fn distance_unit_name(v: u8) -> &'static str {
    match v {
        0 => "meter",
        1 => "feet",
        2 => "centimeter",
        _ => "unknown",
    }
}

pub(crate) fn thermometry_range_name(v: u8) -> &'static str {
    match v {
        0 => "default",
        1 => "-20..150",
        2 => "0..550",
        3 => "0..650",
        4 => "-40..150",
        5 => "0..1200",
        6 => "-20..120",
        7 => "20..350",
        8 => "20..45",
        9 => "30..45",
        0xFF => "auto",
        _ => "unknown",
    }
}

pub(crate) fn thermal_pip_mode_name(v: u8) -> &'static str {
    match v {
        0 => "overlay",
        1 => "fusion",
        2 => "normal",
        _ => "unknown",
    }
}

pub(crate) fn thermal_overlap_type_name(v: u8) -> &'static str {
    match v {
        0 => "visible-over-thermal",
        1 => "thermal-over-visible",
        _ => "unknown",
    }
}

pub(crate) fn buffer_to_text(buf: &[u8], preferred_len: Option<usize>) -> String {
    let mut end = preferred_len.unwrap_or(buf.len()).min(buf.len());
    if end == 0 {
        end = buf.len();
    }
    if let Some(nul_pos) = buf[..end].iter().position(|&b| b == 0) {
        end = nul_pos;
    }
    String::from_utf8_lossy(&buf[..end]).trim().to_string()
}

pub(crate) fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(crate) fn read_f32_le(buf: &[u8], offset: usize) -> Option<f32> {
    let bytes = buf.get(offset..offset + 4)?;
    Some(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(crate) fn build_thermometry_cond_bytes(channel: i32) -> Vec<u8> {
    let cond = NetDvrThermometryCond {
        size: std::mem::size_of::<NetDvrThermometryCond>() as Dword,
        channel: channel.max(1) as Dword,
        preset_no: 0,
        res: [0; 62],
    };
    unsafe {
        std::slice::from_raw_parts(
            (&cond as *const NetDvrThermometryCond).cast(),
            std::mem::size_of::<NetDvrThermometryCond>(),
        )
        .to_vec()
    }
}

pub(crate) fn extract_sdk_error_code(text: &str) -> Option<u32> {
    let marker = "SDK error ";
    let idx = text.find(marker)?;
    let digits: String = text[idx + marker.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<u32>().ok()
    }
}

pub(crate) fn truncate_for_report(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let mut truncated = String::new();
    for ch in text.chars().take(max_chars) {
        truncated.push(ch);
    }
    truncated.push_str("...(truncated)");
    truncated
}

pub(crate) fn sanitize_filename_token(token: &str) -> String {
    token
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn hex_sample(bytes: &[u8], max_bytes: usize) -> String {
    let mut out = String::new();
    for (idx, b) in bytes.iter().take(max_bytes).enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
    if bytes.len() > max_bytes {
        out.push_str(" ...");
    }
    out
}

pub(crate) fn write_c_char_buf(buf: &mut [c_char], value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    if bytes.len() >= buf.len() {
        bail!(
            "value too long for C buffer (max {}): {value}",
            buf.len() - 1
        );
    }
    buf.fill(0);
    for (i, byte) in bytes.iter().enumerate() {
        buf[i] = *byte as c_char;
    }
    Ok(())
}

pub(crate) fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
