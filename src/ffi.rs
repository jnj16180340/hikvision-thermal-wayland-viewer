use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use libloading::Library;

pub(crate) type Bool = i32;
pub(crate) type Dword = u32;
pub(crate) type Word = u16;
pub(crate) type Long = i32;
pub(crate) type Byte = u8;
pub(crate) type Hwnd = u32;

pub(crate) const NET_DVR_SYSHEAD: Dword = 1;
pub(crate) const NET_DVR_STREAMDATA: Dword = 2;

pub(crate) const NET_SDK_INIT_CFG_SDK_PATH: i32 = 2;
pub(crate) const NET_SDK_INIT_CFG_LIBEAY_PATH: i32 = 3;
pub(crate) const NET_SDK_INIT_CFG_SSLEAY_PATH: i32 = 4;

pub(crate) const NET_DVR_GET_IPPARACFG_V40: Dword = 1062;

pub(crate) const NET_DVR_GET_THERMOMETRY_BASICPARAM: Dword = 3621;
pub(crate) const NET_DVR_GET_THERMAL_CAPABILITIES: Dword = 3634;
pub(crate) const NET_DVR_GET_STREAMING_CAPABILITIES: Dword = 3712;
pub(crate) const NET_DVR_GET_PANORAMAIMAGE_CAPABILITIES: Dword = 3746;
pub(crate) const NET_DVR_GET_PREVIEW_MODE_CAPABILITIES: Dword = 3983;
pub(crate) const NET_DVR_GET_MANUALTHERM_CAPABILITIES: Dword = 6707;
pub(crate) const NET_DVR_GET_THERMINTELL_CAPABILITIES: Dword = 6711;
pub(crate) const NET_DVR_GET_THERMOMETRY_MODE: Dword = 6765;
pub(crate) const NET_DVR_GET_THERMAL_PIP: Dword = 6768;

pub(crate) const NET_DVR_GET_THERMOMETRY_BASICPARAM_CAPABILITIES: Dword = 3620;
pub(crate) const NET_DVR_GET_THERMOMETRY_MODE_CAPABILITIES: Dword = 6764;
pub(crate) const NET_DVR_GET_THERMAL_PIP_CAPABILITIES: Dword = 6767;

pub(crate) const DEVICE_NETWORK_ABILITY: Dword = 0x002;
pub(crate) const DEVICE_ABILITY_INFO: Dword = 0x011;
pub(crate) const STREAM_ABILITY: Dword = 0x012;
pub(crate) const SYSTEM_MANAGEMENT_ABILITY: Dword = 0x013;

pub(crate) const STREAME_REALTIME: u32 = 0;
pub(crate) const PLAYM4_BUF_OVER: u32 = 11;
pub(crate) const T_YV12: i32 = 3;
pub(crate) const REEXEC_MARKER: &str = "HIK_SDK_LD_READY";
pub(crate) const DEFAULT_VENDOR_SDK_ROOT: &str = "vendor/linux/x86_64/hik_sdk";

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NetDvrDeviceInfoV30 {
    pub(crate) serial_number: [Byte; 48],
    pub(crate) alarm_in_port_num: Byte,
    pub(crate) alarm_out_port_num: Byte,
    pub(crate) disk_num: Byte,
    pub(crate) dvr_type: Byte,
    pub(crate) chan_num: Byte,
    pub(crate) start_chan: Byte,
    pub(crate) audio_chan_num: Byte,
    pub(crate) ip_chan_num: Byte,
    pub(crate) zero_chan_num: Byte,
    pub(crate) main_proto: Byte,
    pub(crate) sub_proto: Byte,
    pub(crate) support: Byte,
    pub(crate) support1: Byte,
    pub(crate) support2: Byte,
    pub(crate) dev_type: Word,
    pub(crate) support3: Byte,
    pub(crate) multi_stream_proto: Byte,
    pub(crate) start_dchan: Byte,
    pub(crate) start_dtalk_chan: Byte,
    pub(crate) high_dchan_num: Byte,
    pub(crate) support4: Byte,
    pub(crate) language_type: Byte,
    pub(crate) voice_in_chan_num: Byte,
    pub(crate) start_voice_in_chan_no: Byte,
    pub(crate) support5: Byte,
    pub(crate) support6: Byte,
    pub(crate) mirror_chan_num: Byte,
    pub(crate) start_mirror_chan_no: Word,
    pub(crate) support7: Byte,
    pub(crate) res2: Byte,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NetDvrDeviceInfoV40 {
    pub(crate) device_v30: NetDvrDeviceInfoV30,
    pub(crate) support_lock: Byte,
    pub(crate) retry_login_time: Byte,
    pub(crate) password_level: Byte,
    pub(crate) proxy_type: Byte,
    pub(crate) surplus_lock_time: Dword,
    pub(crate) char_encode_type: Byte,
    pub(crate) support_dev5: Byte,
    pub(crate) support: Byte,
    pub(crate) login_mode: Byte,
    pub(crate) oem_code: Dword,
    pub(crate) residual_validity_i32: i32,
    pub(crate) residual_validity: Byte,
    pub(crate) single_start_dtalk_chan: Byte,
    pub(crate) single_dtalk_chan_nums: Byte,
    pub(crate) password_reset_level: Byte,
    pub(crate) support_stream_encrypt: Byte,
    pub(crate) market_type: Byte,
    pub(crate) res2: [Byte; 238],
}

pub(crate) type LoginResultCallback = Option<
    unsafe extern "C" fn(
        user_id: Long,
        result: Dword,
        device_info: *mut NetDvrDeviceInfoV30,
        user_data: *mut c_void,
    ),
>;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NetDvrUserLoginInfo {
    pub(crate) device_address: [c_char; 129],
    pub(crate) use_transport: Byte,
    pub(crate) port: Word,
    pub(crate) username: [c_char; 64],
    pub(crate) password: [c_char; 64],
    pub(crate) cb_login_result: LoginResultCallback,
    pub(crate) user_data: *mut c_void,
    pub(crate) use_async_login: Bool,
    pub(crate) proxy_type: Byte,
    pub(crate) use_utc_time: Byte,
    pub(crate) login_mode: Byte,
    pub(crate) https_mode: Byte,
    pub(crate) proxy_id: Long,
    pub(crate) verify_mode: Byte,
    pub(crate) res3: [Byte; 119],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NetDvrPreviewInfo {
    pub(crate) channel: Long,
    pub(crate) stream_type: Dword,
    pub(crate) link_mode: Dword,
    pub(crate) play_wnd: Hwnd,
    pub(crate) blocked: Dword,
    pub(crate) passback_record: Dword,
    pub(crate) preview_mode: Byte,
    pub(crate) stream_id: [Byte; 32],
    pub(crate) proto_type: Byte,
    pub(crate) res1: Byte,
    pub(crate) video_coding_type: Byte,
    pub(crate) display_buf_num: Dword,
    pub(crate) npq_mode: Byte,
    pub(crate) recv_metadata: Byte,
    pub(crate) data_type: Byte,
    pub(crate) res: [Byte; 213],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NetDvrLocalSdkPath {
    pub(crate) path: [c_char; 256],
    pub(crate) res: [Byte; 128],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct FrameInfo {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) stamp: i32,
    pub(crate) frame_type: i32,
    pub(crate) frame_rate: i32,
    pub(crate) frame_num: u32,
}

#[repr(C)]
pub(crate) struct NetDvrStdAbility {
    pub(crate) cond_buffer: *mut c_void,
    pub(crate) cond_size: Dword,
    pub(crate) out_buffer: *mut c_void,
    pub(crate) out_size: Dword,
    pub(crate) status_buffer: *mut c_void,
    pub(crate) status_size: Dword,
    pub(crate) ret_size: Dword,
    pub(crate) res: [Byte; 32],
}

#[repr(C)]
pub(crate) struct NetDvrStdConfig {
    pub(crate) cond_buffer: *mut c_void,
    pub(crate) cond_size: Dword,
    pub(crate) in_buffer: *mut c_void,
    pub(crate) in_size: Dword,
    pub(crate) out_buffer: *mut c_void,
    pub(crate) out_size: Dword,
    pub(crate) status_buffer: *mut c_void,
    pub(crate) status_size: Dword,
    pub(crate) xml_buffer: *mut c_void,
    pub(crate) xml_size: Dword,
    pub(crate) data_type: Byte,
    pub(crate) res: [Byte; 23],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct NetDvrThermometryCond {
    pub(crate) size: Dword,
    pub(crate) channel: Dword,
    pub(crate) preset_no: Word,
    pub(crate) res: [Byte; 62],
}

#[repr(C)]
pub(crate) struct NetDvrXmlConfigInput {
    pub(crate) size: Dword,
    pub(crate) request_url: *mut c_void,
    pub(crate) request_url_len: Dword,
    pub(crate) in_buffer: *mut c_void,
    pub(crate) in_buffer_size: Dword,
    pub(crate) recv_timeout: Dword,
    pub(crate) force_encrypt: Byte,
    pub(crate) num_of_multipart: Byte,
    pub(crate) mime_type: Byte,
    pub(crate) res: [Byte; 29],
}

#[repr(C)]
pub(crate) struct NetDvrXmlConfigOutput {
    pub(crate) size: Dword,
    pub(crate) out_buffer: *mut c_void,
    pub(crate) out_buffer_size: Dword,
    pub(crate) returned_xml_size: Dword,
    pub(crate) status_buffer: *mut c_void,
    pub(crate) status_size: Dword,
    pub(crate) data_buffer: *mut c_void,
    pub(crate) num_of_multipart: Byte,
    pub(crate) res: [Byte; 23],
}

pub(crate) type RealDataCallback = Option<
    unsafe extern "C" fn(
        real_handle: Long,
        data_type: Dword,
        buffer: *mut Byte,
        buffer_size: Dword,
        user_data: *mut c_void,
    ),
>;

pub(crate) type DecDataCallbackMend = Option<
    unsafe extern "C" fn(
        port: i32,
        buffer: *mut c_char,
        size: i32,
        frame_info: *mut FrameInfo,
        user_data: *mut c_void,
        reserved: i32,
    ),
>;

pub(crate) type NetDvrSetSdkInitCfg = unsafe extern "C" fn(i32, *const c_void) -> Bool;
pub(crate) type NetDvrInit = unsafe extern "C" fn() -> Bool;
pub(crate) type NetDvrCleanup = unsafe extern "C" fn() -> Bool;
pub(crate) type NetDvrSetConnectTime = unsafe extern "C" fn(Dword, Dword) -> Bool;
pub(crate) type NetDvrSetReconnect = unsafe extern "C" fn(Dword, Bool) -> Bool;
pub(crate) type NetDvrSetLogToFile = unsafe extern "C" fn(Dword, *const c_char, Bool) -> Bool;
pub(crate) type NetDvrLoginV40 =
    unsafe extern "C" fn(*const NetDvrUserLoginInfo, *mut NetDvrDeviceInfoV40) -> Long;
pub(crate) type NetDvrLogoutV30 = unsafe extern "C" fn(Long) -> Bool;
pub(crate) type NetDvrRealPlayV40 =
    unsafe extern "C" fn(Long, *const NetDvrPreviewInfo, RealDataCallback, *mut c_void) -> Long;
pub(crate) type NetDvrStopRealPlay = unsafe extern "C" fn(Long) -> Bool;
pub(crate) type NetDvrGetLastError = unsafe extern "C" fn() -> Dword;
pub(crate) type NetDvrGetDvrConfig =
    unsafe extern "C" fn(Long, Dword, Long, *mut c_void, Dword, *mut Dword) -> Bool;
pub(crate) type NetDvrGetDeviceAbility =
    unsafe extern "C" fn(Long, Dword, *mut c_char, Dword, *mut c_char, Dword) -> Bool;
pub(crate) type NetDvrGetStdAbility =
    unsafe extern "C" fn(Long, Dword, *mut NetDvrStdAbility) -> Bool;
pub(crate) type NetDvrGetStdConfig =
    unsafe extern "C" fn(Long, Dword, *mut NetDvrStdConfig) -> Bool;
pub(crate) type NetDvrStdXmlConfig =
    unsafe extern "C" fn(Long, *mut NetDvrXmlConfigInput, *mut NetDvrXmlConfigOutput) -> Bool;

pub(crate) type PlayM4GetPort = unsafe extern "C" fn(*mut i32) -> i32;
pub(crate) type PlayM4FreePort = unsafe extern "C" fn(i32) -> i32;
pub(crate) type PlayM4SetStreamOpenMode = unsafe extern "C" fn(i32, u32) -> i32;
pub(crate) type PlayM4OpenStream = unsafe extern "C" fn(i32, *mut Byte, u32, u32) -> i32;
pub(crate) type PlayM4InputData = unsafe extern "C" fn(i32, *mut Byte, u32) -> i32;
pub(crate) type PlayM4Play = unsafe extern "C" fn(i32, Hwnd) -> i32;
pub(crate) type PlayM4Stop = unsafe extern "C" fn(i32) -> i32;
pub(crate) type PlayM4CloseStream = unsafe extern "C" fn(i32) -> i32;
pub(crate) type PlayM4SetDecCallBackMend =
    unsafe extern "C" fn(i32, DecDataCallbackMend, *mut c_void) -> i32;
pub(crate) type PlayM4GetLastError = unsafe extern "C" fn(i32) -> u32;

pub(crate) struct HcNetSdkApi {
    pub(crate) _lib: Library,
    pub(crate) set_sdk_init_cfg: NetDvrSetSdkInitCfg,
    pub(crate) init: NetDvrInit,
    pub(crate) cleanup: NetDvrCleanup,
    pub(crate) set_connect_time: NetDvrSetConnectTime,
    pub(crate) set_reconnect: NetDvrSetReconnect,
    pub(crate) set_log_to_file: NetDvrSetLogToFile,
    pub(crate) login_v40: NetDvrLoginV40,
    pub(crate) logout_v30: NetDvrLogoutV30,
    pub(crate) real_play_v40: NetDvrRealPlayV40,
    pub(crate) stop_real_play: NetDvrStopRealPlay,
    pub(crate) get_last_error: NetDvrGetLastError,
    pub(crate) get_dvr_config: NetDvrGetDvrConfig,
    pub(crate) get_device_ability: NetDvrGetDeviceAbility,
    pub(crate) get_std_ability: NetDvrGetStdAbility,
    pub(crate) get_std_config: NetDvrGetStdConfig,
    pub(crate) std_xml_config: NetDvrStdXmlConfig,
}

impl HcNetSdkApi {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let lib = load_library_global(path)
            .with_context(|| format!("failed to load HCNetSDK library: {}", path.display()))?;
        unsafe {
            Ok(Self {
                set_sdk_init_cfg: *lib.get(b"NET_DVR_SetSDKInitCfg\0")?,
                init: *lib.get(b"NET_DVR_Init\0")?,
                cleanup: *lib.get(b"NET_DVR_Cleanup\0")?,
                set_connect_time: *lib.get(b"NET_DVR_SetConnectTime\0")?,
                set_reconnect: *lib.get(b"NET_DVR_SetReconnect\0")?,
                set_log_to_file: *lib.get(b"NET_DVR_SetLogToFile\0")?,
                login_v40: *lib.get(b"NET_DVR_Login_V40\0")?,
                logout_v30: *lib.get(b"NET_DVR_Logout_V30\0")?,
                real_play_v40: *lib.get(b"NET_DVR_RealPlay_V40\0")?,
                stop_real_play: *lib.get(b"NET_DVR_StopRealPlay\0")?,
                get_last_error: *lib.get(b"NET_DVR_GetLastError\0")?,
                get_dvr_config: *lib.get(b"NET_DVR_GetDVRConfig\0")?,
                get_device_ability: *lib.get(b"NET_DVR_GetDeviceAbility\0")?,
                get_std_ability: *lib.get(b"NET_DVR_GetSTDAbility\0")?,
                get_std_config: *lib.get(b"NET_DVR_GetSTDConfig\0")?,
                std_xml_config: *lib.get(b"NET_DVR_STDXMLConfig\0")?,
                _lib: lib,
            })
        }
    }
}

pub(crate) struct PlayCtrlApi {
    pub(crate) _lib: Library,
    pub(crate) get_port: PlayM4GetPort,
    pub(crate) free_port: PlayM4FreePort,
    pub(crate) set_stream_open_mode: PlayM4SetStreamOpenMode,
    pub(crate) open_stream: PlayM4OpenStream,
    pub(crate) input_data: PlayM4InputData,
    pub(crate) play: PlayM4Play,
    pub(crate) stop: PlayM4Stop,
    pub(crate) close_stream: PlayM4CloseStream,
    pub(crate) set_dec_callback_mend: PlayM4SetDecCallBackMend,
    pub(crate) get_last_error: PlayM4GetLastError,
}

impl PlayCtrlApi {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let lib = load_library_global(path)
            .with_context(|| format!("failed to load PlayCtrl library: {}", path.display()))?;
        unsafe {
            Ok(Self {
                get_port: *lib.get(b"PlayM4_GetPort\0")?,
                free_port: *lib.get(b"PlayM4_FreePort\0")?,
                set_stream_open_mode: *lib.get(b"PlayM4_SetStreamOpenMode\0")?,
                open_stream: *lib.get(b"PlayM4_OpenStream\0")?,
                input_data: *lib.get(b"PlayM4_InputData\0")?,
                play: *lib.get(b"PlayM4_Play\0")?,
                stop: *lib.get(b"PlayM4_Stop\0")?,
                close_stream: *lib.get(b"PlayM4_CloseStream\0")?,
                set_dec_callback_mend: *lib.get(b"PlayM4_SetDecCallBackMend\0")?,
                get_last_error: *lib.get(b"PlayM4_GetLastError\0")?,
                _lib: lib,
            })
        }
    }
}

pub(crate) fn load_library_global(path: &Path) -> Result<Library> {
    #[cfg(unix)]
    {
        use libloading::os::unix::{Library as UnixLibrary, RTLD_GLOBAL, RTLD_NOW};
        let lib = unsafe { UnixLibrary::open(Some(path), RTLD_NOW | RTLD_GLOBAL) }?;
        Ok(lib.into())
    }
    #[cfg(not(unix))]
    {
        let lib = unsafe { Library::new(path) }?;
        Ok(lib)
    }
}

pub(crate) fn preload_sdk_dependencies(sdk_lib_dir: &Path) -> Result<Vec<Library>> {
    let deps = [
        "libhpr.so",
        "libHCCore.so",
        "libNPQos.so",
        "libAudioRender.so",
        "libSuperRender.so",
        "libopenal.so.1",
        "libcrypto.so.1.1",
        "libssl.so.1.1",
        "libz.so",
    ];

    let mut loaded = Vec::new();
    for dep in deps {
        let dep_path = sdk_lib_dir.join(dep);
        if dep_path.exists() {
            let handle = load_library_global(&dep_path)
                .with_context(|| format!("failed to preload dependency {}", dep_path.display()))?;
            loaded.push(handle);
        }
    }
    Ok(loaded)
}

#[derive(Clone)]
pub(crate) struct DecodedFrame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels_argb: Vec<u32>,
    pub(crate) sequence: u64,
}

pub(crate) fn default_sdk_root() -> PathBuf {
    if let Ok(path) = std::env::var("HIK_SDK_ROOT") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_VENDOR_SDK_ROOT)
}

pub(crate) fn ensure_loader_path_reexec(sdk_lib_dir: &Path) -> Result<()> {
    if std::env::var_os(REEXEC_MARKER).is_some() {
        return Ok(());
    }

    let com_dir = sdk_lib_dir.join("HCNetSDKCom");
    let mut merged = format!("{}:{}", sdk_lib_dir.display(), com_dir.display());
    if let Ok(existing) = std::env::var("LD_LIBRARY_PATH")
        && !existing.is_empty()
    {
        merged.push(':');
        merged.push_str(&existing);
    }

    let exe = std::env::current_exe().context("failed to resolve current executable path")?;
    let status = Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env(REEXEC_MARKER, "1")
        .env("LD_LIBRARY_PATH", merged)
        .status()
        .context("failed to re-exec process with SDK loader path")?;

    std::process::exit(status.code().unwrap_or(1));
}
