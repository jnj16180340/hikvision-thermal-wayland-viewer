use std::ffi::{CString, c_char, c_void};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, ValueEnum};
use libloading::Library;
use softbuffer::{Context as SoftbufferContext, Surface as SoftbufferSurface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

type Bool = i32;
type Dword = u32;
type Word = u16;
type Long = i32;
type Byte = u8;
type Hwnd = u32;

const NET_DVR_SYSHEAD: Dword = 1;
const NET_DVR_STREAMDATA: Dword = 2;

const NET_SDK_INIT_CFG_SDK_PATH: i32 = 2;
const NET_SDK_INIT_CFG_LIBEAY_PATH: i32 = 3;
const NET_SDK_INIT_CFG_SSLEAY_PATH: i32 = 4;

const NET_DVR_GET_IPPARACFG_V40: Dword = 1062;

const NET_DVR_GET_THERMOMETRY_BASICPARAM: Dword = 3621;
const NET_DVR_GET_THERMAL_CAPABILITIES: Dword = 3634;
const NET_DVR_GET_STREAMING_CAPABILITIES: Dword = 3712;
const NET_DVR_GET_PANORAMAIMAGE_CAPABILITIES: Dword = 3746;
const NET_DVR_GET_PREVIEW_MODE_CAPABILITIES: Dword = 3983;
const NET_DVR_GET_MANUALTHERM_CAPABILITIES: Dword = 6707;
const NET_DVR_GET_THERMINTELL_CAPABILITIES: Dword = 6711;
const NET_DVR_GET_THERMOMETRY_MODE: Dword = 6765;
const NET_DVR_GET_THERMAL_PIP: Dword = 6768;

const NET_DVR_GET_THERMOMETRY_BASICPARAM_CAPABILITIES: Dword = 3620;
const NET_DVR_GET_THERMOMETRY_MODE_CAPABILITIES: Dword = 6764;
const NET_DVR_GET_THERMAL_PIP_CAPABILITIES: Dword = 6767;

const DEVICE_NETWORK_ABILITY: Dword = 0x002;
const DEVICE_ABILITY_INFO: Dword = 0x011;
const STREAM_ABILITY: Dword = 0x012;
const SYSTEM_MANAGEMENT_ABILITY: Dword = 0x013;

const STREAME_REALTIME: u32 = 0;
const PLAYM4_BUF_OVER: u32 = 11;
const T_YV12: i32 = 3;
const REEXEC_MARKER: &str = "HIK_SDK_LD_READY";
const DEFAULT_VENDOR_SDK_ROOT: &str = "vendor/linux/x86_64/hik_sdk";
const DEFAULT_WINDOW_TITLE: &str = "Hikvision Thermal Viewer (Wayland)";
const DEFAULT_WINDOW_WIDTH: u32 = 960;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;

#[repr(C)]
#[derive(Clone, Copy)]
struct NetDvrDeviceInfoV30 {
    serial_number: [Byte; 48],
    alarm_in_port_num: Byte,
    alarm_out_port_num: Byte,
    disk_num: Byte,
    dvr_type: Byte,
    chan_num: Byte,
    start_chan: Byte,
    audio_chan_num: Byte,
    ip_chan_num: Byte,
    zero_chan_num: Byte,
    main_proto: Byte,
    sub_proto: Byte,
    support: Byte,
    support1: Byte,
    support2: Byte,
    dev_type: Word,
    support3: Byte,
    multi_stream_proto: Byte,
    start_dchan: Byte,
    start_dtalk_chan: Byte,
    high_dchan_num: Byte,
    support4: Byte,
    language_type: Byte,
    voice_in_chan_num: Byte,
    start_voice_in_chan_no: Byte,
    support5: Byte,
    support6: Byte,
    mirror_chan_num: Byte,
    start_mirror_chan_no: Word,
    support7: Byte,
    res2: Byte,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NetDvrDeviceInfoV40 {
    device_v30: NetDvrDeviceInfoV30,
    support_lock: Byte,
    retry_login_time: Byte,
    password_level: Byte,
    proxy_type: Byte,
    surplus_lock_time: Dword,
    char_encode_type: Byte,
    support_dev5: Byte,
    support: Byte,
    login_mode: Byte,
    oem_code: Dword,
    residual_validity_i32: i32,
    residual_validity: Byte,
    single_start_dtalk_chan: Byte,
    single_dtalk_chan_nums: Byte,
    password_reset_level: Byte,
    support_stream_encrypt: Byte,
    market_type: Byte,
    res2: [Byte; 238],
}

type LoginResultCallback = Option<
    unsafe extern "C" fn(
        user_id: Long,
        result: Dword,
        device_info: *mut NetDvrDeviceInfoV30,
        user_data: *mut c_void,
    ),
>;

#[repr(C)]
#[derive(Clone, Copy)]
struct NetDvrUserLoginInfo {
    device_address: [c_char; 129],
    use_transport: Byte,
    port: Word,
    username: [c_char; 64],
    password: [c_char; 64],
    cb_login_result: LoginResultCallback,
    user_data: *mut c_void,
    use_async_login: Bool,
    proxy_type: Byte,
    use_utc_time: Byte,
    login_mode: Byte,
    https_mode: Byte,
    proxy_id: Long,
    verify_mode: Byte,
    res3: [Byte; 119],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NetDvrPreviewInfo {
    channel: Long,
    stream_type: Dword,
    link_mode: Dword,
    play_wnd: Hwnd,
    blocked: Dword,
    passback_record: Dword,
    preview_mode: Byte,
    stream_id: [Byte; 32],
    proto_type: Byte,
    res1: Byte,
    video_coding_type: Byte,
    display_buf_num: Dword,
    npq_mode: Byte,
    recv_metadata: Byte,
    data_type: Byte,
    res: [Byte; 213],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NetDvrLocalSdkPath {
    path: [c_char; 256],
    res: [Byte; 128],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameInfo {
    width: i32,
    height: i32,
    stamp: i32,
    frame_type: i32,
    frame_rate: i32,
    frame_num: u32,
}

#[repr(C)]
struct NetDvrStdAbility {
    cond_buffer: *mut c_void,
    cond_size: Dword,
    out_buffer: *mut c_void,
    out_size: Dword,
    status_buffer: *mut c_void,
    status_size: Dword,
    ret_size: Dword,
    res: [Byte; 32],
}

#[repr(C)]
struct NetDvrStdConfig {
    cond_buffer: *mut c_void,
    cond_size: Dword,
    in_buffer: *mut c_void,
    in_size: Dword,
    out_buffer: *mut c_void,
    out_size: Dword,
    status_buffer: *mut c_void,
    status_size: Dword,
    xml_buffer: *mut c_void,
    xml_size: Dword,
    data_type: Byte,
    res: [Byte; 23],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NetDvrThermometryCond {
    size: Dword,
    channel: Dword,
    preset_no: Word,
    res: [Byte; 62],
}

#[repr(C)]
struct NetDvrXmlConfigInput {
    size: Dword,
    request_url: *mut c_void,
    request_url_len: Dword,
    in_buffer: *mut c_void,
    in_buffer_size: Dword,
    recv_timeout: Dword,
    force_encrypt: Byte,
    num_of_multipart: Byte,
    mime_type: Byte,
    res: [Byte; 29],
}

#[repr(C)]
struct NetDvrXmlConfigOutput {
    size: Dword,
    out_buffer: *mut c_void,
    out_buffer_size: Dword,
    returned_xml_size: Dword,
    status_buffer: *mut c_void,
    status_size: Dword,
    data_buffer: *mut c_void,
    num_of_multipart: Byte,
    res: [Byte; 23],
}

type RealDataCallback = Option<
    unsafe extern "C" fn(
        real_handle: Long,
        data_type: Dword,
        buffer: *mut Byte,
        buffer_size: Dword,
        user_data: *mut c_void,
    ),
>;

type DecDataCallbackMend = Option<
    unsafe extern "C" fn(
        port: i32,
        buffer: *mut c_char,
        size: i32,
        frame_info: *mut FrameInfo,
        user_data: *mut c_void,
        reserved: i32,
    ),
>;

type NetDvrSetSdkInitCfg = unsafe extern "C" fn(i32, *const c_void) -> Bool;
type NetDvrInit = unsafe extern "C" fn() -> Bool;
type NetDvrCleanup = unsafe extern "C" fn() -> Bool;
type NetDvrSetConnectTime = unsafe extern "C" fn(Dword, Dword) -> Bool;
type NetDvrSetReconnect = unsafe extern "C" fn(Dword, Bool) -> Bool;
type NetDvrSetLogToFile = unsafe extern "C" fn(Dword, *const c_char, Bool) -> Bool;
type NetDvrLoginV40 =
    unsafe extern "C" fn(*const NetDvrUserLoginInfo, *mut NetDvrDeviceInfoV40) -> Long;
type NetDvrLogoutV30 = unsafe extern "C" fn(Long) -> Bool;
type NetDvrRealPlayV40 =
    unsafe extern "C" fn(Long, *const NetDvrPreviewInfo, RealDataCallback, *mut c_void) -> Long;
type NetDvrStopRealPlay = unsafe extern "C" fn(Long) -> Bool;
type NetDvrGetLastError = unsafe extern "C" fn() -> Dword;
type NetDvrGetDvrConfig =
    unsafe extern "C" fn(Long, Dword, Long, *mut c_void, Dword, *mut Dword) -> Bool;
type NetDvrGetDeviceAbility =
    unsafe extern "C" fn(Long, Dword, *mut c_char, Dword, *mut c_char, Dword) -> Bool;
type NetDvrGetStdAbility = unsafe extern "C" fn(Long, Dword, *mut NetDvrStdAbility) -> Bool;
type NetDvrGetStdConfig = unsafe extern "C" fn(Long, Dword, *mut NetDvrStdConfig) -> Bool;
type NetDvrStdXmlConfig =
    unsafe extern "C" fn(Long, *mut NetDvrXmlConfigInput, *mut NetDvrXmlConfigOutput) -> Bool;

type PlayM4GetPort = unsafe extern "C" fn(*mut i32) -> i32;
type PlayM4FreePort = unsafe extern "C" fn(i32) -> i32;
type PlayM4SetStreamOpenMode = unsafe extern "C" fn(i32, u32) -> i32;
type PlayM4OpenStream = unsafe extern "C" fn(i32, *mut Byte, u32, u32) -> i32;
type PlayM4InputData = unsafe extern "C" fn(i32, *mut Byte, u32) -> i32;
type PlayM4Play = unsafe extern "C" fn(i32, Hwnd) -> i32;
type PlayM4Stop = unsafe extern "C" fn(i32) -> i32;
type PlayM4CloseStream = unsafe extern "C" fn(i32) -> i32;
type PlayM4SetDecCallBackMend = unsafe extern "C" fn(i32, DecDataCallbackMend, *mut c_void) -> i32;
type PlayM4GetLastError = unsafe extern "C" fn(i32) -> u32;

struct HcNetSdkApi {
    _lib: Library,
    set_sdk_init_cfg: NetDvrSetSdkInitCfg,
    init: NetDvrInit,
    cleanup: NetDvrCleanup,
    set_connect_time: NetDvrSetConnectTime,
    set_reconnect: NetDvrSetReconnect,
    set_log_to_file: NetDvrSetLogToFile,
    login_v40: NetDvrLoginV40,
    logout_v30: NetDvrLogoutV30,
    real_play_v40: NetDvrRealPlayV40,
    stop_real_play: NetDvrStopRealPlay,
    get_last_error: NetDvrGetLastError,
    get_dvr_config: NetDvrGetDvrConfig,
    get_device_ability: NetDvrGetDeviceAbility,
    get_std_ability: NetDvrGetStdAbility,
    get_std_config: NetDvrGetStdConfig,
    std_xml_config: NetDvrStdXmlConfig,
}

impl HcNetSdkApi {
    fn load(path: &Path) -> Result<Self> {
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

struct PlayCtrlApi {
    _lib: Library,
    get_port: PlayM4GetPort,
    free_port: PlayM4FreePort,
    set_stream_open_mode: PlayM4SetStreamOpenMode,
    open_stream: PlayM4OpenStream,
    input_data: PlayM4InputData,
    play: PlayM4Play,
    stop: PlayM4Stop,
    close_stream: PlayM4CloseStream,
    set_dec_callback_mend: PlayM4SetDecCallBackMend,
    get_last_error: PlayM4GetLastError,
}

impl PlayCtrlApi {
    fn load(path: &Path) -> Result<Self> {
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

fn load_library_global(path: &Path) -> Result<Library> {
    #[cfg(unix)]
    {
        use libloading::os::unix::{Library as UnixLibrary, RTLD_GLOBAL, RTLD_NOW};
        let lib = unsafe { UnixLibrary::open(Some(path), RTLD_NOW | RTLD_GLOBAL) }?;
        return Ok(lib.into());
    }
    #[cfg(not(unix))]
    {
        let lib = unsafe { Library::new(path) }?;
        Ok(lib)
    }
}

fn preload_sdk_dependencies(sdk_lib_dir: &Path) -> Result<Vec<Library>> {
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
struct DecodedFrame {
    width: u32,
    height: u32,
    pixels_argb: Vec<u32>,
    sequence: u64,
}

struct CallbackContext {
    playctrl: Arc<PlayCtrlApi>,
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    frame_sequence: AtomicU64,
    play_port: AtomicI32,
    playm4_buffer_bytes: u32,
    last_warn_ms: AtomicU64,
}

#[derive(Clone)]
struct DiscoveryProbeRecord {
    category: String,
    name: String,
    target: String,
    success: bool,
    sdk_error: Option<u32>,
    sdk_error_name: Option<String>,
    summary: String,
    payload_bytes: usize,
    status_code: Option<String>,
    status_string: Option<String>,
    sub_status_code: Option<String>,
    tags: Vec<String>,
    hints: Vec<String>,
    payload_sample: String,
    error_text: Option<String>,
}

#[derive(Clone)]
struct DiscoveryReport {
    generated_at_ms: u64,
    camera_ip: String,
    camera_port: u16,
    channel: i32,
    sdk_root: String,
    login_serial: String,
    login_device_type: u16,
    login_analog_channels: u8,
    login_ip_channels_max: u32,
    records: Vec<DiscoveryProbeRecord>,
}

struct HikSession {
    hcnetsdk: Arc<HcNetSdkApi>,
    playctrl: Arc<PlayCtrlApi>,
    callback_ctx: *mut CallbackContext,
    user_id: Long,
    real_handle: Long,
    sdk_initialized: bool,
}

impl HikSession {
    fn new(
        args: &Args,
        sdk_lib_dir: &Path,
        hcnetsdk: Arc<HcNetSdkApi>,
        playctrl: Arc<PlayCtrlApi>,
        latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    ) -> Result<Self> {
        let mut session = Self {
            hcnetsdk,
            playctrl,
            callback_ctx: std::ptr::null_mut(),
            user_id: -1,
            real_handle: -1,
            sdk_initialized: false,
        };

        session.configure_init_paths(sdk_lib_dir)?;
        session.init_sdk(args)?;
        let device_info = session.login(args)?;
        session.apply_requested_image_controls(args)?;
        if args.discover
            && let Err(err) = session.log_startup_introspection(args, &device_info)
        {
            eprintln!("Discovery failed: {err}");
        }
        session.start_preview(args, latest_frame)?;
        Ok(session)
    }

    fn sdk_last_error(&self) -> Dword {
        unsafe { (self.hcnetsdk.get_last_error)() }
    }

    fn configure_init_paths(&self, sdk_lib_dir: &Path) -> Result<()> {
        let mut sdk_path = NetDvrLocalSdkPath {
            path: [0; 256],
            res: [0; 128],
        };
        write_c_char_buf(&mut sdk_path.path, &sdk_lib_dir.to_string_lossy())?;

        let libcrypto = CString::new(
            sdk_lib_dir
                .join("libcrypto.so.1.1")
                .to_string_lossy()
                .as_bytes(),
        )?;
        let libssl = CString::new(
            sdk_lib_dir
                .join("libssl.so.1.1")
                .to_string_lossy()
                .as_bytes(),
        )?;

        unsafe {
            if (self.hcnetsdk.set_sdk_init_cfg)(
                NET_SDK_INIT_CFG_SDK_PATH,
                (&sdk_path as *const NetDvrLocalSdkPath).cast(),
            ) == 0
            {
                bail!(
                    "NET_DVR_SetSDKInitCfg(SDK_PATH) failed (error {})",
                    format_sdk_error(self.sdk_last_error())
                );
            }
            if (self.hcnetsdk.set_sdk_init_cfg)(
                NET_SDK_INIT_CFG_LIBEAY_PATH,
                libcrypto.as_ptr().cast(),
            ) == 0
            {
                bail!(
                    "NET_DVR_SetSDKInitCfg(LIBEAY_PATH) failed (error {})",
                    format_sdk_error(self.sdk_last_error())
                );
            }
            if (self.hcnetsdk.set_sdk_init_cfg)(
                NET_SDK_INIT_CFG_SSLEAY_PATH,
                libssl.as_ptr().cast(),
            ) == 0
            {
                bail!(
                    "NET_DVR_SetSDKInitCfg(SSLEAY_PATH) failed (error {})",
                    format_sdk_error(self.sdk_last_error())
                );
            }
        }

        Ok(())
    }

    fn init_sdk(&mut self, args: &Args) -> Result<()> {
        std::fs::create_dir_all(&args.log_dir)
            .with_context(|| format!("failed to create SDK log dir: {}", args.log_dir.display()))?;

        let log_dir = CString::new(args.log_dir.to_string_lossy().as_bytes())?;
        unsafe {
            if (self.hcnetsdk.init)() == 0 {
                bail!(
                    "NET_DVR_Init failed (error {})",
                    format_sdk_error(self.sdk_last_error())
                );
            }
            self.sdk_initialized = true;

            (self.hcnetsdk.set_connect_time)(args.connect_timeout_ms, args.connect_retries);
            (self.hcnetsdk.set_reconnect)(args.reconnect_interval_ms, 1);
            (self.hcnetsdk.set_log_to_file)(3, log_dir.as_ptr(), 0);
        }
        Ok(())
    }

    fn login(&mut self, args: &Args) -> Result<NetDvrDeviceInfoV40> {
        let mut login = NetDvrUserLoginInfo {
            device_address: [0; 129],
            use_transport: 0,
            port: args.device_port,
            username: [0; 64],
            password: [0; 64],
            cb_login_result: None,
            user_data: std::ptr::null_mut(),
            use_async_login: 0,
            proxy_type: 0,
            use_utc_time: 0,
            login_mode: args.login_mode,
            https_mode: 0,
            proxy_id: -1,
            verify_mode: 0,
            res3: [0; 119],
        };
        write_c_char_buf(&mut login.device_address, &args.ip)?;
        write_c_char_buf(&mut login.username, &args.username)?;
        write_c_char_buf(&mut login.password, &args.password)?;

        let mut device_info = NetDvrDeviceInfoV40 {
            device_v30: NetDvrDeviceInfoV30 {
                serial_number: [0; 48],
                alarm_in_port_num: 0,
                alarm_out_port_num: 0,
                disk_num: 0,
                dvr_type: 0,
                chan_num: 0,
                start_chan: 0,
                audio_chan_num: 0,
                ip_chan_num: 0,
                zero_chan_num: 0,
                main_proto: 0,
                sub_proto: 0,
                support: 0,
                support1: 0,
                support2: 0,
                dev_type: 0,
                support3: 0,
                multi_stream_proto: 0,
                start_dchan: 0,
                start_dtalk_chan: 0,
                high_dchan_num: 0,
                support4: 0,
                language_type: 0,
                voice_in_chan_num: 0,
                start_voice_in_chan_no: 0,
                support5: 0,
                support6: 0,
                mirror_chan_num: 0,
                start_mirror_chan_no: 0,
                support7: 0,
                res2: 0,
            },
            support_lock: 0,
            retry_login_time: 0,
            password_level: 0,
            proxy_type: 0,
            surplus_lock_time: 0,
            char_encode_type: 0,
            support_dev5: 0,
            support: 0,
            login_mode: 0,
            oem_code: 0,
            residual_validity_i32: 0,
            residual_validity: 0,
            single_start_dtalk_chan: 0,
            single_dtalk_chan_nums: 0,
            password_reset_level: 0,
            support_stream_encrypt: 0,
            market_type: 0,
            res2: [0; 238],
        };

        self.user_id = unsafe { (self.hcnetsdk.login_v40)(&login, &mut device_info) };
        if self.user_id < 0 {
            bail!(
                "NET_DVR_Login_V40 failed for {}:{} (error {})",
                args.ip,
                args.device_port,
                format_sdk_error(self.sdk_last_error())
            );
        }
        Ok(device_info)
    }

    fn apply_requested_image_controls(&self, args: &Args) -> Result<()> {
        if args.focus_style.is_none()
            && args.noise_reduction_mode.is_none()
            && args.digital_zoom.is_none()
            && args.palette.is_none()
        {
            return Ok(());
        }

        let channel = args.channel.max(1);
        println!("Applying image controls on channel {channel}...");

        let mut applied = Vec::new();

        if let Some(mode) = args.noise_reduction_mode {
            self.apply_noise_reduction_mode(channel, mode)?;
            applied.push(format!("noise_reduction_mode={}", mode.as_isapi_value()));
        }

        if let Some(style) = args.focus_style {
            self.apply_focus_style(channel, style)?;
            applied.push(format!("focus_style={}", style.as_isapi_value()));
        }

        if let Some(zoom) = args.digital_zoom {
            self.apply_digital_zoom(channel, zoom)?;
            applied.push(format!("digital_zoom={}", zoom.as_isapi_value()));
        }

        if let Some(palette) = args.palette {
            self.apply_palette(channel, palette)?;
            applied.push(format!("palette={}", palette.as_isapi_value()));
        }

        if !applied.is_empty() {
            println!("  image_control: {}", applied.join(", "));
        }

        Ok(())
    }

    fn apply_focus_style(&self, channel: i32, style: FocusStyleOption) -> Result<()> {
        let path = format!("/ISAPI/Image/channels/{channel}/focusConfiguration");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read focus config from {path}"))?;
        let updated_xml = replace_xml_tag_value(&current_xml, "focusStyle", style.as_isapi_value())
            .with_context(|| format!("focus style not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write focus config to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify focus config from {path}"))?;
        let actual = extract_xml_value(&verify_xml, "focusStyle").unwrap_or_default();
        if actual != style.as_isapi_value() {
            bail!(
                "focus style did not stick (expected {}, got {})",
                style.as_isapi_value(),
                if actual.is_empty() {
                    "<missing>"
                } else {
                    &actual
                }
            );
        }
        Ok(())
    }

    fn apply_noise_reduction_mode(
        &self,
        channel: i32,
        mode: NoiseReductionModeOption,
    ) -> Result<()> {
        let path = format!("/ISAPI/Image/channels/{channel}/noiseReduce");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read noise-reduce config from {path}"))?;
        let updated_xml = replace_xml_tag_value(&current_xml, "mode", mode.as_isapi_value())
            .with_context(|| format!("noise-reduction mode not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write noise-reduce config to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify noise-reduce config from {path}"))?;
        let actual = extract_xml_value(&verify_xml, "mode").unwrap_or_default();
        if actual != mode.as_isapi_value() {
            bail!(
                "noise-reduction mode did not stick (expected {}, got {})",
                mode.as_isapi_value(),
                if actual.is_empty() {
                    "<missing>"
                } else {
                    &actual
                }
            );
        }
        Ok(())
    }

    fn apply_digital_zoom(&self, channel: i32, zoom: DigitalZoomOption) -> Result<()> {
        let path = format!("/ISAPI/Image/channels/{channel}");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read image channel config from {path}"))?;
        let expected = zoom.as_isapi_value();

        let updated_xml =
            replace_xml_tag_value_in_section(&current_xml, "DigitalZoom", "ZoomRatio", expected)
                .or_else(|_| {
                    replace_xml_tag_value_in_section(
                        &current_xml,
                        "DigitalZoom",
                        "zoomRatio",
                        expected,
                    )
                })
                .with_context(|| format!("digital zoom not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write image channel config to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify image channel config from {path}"))?;
        let actual = extract_xml_value_in_section(&verify_xml, "DigitalZoom", "ZoomRatio")
            .or_else(|| extract_xml_value_in_section(&verify_xml, "DigitalZoom", "zoomRatio"))
            .unwrap_or_default();
        if actual != expected {
            bail!(
                "digital zoom did not stick (expected {}, got {})",
                expected,
                if actual.is_empty() {
                    "<missing>"
                } else {
                    &actual
                }
            );
        }
        Ok(())
    }

    fn apply_palette(&self, channel: i32, palette: PaletteOption) -> Result<()> {
        let path = format!("/ISAPI/Image/channels/{channel}/Palettes");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read palette config from {path}"))?;
        let expected = palette.as_isapi_value();
        let updated_xml = replace_xml_tag_value(&current_xml, "mode", expected)
            .with_context(|| format!("palette mode not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write palette config to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify palette config from {path}"))?;
        let actual = extract_xml_value(&verify_xml, "mode").unwrap_or_default();
        if actual != expected {
            bail!(
                "palette did not stick (expected {}, got {})",
                expected,
                if actual.is_empty() {
                    "<missing>"
                } else {
                    &actual
                }
            );
        }
        Ok(())
    }

    fn log_startup_introspection(
        &self,
        args: &Args,
        device_info: &NetDvrDeviceInfoV40,
    ) -> Result<()> {
        println!(
            "SDK introspection summary (camera {}:{}):",
            args.ip, args.device_port
        );
        self.log_login_device_summary(device_info);
        self.log_device_ability_summary();
        self.log_stream_topology_summary();
        self.log_std_capability_summary(args.channel);
        self.log_thermal_config_summary(args.channel);
        self.log_isapi_surface_summary();

        let records = self.collect_discovery_records(args);
        let report = self.build_discovery_report(args, device_info, records);
        let report_path = self.write_discovery_report_json(args, &report)?;
        println!("Discovery report written to {}", report_path.display());
        println!("SDK introspection summary complete.");
        Ok(())
    }

    fn log_login_device_summary(&self, device_info: &NetDvrDeviceInfoV40) {
        let v30 = &device_info.device_v30;
        let serial = c_byte_array_to_string(&v30.serial_number);
        let serial_display = if serial.is_empty() {
            "<empty>".to_string()
        } else {
            serial
        };
        let ip_channel_max = v30.ip_chan_num as u32 + ((v30.high_dchan_num as u32) << 8);
        let stdxml_supported = (v30.support4 & 0x02) != 0;

        println!(
            "  login_device: serial={} dev_type={} analog_chans={} ip_chans_max={} start_chan={} start_dchan={} main_proto={} sub_proto={} stdxml_supported={} stream_encrypt_supported={}",
            serial_display,
            v30.dev_type,
            v30.chan_num,
            ip_channel_max,
            v30.start_chan,
            v30.start_dchan,
            stream_proto_name(v30.main_proto),
            stream_proto_name(v30.sub_proto),
            yes_no(stdxml_supported),
            yes_no(device_info.support_stream_encrypt != 0),
        );
    }

    fn build_discovery_report(
        &self,
        args: &Args,
        device_info: &NetDvrDeviceInfoV40,
        records: Vec<DiscoveryProbeRecord>,
    ) -> DiscoveryReport {
        let v30 = &device_info.device_v30;
        DiscoveryReport {
            generated_at_ms: unix_millis(),
            camera_ip: args.ip.clone(),
            camera_port: args.device_port,
            channel: args.channel,
            sdk_root: args
                .sdk_root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unset>".to_string()),
            login_serial: c_byte_array_to_string(&v30.serial_number),
            login_device_type: v30.dev_type,
            login_analog_channels: v30.chan_num,
            login_ip_channels_max: v30.ip_chan_num as u32 + ((v30.high_dchan_num as u32) << 8),
            records,
        }
    }

    fn write_discovery_report_json(
        &self,
        args: &Args,
        report: &DiscoveryReport,
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(&args.discover_report_dir).with_context(|| {
            format!(
                "failed to create discovery report dir {}",
                args.discover_report_dir.display()
            )
        })?;
        let filename = format!(
            "discovery_{}_{}_{}.json",
            sanitize_filename_token(&args.ip),
            args.device_port,
            report.generated_at_ms
        );
        let path = args.discover_report_dir.join(filename);
        std::fs::write(&path, discovery_report_to_json(report))
            .with_context(|| format!("failed to write discovery report {}", path.display()))?;
        Ok(path)
    }

    fn collect_discovery_records(&self, args: &Args) -> Vec<DiscoveryProbeRecord> {
        let mut records = Vec::new();
        let thermal_cond = build_thermometry_cond_bytes(args.channel);

        for (name, ability_type) in [
            ("device_ability_info", DEVICE_ABILITY_INFO),
            ("device_network_ability", DEVICE_NETWORK_ABILITY),
            ("stream_ability", STREAM_ABILITY),
            ("system_management_ability", SYSTEM_MANAGEMENT_ABILITY),
        ] {
            records.push(Self::discovery_probe_from_text_result(
                "device_ability",
                name,
                &format!("ability_type=0x{ability_type:x}"),
                self.query_device_ability_xml(ability_type),
            ));
        }

        match self.query_ipparacfg_v40_summary() {
            Ok((group_num, analog_num, digital_num, start_dchan, returned_bytes)) => {
                let summary = format!(
                    "groups={group_num} analog_chans={analog_num} digital_chans={digital_num} start_dchan={start_dchan} payload_bytes={returned_bytes}"
                );
                records.push(Self::discovery_probe_custom_success(
                    "stream_topology",
                    "ipparacfg_v40",
                    "NET_DVR_GET_IPPARACFG_V40",
                    &summary,
                ));
            }
            Err(err) => records.push(Self::discovery_probe_error(
                "stream_topology",
                "ipparacfg_v40",
                "NET_DVR_GET_IPPARACFG_V40",
                err.to_string(),
            )),
        }

        for (name, ability_type, needs_channel_cond) in [
            (
                "streaming_capabilities",
                NET_DVR_GET_STREAMING_CAPABILITIES,
                false,
            ),
            (
                "preview_mode_capabilities",
                NET_DVR_GET_PREVIEW_MODE_CAPABILITIES,
                false,
            ),
            (
                "thermal_capabilities",
                NET_DVR_GET_THERMAL_CAPABILITIES,
                true,
            ),
            (
                "thermometry_basicparam_capabilities",
                NET_DVR_GET_THERMOMETRY_BASICPARAM_CAPABILITIES,
                true,
            ),
            (
                "thermometry_mode_capabilities",
                NET_DVR_GET_THERMOMETRY_MODE_CAPABILITIES,
                true,
            ),
            (
                "thermal_pip_capabilities",
                NET_DVR_GET_THERMAL_PIP_CAPABILITIES,
                true,
            ),
            (
                "manual_thermometry_capabilities",
                NET_DVR_GET_MANUALTHERM_CAPABILITIES,
                true,
            ),
            (
                "thermal_intelligence_capabilities",
                NET_DVR_GET_THERMINTELL_CAPABILITIES,
                true,
            ),
            (
                "panorama_image_capabilities",
                NET_DVR_GET_PANORAMAIMAGE_CAPABILITIES,
                false,
            ),
        ] {
            let cond = if needs_channel_cond {
                Some(thermal_cond.as_slice())
            } else {
                None
            };
            records.push(Self::discovery_probe_from_text_result(
                "std_capability",
                name,
                &format!("ability_type=0x{ability_type:x}"),
                self.query_std_ability_xml(ability_type, cond),
            ));
        }

        match self.query_std_config_raw(
            NET_DVR_GET_THERMOMETRY_MODE,
            Some(thermal_cond.as_slice()),
            256,
        ) {
            Ok(bytes) => {
                let mode = bytes.get(4).copied().unwrap_or(0xFF);
                let roi_enabled = bytes.get(5).copied().unwrap_or(0xFF);
                let summary = format!(
                    "mode={} ({}) roi_mode={} ({})",
                    mode,
                    thermometry_mode_name(mode),
                    roi_enabled,
                    thermometry_roi_mode_name(roi_enabled)
                );
                records.push(Self::discovery_probe_custom_success(
                    "thermal_config",
                    "thermometry_mode",
                    "NET_DVR_GET_THERMOMETRY_MODE",
                    &format!("{summary}; bytes_sample={}", hex_sample(&bytes, 32)),
                ));
            }
            Err(err) => records.push(Self::discovery_probe_error(
                "thermal_config",
                "thermometry_mode",
                "NET_DVR_GET_THERMOMETRY_MODE",
                err.to_string(),
            )),
        }

        match self.query_std_config_raw(
            NET_DVR_GET_THERMOMETRY_BASICPARAM,
            Some(thermal_cond.as_slice()),
            2048,
        ) {
            Ok(bytes) => {
                let enabled = bytes.get(4).copied().unwrap_or(0xFF);
                let stream_overlay = bytes.get(5).copied().unwrap_or(0xFF);
                let picture_overlay = bytes.get(6).copied().unwrap_or(0xFF);
                let range = bytes.get(7).copied().unwrap_or(0xFF);
                let unit = bytes.get(8).copied().unwrap_or(0xFF);
                let emissivity = read_f32_le(&bytes, 12).unwrap_or(f32::NAN);
                let summary = format!(
                    "enabled={} stream_overlay={} picture_overlay={} range={} ({}) unit={} ({}) emissivity={:.2}",
                    bool_u8_text(enabled),
                    bool_u8_text(stream_overlay),
                    bool_u8_text(picture_overlay),
                    range,
                    thermometry_range_name(range),
                    unit,
                    thermometry_unit_name(unit),
                    emissivity
                );
                records.push(Self::discovery_probe_custom_success(
                    "thermal_config",
                    "thermometry_basic",
                    "NET_DVR_GET_THERMOMETRY_BASICPARAM",
                    &format!("{summary}; bytes_sample={}", hex_sample(&bytes, 64)),
                ));
            }
            Err(err) => records.push(Self::discovery_probe_error(
                "thermal_config",
                "thermometry_basic",
                "NET_DVR_GET_THERMOMETRY_BASICPARAM",
                err.to_string(),
            )),
        }

        match self.query_std_config_raw(
            NET_DVR_GET_THERMAL_PIP,
            Some(thermal_cond.as_slice()),
            1024,
        ) {
            Ok(bytes) => {
                let enabled = bytes.get(4).copied().unwrap_or(0xFF);
                let pip_mode = bytes.get(5).copied().unwrap_or(0xFF);
                let overlap = bytes.get(6).copied().unwrap_or(0xFF);
                let transparency = bytes.get(7).copied().unwrap_or(0xFF);
                let summary = format!(
                    "enabled={} mode={} ({}) overlap_type={} ({}) transparency={}%",
                    bool_u8_text(enabled),
                    pip_mode,
                    thermal_pip_mode_name(pip_mode),
                    overlap,
                    thermal_overlap_type_name(overlap),
                    transparency
                );
                records.push(Self::discovery_probe_custom_success(
                    "thermal_config",
                    "thermal_pip",
                    "NET_DVR_GET_THERMAL_PIP",
                    &format!("{summary}; bytes_sample={}", hex_sample(&bytes, 32)),
                ));
            }
            Err(err) => records.push(Self::discovery_probe_error(
                "thermal_config",
                "thermal_pip",
                "NET_DVR_GET_THERMAL_PIP",
                err.to_string(),
            )),
        }

        for (name, path) in [
            ("isapi_system_device_info", "/ISAPI/System/deviceInfo"),
            ("isapi_streaming_channels", "/ISAPI/Streaming/channels"),
            ("isapi_image_channels", "/ISAPI/Image/channels"),
            (
                "isapi_video_input_channels",
                "/ISAPI/System/Video/inputs/channels",
            ),
            (
                "isapi_thermal_basic_param",
                "/ISAPI/Thermal/channels/1/thermometry/basicParam",
            ),
            (
                "isapi_thermal_thermometry_mode",
                "/ISAPI/Thermal/channels/1/thermometry/mode",
            ),
            (
                "isapi_thermal_thermometry_caps",
                "/ISAPI/Thermal/channels/1/thermometry/capabilities",
            ),
            (
                "isapi_thermal_channel_caps",
                "/ISAPI/Thermal/channels/1/capabilities",
            ),
        ] {
            records.push(Self::discovery_probe_from_text_result(
                "isapi_core",
                name,
                path,
                self.query_isapi_xml(path),
            ));
        }

        for (category, name, path) in targeted_sweep_paths(args.channel) {
            records.push(Self::discovery_probe_from_text_result(
                &category,
                &name,
                &path,
                self.query_isapi_xml(&path),
            ));
        }

        records
    }

    fn discovery_probe_custom_success(
        category: &str,
        name: &str,
        target: &str,
        summary: &str,
    ) -> DiscoveryProbeRecord {
        DiscoveryProbeRecord {
            category: category.to_string(),
            name: name.to_string(),
            target: target.to_string(),
            success: true,
            sdk_error: None,
            sdk_error_name: None,
            summary: summary.to_string(),
            payload_bytes: summary.len(),
            status_code: None,
            status_string: None,
            sub_status_code: None,
            tags: Vec::new(),
            hints: Vec::new(),
            payload_sample: truncate_for_report(summary, 1024),
            error_text: None,
        }
    }

    fn discovery_probe_error(
        category: &str,
        name: &str,
        target: &str,
        error_message: String,
    ) -> DiscoveryProbeRecord {
        let sdk_error = extract_sdk_error_code(&error_message);
        DiscoveryProbeRecord {
            category: category.to_string(),
            name: name.to_string(),
            target: target.to_string(),
            success: false,
            sdk_error,
            sdk_error_name: sdk_error
                .and_then(net_dvr_error_name)
                .map(std::string::ToString::to_string),
            summary: truncate_for_report(&error_message, 240),
            payload_bytes: error_message.len(),
            status_code: extract_xml_value(&error_message, "statusCode"),
            status_string: extract_xml_value(&error_message, "statusString"),
            sub_status_code: extract_xml_value(&error_message, "subStatusCode"),
            tags: extract_xml_tags(&error_message, 12),
            hints: extract_hints(&error_message),
            payload_sample: truncate_for_report(&error_message, 2048),
            error_text: Some(truncate_for_report(&error_message, 2048)),
        }
    }

    fn discovery_probe_from_text_result(
        category: &str,
        name: &str,
        target: &str,
        result: Result<String>,
    ) -> DiscoveryProbeRecord {
        match result {
            Ok(payload) => DiscoveryProbeRecord {
                category: category.to_string(),
                name: name.to_string(),
                target: target.to_string(),
                success: true,
                sdk_error: None,
                sdk_error_name: None,
                summary: summarize_xml_payload(&payload),
                payload_bytes: payload.len(),
                status_code: extract_xml_value(&payload, "statusCode"),
                status_string: extract_xml_value(&payload, "statusString"),
                sub_status_code: extract_xml_value(&payload, "subStatusCode"),
                tags: extract_xml_tags(&payload, 12),
                hints: extract_hints(&payload),
                payload_sample: truncate_for_report(&payload, 2048),
                error_text: None,
            },
            Err(err) => Self::discovery_probe_error(category, name, target, err.to_string()),
        }
    }

    fn log_device_ability_summary(&self) {
        let queries = [
            ("device_ability_info", DEVICE_ABILITY_INFO),
            ("device_network_ability", DEVICE_NETWORK_ABILITY),
            ("stream_ability", STREAM_ABILITY),
            ("system_management_ability", SYSTEM_MANAGEMENT_ABILITY),
        ];

        for (label, ability_type) in queries {
            match self.query_device_ability_xml(ability_type) {
                Ok(xml) => println!("  {label}: {}", summarize_xml_payload(&xml)),
                Err(err) => println!("  {label}: unavailable ({err})"),
            }
        }
    }

    fn log_stream_topology_summary(&self) {
        match self.query_ipparacfg_v40_summary() {
            Ok((group_num, analog_num, digital_num, start_dchan, returned_bytes)) => println!(
                "  ipparacfg_v40: groups={} analog_chans={} digital_chans={} start_dchan={} payload_bytes={}",
                group_num, analog_num, digital_num, start_dchan, returned_bytes
            ),
            Err(err) => println!("  ipparacfg_v40: unavailable ({err})"),
        }
    }

    fn log_std_capability_summary(&self, channel: i32) {
        let thermal_cond = build_thermometry_cond_bytes(channel);
        let queries = [
            (
                "streaming_capabilities",
                NET_DVR_GET_STREAMING_CAPABILITIES,
                false,
            ),
            (
                "preview_mode_capabilities",
                NET_DVR_GET_PREVIEW_MODE_CAPABILITIES,
                false,
            ),
            (
                "thermal_capabilities",
                NET_DVR_GET_THERMAL_CAPABILITIES,
                true,
            ),
            (
                "thermometry_basicparam_capabilities",
                NET_DVR_GET_THERMOMETRY_BASICPARAM_CAPABILITIES,
                true,
            ),
            (
                "thermometry_mode_capabilities",
                NET_DVR_GET_THERMOMETRY_MODE_CAPABILITIES,
                true,
            ),
            (
                "thermal_pip_capabilities",
                NET_DVR_GET_THERMAL_PIP_CAPABILITIES,
                true,
            ),
            (
                "manual_thermometry_capabilities",
                NET_DVR_GET_MANUALTHERM_CAPABILITIES,
                true,
            ),
            (
                "thermal_intelligence_capabilities",
                NET_DVR_GET_THERMINTELL_CAPABILITIES,
                true,
            ),
            (
                "panorama_image_capabilities",
                NET_DVR_GET_PANORAMAIMAGE_CAPABILITIES,
                false,
            ),
        ];

        for (label, ability_type, needs_channel_cond) in queries {
            let cond = if needs_channel_cond {
                Some(thermal_cond.as_slice())
            } else {
                None
            };
            match self.query_std_ability_xml(ability_type, cond) {
                Ok(xml) => println!("  {label}: {}", summarize_xml_payload(&xml)),
                Err(err) => println!("  {label}: unavailable ({err})"),
            }
        }
    }

    fn log_thermal_config_summary(&self, channel: i32) {
        let thermal_cond = build_thermometry_cond_bytes(channel);

        match self.query_std_config_raw(
            NET_DVR_GET_THERMOMETRY_MODE,
            Some(thermal_cond.as_slice()),
            256,
        ) {
            Ok(bytes) => {
                let mode = bytes.get(4).copied().unwrap_or(0xFF);
                let roi_enabled = bytes.get(5).copied().unwrap_or(0xFF);
                println!(
                    "  thermometry_mode: mode={} ({}) roi_mode={} ({})",
                    mode,
                    thermometry_mode_name(mode),
                    roi_enabled,
                    thermometry_roi_mode_name(roi_enabled)
                );
            }
            Err(err) => println!("  thermometry_mode: unavailable ({err})"),
        }

        match self.query_std_config_raw(
            NET_DVR_GET_THERMOMETRY_BASICPARAM,
            Some(thermal_cond.as_slice()),
            2048,
        ) {
            Ok(bytes) => {
                let enabled = bytes.get(4).copied().unwrap_or(0xFF);
                let stream_overlay = bytes.get(5).copied().unwrap_or(0xFF);
                let picture_overlay = bytes.get(6).copied().unwrap_or(0xFF);
                let range = bytes.get(7).copied().unwrap_or(0xFF);
                let unit = bytes.get(8).copied().unwrap_or(0xFF);
                let curve_mode = bytes.get(9).copied().unwrap_or(0xFF);
                let fire_image_mode = bytes.get(10).copied().unwrap_or(0xFF);
                let show_temp_strip = bytes.get(11).copied().unwrap_or(0xFF);
                let emissivity = read_f32_le(&bytes, 12).unwrap_or(f32::NAN);
                let distance_unit = bytes.get(16).copied().unwrap_or(0xFF);
                let humidity = bytes.get(17).copied().unwrap_or(0xFF);

                println!(
                    "  thermometry_basic: enabled={} stream_overlay={} picture_overlay={} range={} ({}) unit={} ({}) curve_mode={} fire_image_mode={} show_temp_strip={} emissivity={:.2} distance_unit={} ({}) env_humidity={}%",
                    bool_u8_text(enabled),
                    bool_u8_text(stream_overlay),
                    bool_u8_text(picture_overlay),
                    range,
                    thermometry_range_name(range),
                    unit,
                    thermometry_unit_name(unit),
                    curve_mode,
                    fire_image_mode,
                    bool_u8_text(show_temp_strip),
                    emissivity,
                    distance_unit,
                    distance_unit_name(distance_unit),
                    humidity
                );
            }
            Err(err) => println!("  thermometry_basic: unavailable ({err})"),
        }

        match self.query_std_config_raw(
            NET_DVR_GET_THERMAL_PIP,
            Some(thermal_cond.as_slice()),
            1024,
        ) {
            Ok(bytes) => {
                let enabled = bytes.get(4).copied().unwrap_or(0xFF);
                let pip_mode = bytes.get(5).copied().unwrap_or(0xFF);
                let overlap = bytes.get(6).copied().unwrap_or(0xFF);
                let transparency = bytes.get(7).copied().unwrap_or(0xFF);
                println!(
                    "  thermal_pip: enabled={} mode={} ({}) overlap_type={} ({}) transparency={}%",
                    bool_u8_text(enabled),
                    pip_mode,
                    thermal_pip_mode_name(pip_mode),
                    overlap,
                    thermal_overlap_type_name(overlap),
                    transparency
                );
            }
            Err(err) => println!("  thermal_pip: unavailable ({err})"),
        }
    }

    fn query_device_ability_xml(&self, ability_type: Dword) -> Result<String> {
        let mut out = vec![0_u8; 256 * 1024];
        let ok = unsafe {
            (self.hcnetsdk.get_device_ability)(
                self.user_id,
                ability_type,
                std::ptr::null_mut(),
                0,
                out.as_mut_ptr().cast(),
                out.len() as Dword,
            )
        };
        if ok == 0 {
            bail!("SDK error {}", format_sdk_error(self.sdk_last_error()));
        }
        Ok(buffer_to_text(&out, None))
    }

    fn query_std_ability_xml(&self, ability_type: Dword, cond: Option<&[u8]>) -> Result<String> {
        let mut out = vec![0_u8; 256 * 1024];
        let mut status = vec![0_u8; 64 * 1024];
        let mut ability = NetDvrStdAbility {
            cond_buffer: cond.map_or(std::ptr::null_mut(), |c| c.as_ptr().cast_mut().cast()),
            cond_size: cond.map_or(0, |c| c.len() as Dword),
            out_buffer: out.as_mut_ptr().cast(),
            out_size: out.len() as Dword,
            status_buffer: status.as_mut_ptr().cast(),
            status_size: status.len() as Dword,
            ret_size: 0,
            res: [0; 32],
        };

        let ok =
            unsafe { (self.hcnetsdk.get_std_ability)(self.user_id, ability_type, &mut ability) };
        if ok != 0 {
            let mut payload = buffer_to_text(&out, Some(ability.ret_size as usize));
            if payload.is_empty() {
                payload = buffer_to_text(&status, None);
            }
            if !payload.is_empty() {
                return Ok(payload);
            }
        }

        // Fallback: some capabilities are exposed through GetDeviceAbility.
        let mut fallback_out = vec![0_u8; 256 * 1024];
        let fallback_ok = unsafe {
            (self.hcnetsdk.get_device_ability)(
                self.user_id,
                ability_type,
                cond.map_or(std::ptr::null_mut(), |c| c.as_ptr().cast_mut().cast()),
                cond.map_or(0, |c| c.len() as Dword),
                fallback_out.as_mut_ptr().cast(),
                fallback_out.len() as Dword,
            )
        };
        if fallback_ok == 0 {
            bail!("SDK error {}", format_sdk_error(self.sdk_last_error()));
        }
        Ok(buffer_to_text(&fallback_out, None))
    }

    fn query_isapi_xml(&self, path: &str) -> Result<String> {
        self.request_isapi_xml("GET", path, None)
    }

    fn put_isapi_xml(&self, path: &str, body: &str) -> Result<String> {
        self.request_isapi_xml("PUT", path, Some(body))
    }

    fn request_isapi_xml(&self, method: &str, path: &str, body: Option<&str>) -> Result<String> {
        let request = format!("{method} {path}");
        let mut request_buf = request.into_bytes();
        request_buf.push(0);

        let mut body_buf = body.map(|xml| xml.as_bytes().to_vec());
        let (in_buffer, in_buffer_size) = if let Some(ref mut data) = body_buf {
            (data.as_mut_ptr().cast(), data.len() as Dword)
        } else {
            (std::ptr::null_mut(), 0)
        };

        let mut out = vec![0_u8; 256 * 1024];
        let mut status = vec![0_u8; 16 * 1024];

        let mut input = NetDvrXmlConfigInput {
            size: std::mem::size_of::<NetDvrXmlConfigInput>() as Dword,
            request_url: request_buf.as_mut_ptr().cast(),
            request_url_len: request_buf.len().saturating_sub(1) as Dword,
            in_buffer,
            in_buffer_size,
            recv_timeout: 5000,
            force_encrypt: 0,
            num_of_multipart: 0,
            mime_type: 0,
            res: [0; 29],
        };
        let mut output = NetDvrXmlConfigOutput {
            size: std::mem::size_of::<NetDvrXmlConfigOutput>() as Dword,
            out_buffer: out.as_mut_ptr().cast(),
            out_buffer_size: out.len() as Dword,
            returned_xml_size: 0,
            status_buffer: status.as_mut_ptr().cast(),
            status_size: status.len() as Dword,
            data_buffer: std::ptr::null_mut(),
            num_of_multipart: 0,
            res: [0; 23],
        };

        let ok = unsafe { (self.hcnetsdk.std_xml_config)(self.user_id, &mut input, &mut output) };
        if ok == 0 {
            let status_text = buffer_to_text(&status, None);
            if status_text.is_empty() {
                bail!("SDK error {}", format_sdk_error(self.sdk_last_error()));
            }
            bail!(
                "SDK error {} ({status_text})",
                format_sdk_error(self.sdk_last_error())
            );
        }

        let mut payload = buffer_to_text(&out, Some(output.returned_xml_size as usize));
        if payload.is_empty() {
            payload = buffer_to_text(&status, None);
        }
        Ok(payload)
    }

    fn log_isapi_surface_summary(&self) {
        let requests = [
            ("isapi_system_device_info", "/ISAPI/System/deviceInfo"),
            ("isapi_streaming_channels", "/ISAPI/Streaming/channels"),
            ("isapi_image_channels", "/ISAPI/Image/channels"),
            (
                "isapi_video_input_channels",
                "/ISAPI/System/Video/inputs/channels",
            ),
            (
                "isapi_thermal_basic_param",
                "/ISAPI/Thermal/channels/1/thermometry/basicParam",
            ),
            (
                "isapi_thermal_thermometry_mode",
                "/ISAPI/Thermal/channels/1/thermometry/mode",
            ),
            (
                "isapi_thermal_thermometry_caps",
                "/ISAPI/Thermal/channels/1/thermometry/capabilities",
            ),
            (
                "isapi_thermal_channel_caps",
                "/ISAPI/Thermal/channels/1/capabilities",
            ),
        ];

        for (label, path) in requests {
            match self.query_isapi_xml(path) {
                Ok(xml) => println!("  {label}: {}", summarize_xml_payload(&xml)),
                Err(err) => println!("  {label}: unavailable ({err})"),
            }
        }
    }

    fn query_std_config_raw(
        &self,
        command: Dword,
        cond: Option<&[u8]>,
        out_size: usize,
    ) -> Result<Vec<u8>> {
        let mut out = vec![0_u8; out_size.max(128)];
        let mut status = vec![0_u8; 16 * 1024];

        let mut cfg = NetDvrStdConfig {
            cond_buffer: cond.map_or(std::ptr::null_mut(), |c| c.as_ptr().cast_mut().cast()),
            cond_size: cond.map_or(0, |c| c.len() as Dword),
            in_buffer: std::ptr::null_mut(),
            in_size: 0,
            out_buffer: out.as_mut_ptr().cast(),
            out_size: out.len() as Dword,
            status_buffer: status.as_mut_ptr().cast(),
            status_size: status.len() as Dword,
            xml_buffer: std::ptr::null_mut(),
            xml_size: 0,
            data_type: 0,
            res: [0; 23],
        };

        let ok = unsafe { (self.hcnetsdk.get_std_config)(self.user_id, command, &mut cfg) };
        if ok == 0 {
            let status_text = buffer_to_text(&status, None);
            if status_text.is_empty() {
                bail!("SDK error {}", format_sdk_error(self.sdk_last_error()));
            }
            bail!(
                "SDK error {} ({status_text})",
                format_sdk_error(self.sdk_last_error())
            );
        }
        Ok(out)
    }

    fn query_ipparacfg_v40_summary(&self) -> Result<(u32, u32, u32, u32, u32)> {
        let mut out = vec![0_u8; 512 * 1024];
        let mut returned = 0_u32;
        let ok = unsafe {
            (self.hcnetsdk.get_dvr_config)(
                self.user_id,
                NET_DVR_GET_IPPARACFG_V40,
                0,
                out.as_mut_ptr().cast(),
                out.len() as Dword,
                &mut returned,
            )
        };
        if ok == 0 {
            bail!("SDK error {}", format_sdk_error(self.sdk_last_error()));
        }
        if returned < 20 {
            bail!("short response ({returned} bytes)");
        }

        let group_num = read_u32_le(&out, 4).unwrap_or_default();
        let analog_num = read_u32_le(&out, 8).unwrap_or_default();
        let digital_num = read_u32_le(&out, 12).unwrap_or_default();
        let start_dchan = read_u32_le(&out, 16).unwrap_or_default();
        Ok((group_num, analog_num, digital_num, start_dchan, returned))
    }

    fn start_preview(
        &mut self,
        args: &Args,
        latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    ) -> Result<()> {
        let callback_ctx = Box::new(CallbackContext {
            playctrl: Arc::clone(&self.playctrl),
            latest_frame,
            frame_sequence: AtomicU64::new(0),
            play_port: AtomicI32::new(-1),
            playm4_buffer_bytes: args.playm4_buffer_bytes,
            last_warn_ms: AtomicU64::new(0),
        });
        self.callback_ctx = Box::into_raw(callback_ctx);

        let preview = NetDvrPreviewInfo {
            channel: args.channel,
            stream_type: args.stream_type,
            link_mode: args.link_mode,
            play_wnd: 0,
            blocked: 1,
            passback_record: 1,
            preview_mode: 0,
            stream_id: [0; 32],
            proto_type: args.proto_type,
            res1: 0,
            video_coding_type: 0,
            display_buf_num: 1,
            npq_mode: 0,
            recv_metadata: 0,
            data_type: 0,
            res: [0; 213],
        };

        self.real_handle = unsafe {
            (self.hcnetsdk.real_play_v40)(
                self.user_id,
                &preview,
                Some(real_data_callback),
                self.callback_ctx.cast(),
            )
        };
        if self.real_handle < 0 {
            bail!(
                "NET_DVR_RealPlay_V40 failed (error {})",
                format_sdk_error(self.sdk_last_error())
            );
        }
        Ok(())
    }

    fn stop_preview(&mut self) {
        if self.real_handle >= 0 {
            unsafe {
                (self.hcnetsdk.stop_real_play)(self.real_handle);
            }
            self.real_handle = -1;
        }

        if !self.callback_ctx.is_null() {
            let ctx = unsafe { &*self.callback_ctx };
            let port = ctx.play_port.swap(-1, Ordering::SeqCst);
            if port >= 0 {
                unsafe {
                    (ctx.playctrl.stop)(port);
                    (ctx.playctrl.close_stream)(port);
                    (ctx.playctrl.free_port)(port);
                }
            }
        }
    }

    fn logout(&mut self) {
        if self.user_id >= 0 {
            unsafe {
                (self.hcnetsdk.logout_v30)(self.user_id);
            }
            self.user_id = -1;
        }
    }

    fn cleanup_sdk(&mut self) {
        if self.sdk_initialized {
            unsafe {
                (self.hcnetsdk.cleanup)();
            }
            self.sdk_initialized = false;
        }
    }

    fn shutdown(&mut self) {
        self.stop_preview();
        self.logout();
        self.cleanup_sdk();

        if !self.callback_ctx.is_null() {
            unsafe {
                drop(Box::from_raw(self.callback_ctx));
            }
            self.callback_ctx = std::ptr::null_mut();
        }
    }
}

impl Drop for HikSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

unsafe extern "C" fn real_data_callback(
    _real_handle: Long,
    data_type: Dword,
    buffer: *mut Byte,
    buffer_size: Dword,
    user_data: *mut c_void,
) {
    if user_data.is_null() || buffer.is_null() || buffer_size == 0 {
        return;
    }
    let ctx = unsafe { &*(user_data as *const CallbackContext) };

    match data_type {
        NET_DVR_SYSHEAD => {
            if ctx.play_port.load(Ordering::SeqCst) >= 0 {
                return;
            }
            let mut port = -1_i32;
            unsafe {
                if (ctx.playctrl.get_port)(&mut port) == 0 {
                    eprintln!("[RealData] PlayM4_GetPort failed");
                    return;
                }
                if (ctx.playctrl.set_stream_open_mode)(port, STREAME_REALTIME) == 0 {
                    eprintln!("[RealData] PlayM4_SetStreamOpenMode failed");
                    (ctx.playctrl.free_port)(port);
                    return;
                }
                if (ctx.playctrl.open_stream)(port, buffer, buffer_size, ctx.playm4_buffer_bytes)
                    == 0
                {
                    let err = (ctx.playctrl.get_last_error)(port);
                    eprintln!(
                        "[RealData] PlayM4_OpenStream failed (err={})",
                        format_playm4_error(err)
                    );
                    (ctx.playctrl.free_port)(port);
                    return;
                }
                if (ctx.playctrl.set_dec_callback_mend)(port, Some(dec_data_callback), user_data)
                    == 0
                {
                    let err = (ctx.playctrl.get_last_error)(port);
                    eprintln!(
                        "[RealData] PlayM4_SetDecCallBackMend failed (err={})",
                        format_playm4_error(err)
                    );
                    (ctx.playctrl.close_stream)(port);
                    (ctx.playctrl.free_port)(port);
                    return;
                }
                if (ctx.playctrl.play)(port, 0) == 0 {
                    let err = (ctx.playctrl.get_last_error)(port);
                    eprintln!(
                        "[RealData] PlayM4_Play failed (err={})",
                        format_playm4_error(err)
                    );
                    (ctx.playctrl.close_stream)(port);
                    (ctx.playctrl.free_port)(port);
                    return;
                }
            }
            ctx.play_port.store(port, Ordering::SeqCst);
        }
        NET_DVR_STREAMDATA => {
            let port = ctx.play_port.load(Ordering::SeqCst);
            if port < 0 {
                return;
            }
            let mut ok = false;
            for _ in 0..40 {
                let ret = unsafe { (ctx.playctrl.input_data)(port, buffer, buffer_size) };
                if ret != 0 {
                    ok = true;
                    break;
                }
                let err = unsafe { (ctx.playctrl.get_last_error)(port) };
                if err != PLAYM4_BUF_OVER {
                    let now_ms = unix_millis();
                    let prev = ctx.last_warn_ms.load(Ordering::Relaxed);
                    if now_ms.saturating_sub(prev) > 1000 {
                        ctx.last_warn_ms.store(now_ms, Ordering::Relaxed);
                        eprintln!(
                            "[RealData] PlayM4_InputData failed (err={})",
                            format_playm4_error(err)
                        );
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            if !ok {
                let _ = ok;
            }
        }
        _ => {}
    }
}

unsafe extern "C" fn dec_data_callback(
    _port: i32,
    buffer: *mut c_char,
    size: i32,
    frame_info: *mut FrameInfo,
    user_data: *mut c_void,
    _reserved: i32,
) {
    if user_data.is_null() || buffer.is_null() || frame_info.is_null() || size <= 0 {
        return;
    }

    let ctx = unsafe { &*(user_data as *const CallbackContext) };
    let info = unsafe { *frame_info };

    if info.frame_type != T_YV12 || info.width <= 0 || info.height <= 0 {
        return;
    }

    let width = info.width as usize;
    let height = info.height as usize;
    let expected = width * height * 3 / 2;
    if (size as usize) < expected {
        return;
    }

    let src = unsafe { std::slice::from_raw_parts(buffer as *const u8, expected) };
    let mut pixels_argb = Vec::with_capacity(width * height);
    yv12_to_argb(src, width, height, &mut pixels_argb);

    let seq = ctx.frame_sequence.fetch_add(1, Ordering::Relaxed) + 1;
    let frame = DecodedFrame {
        width: width as u32,
        height: height as u32,
        pixels_argb,
        sequence: seq,
    };
    if let Ok(mut guard) = ctx.latest_frame.lock() {
        *guard = Some(frame);
    }
}

fn yv12_to_argb(src: &[u8], width: usize, height: usize, out: &mut Vec<u32>) {
    let y_size = width * height;
    let uv_size = y_size / 4;
    let y_plane = &src[..y_size];
    let v_plane = &src[y_size..(y_size + uv_size)];
    let u_plane = &src[(y_size + uv_size)..(y_size + uv_size * 2)];

    out.clear();
    out.resize(y_size, 0);

    for y in 0..height {
        let uv_row = (y / 2) * (width / 2);
        let y_row = y * width;
        for x in 0..width {
            let yv = y_plane[y_row + x] as i32;
            let uv_idx = uv_row + (x / 2);
            let u = u_plane[uv_idx] as i32;
            let v = v_plane[uv_idx] as i32;

            let c = yv - 16;
            let d = u - 128;
            let e = v - 128;

            let r = clamp_u8((298 * c + 409 * e + 128) >> 8);
            let g = clamp_u8((298 * c - 100 * d - 208 * e + 128) >> 8);
            let b = clamp_u8((298 * c + 516 * d + 128) >> 8);

            out[y_row + x] = 0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
        }
    }
}

fn clamp_u8(v: i32) -> u8 {
    if v < 0 {
        0
    } else if v > 255 {
        255
    } else {
        v as u8
    }
}

fn net_dvr_error_name(code: u32) -> Option<&'static str> {
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

fn format_sdk_error(code: u32) -> String {
    match net_dvr_error_name(code) {
        Some(name) => format!("{code} ({name})"),
        None => code.to_string(),
    }
}

fn playm4_error_name(code: u32) -> Option<&'static str> {
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

fn format_playm4_error(code: u32) -> String {
    match playm4_error_name(code) {
        Some(name) => format!("{code} ({name})"),
        None => code.to_string(),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

fn bool_u8_text(v: u8) -> &'static str {
    match v {
        0 => "no",
        1 => "yes",
        _ => "unknown",
    }
}

fn c_byte_array_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn stream_proto_name(v: u8) -> &'static str {
    match v {
        0 => "private",
        1 => "rtsp",
        2 => "private+rtsp",
        _ => "unknown",
    }
}

fn thermometry_mode_name(v: u8) -> &'static str {
    match v {
        0 => "normal",
        1 => "expert",
        _ => "unknown",
    }
}

fn thermometry_roi_mode_name(v: u8) -> &'static str {
    match v {
        0 => "reserved",
        1 => "close",
        2 => "open",
        _ => "unknown",
    }
}

fn thermometry_unit_name(v: u8) -> &'static str {
    match v {
        0 => "C",
        1 => "F",
        2 => "K",
        _ => "unknown",
    }
}

fn distance_unit_name(v: u8) -> &'static str {
    match v {
        0 => "meter",
        1 => "feet",
        2 => "centimeter",
        _ => "unknown",
    }
}

fn thermometry_range_name(v: u8) -> &'static str {
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

fn thermal_pip_mode_name(v: u8) -> &'static str {
    match v {
        0 => "overlay",
        1 => "fusion",
        2 => "normal",
        _ => "unknown",
    }
}

fn thermal_overlap_type_name(v: u8) -> &'static str {
    match v {
        0 => "visible-over-thermal",
        1 => "thermal-over-visible",
        _ => "unknown",
    }
}

fn buffer_to_text(buf: &[u8], preferred_len: Option<usize>) -> String {
    let mut end = preferred_len.unwrap_or(buf.len()).min(buf.len());
    if end == 0 {
        end = buf.len();
    }
    if let Some(nul_pos) = buf[..end].iter().position(|&b| b == 0) {
        end = nul_pos;
    }
    String::from_utf8_lossy(&buf[..end]).trim().to_string()
}

fn extract_xml_tags(xml: &str, max_tags: usize) -> Vec<String> {
    let bytes = xml.as_bytes();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < bytes.len() && tags.len() < max_tags {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        i += 1;
        if i >= bytes.len() {
            break;
        }
        let kind = bytes[i];
        if kind == b'/' || kind == b'?' || kind == b'!' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'>' || b == b'/' || b.is_ascii_whitespace() {
                break;
            }
            i += 1;
        }
        if i > start {
            let raw_tag = &xml[start..i];
            let tag = raw_tag.rsplit(':').next().unwrap_or(raw_tag).to_string();
            if !tag.is_empty() && !tags.iter().any(|existing| existing == &tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

fn summarize_xml_payload(xml: &str) -> String {
    if xml.is_empty() {
        return "empty".to_string();
    }
    let tags = extract_xml_tags(xml, 10);
    let hints = extract_hints(xml);

    let tag_text = if tags.is_empty() {
        "tags=<none>".to_string()
    } else {
        format!("tags={}", tags.join(","))
    };
    let hint_text = if hints.is_empty() {
        "hints=<none>".to_string()
    } else {
        format!("hints={}", hints.join(","))
    };
    format!("{} bytes; {tag_text}; {hint_text}", xml.len())
}

fn extract_hints(xml: &str) -> Vec<String> {
    let lower = xml.to_ascii_lowercase();
    let mut hints = Vec::new();
    for keyword in [
        "thermometry",
        "thermal",
        "pip",
        "fusion",
        "stream",
        "preview",
        "rtsp",
        "srtp",
        "manual",
        "mode",
        "dehaze",
        "wdr",
        "focus",
    ] {
        if lower.contains(keyword) {
            hints.push(keyword.to_string());
        }
    }
    hints
}

fn extract_xml_value(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)?;
    let value_start = start + open.len();
    let end = text[value_start..].find(&close)?;
    Some(text[value_start..value_start + end].trim().to_string())
}

fn extract_xml_value_in_section(text: &str, section_tag: &str, target_tag: &str) -> Option<String> {
    let (section_value_start, section_value_end, _) = find_xml_tag_bounds(text, section_tag, 0)?;
    let section = &text[section_value_start..section_value_end];
    let (value_start, value_end, _) = find_xml_tag_bounds(section, target_tag, 0)?;
    Some(section[value_start..value_end].trim().to_string())
}

fn ensure_isapi_response_ok(method: &str, path: &str, payload: &str) -> Result<()> {
    if let Some(code) = extract_xml_value(payload, "statusCode")
        && code != "1"
        && code != "0"
    {
        let status_string =
            extract_xml_value(payload, "statusString").unwrap_or_else(|| "<missing>".to_string());
        let sub_status_code =
            extract_xml_value(payload, "subStatusCode").unwrap_or_else(|| "<missing>".to_string());
        bail!(
            "{method} {path} failed with statusCode={code}, statusString={status_string}, subStatusCode={sub_status_code}, payload={}",
            truncate_for_report(payload, 512)
        );
    }
    Ok(())
}

fn find_xml_tag_bounds(text: &str, tag: &str, start_idx: usize) -> Option<(usize, usize, usize)> {
    let open_prefix = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut search_start = start_idx.min(text.len());

    while let Some(rel_idx) = text[search_start..].find(&open_prefix) {
        let open_start = search_start + rel_idx;
        let after_tag = open_start + open_prefix.len();
        let next = text.as_bytes().get(after_tag).copied();
        let valid_boundary = matches!(
            next,
            Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')
        );
        if !valid_boundary {
            search_start = after_tag;
            continue;
        }

        let open_end_rel = text[after_tag..].find('>')?;
        let value_start = after_tag + open_end_rel + 1;
        let value_end_rel = text[value_start..].find(&close)?;
        let value_end = value_start + value_end_rel;
        let close_end = value_end + close.len();
        return Some((value_start, value_end, close_end));
    }

    None
}

fn replace_xml_tag_value(xml: &str, tag: &str, value: &str) -> Result<String> {
    let (value_start, value_end, _) =
        find_xml_tag_bounds(xml, tag, 0).ok_or_else(|| anyhow!("tag <{tag}> not found"))?;
    let mut out = String::with_capacity(xml.len() + value.len());
    out.push_str(&xml[..value_start]);
    out.push_str(value);
    out.push_str(&xml[value_end..]);
    Ok(out)
}

fn replace_xml_tag_value_in_section(
    xml: &str,
    section_tag: &str,
    target_tag: &str,
    value: &str,
) -> Result<String> {
    let section_open = format!("<{section_tag}");
    let section_start = xml
        .find(&section_open)
        .ok_or_else(|| anyhow!("section <{section_tag}> not found"))?;
    let section_close = format!("</{section_tag}>");
    let section_close_rel = xml[section_start..]
        .find(&section_close)
        .ok_or_else(|| anyhow!("section </{section_tag}> not found"))?;
    let section_end = section_start + section_close_rel + section_close.len();

    let section_xml = &xml[section_start..section_end];
    let replaced_section = replace_xml_tag_value(section_xml, target_tag, value)
        .with_context(|| format!("tag <{target_tag}> not found inside <{section_tag}> section"))?;

    let mut out = String::with_capacity(xml.len() + value.len());
    out.push_str(&xml[..section_start]);
    out.push_str(&replaced_section);
    out.push_str(&xml[section_end..]);
    Ok(out)
}

fn extract_sdk_error_code(text: &str) -> Option<u32> {
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

fn truncate_for_report(text: &str, max_chars: usize) -> String {
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

fn sanitize_filename_token(token: &str) -> String {
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

fn hex_sample(bytes: &[u8], max_bytes: usize) -> String {
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

fn json_escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_string(text: &str) -> String {
    format!("\"{}\"", json_escape(text))
}

fn write_json_opt_string(out: &mut String, key: &str, value: &Option<String>, with_comma: bool) {
    out.push_str(&json_string(key));
    out.push(':');
    match value {
        Some(v) => out.push_str(&json_string(v)),
        None => out.push_str("null"),
    }
    if with_comma {
        out.push(',');
    }
}

fn write_json_string_array(out: &mut String, key: &str, values: &[String], with_comma: bool) {
    out.push_str(&json_string(key));
    out.push(':');
    out.push('[');
    for (idx, item) in values.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&json_string(item));
    }
    out.push(']');
    if with_comma {
        out.push(',');
    }
}

fn discovery_report_to_json(report: &DiscoveryReport) -> String {
    let mut out = String::new();
    out.push('{');
    out.push_str(&json_string("generated_at_ms"));
    out.push(':');
    out.push_str(&report.generated_at_ms.to_string());
    out.push(',');
    out.push_str(&json_string("camera_ip"));
    out.push(':');
    out.push_str(&json_string(&report.camera_ip));
    out.push(',');
    out.push_str(&json_string("camera_port"));
    out.push(':');
    out.push_str(&report.camera_port.to_string());
    out.push(',');
    out.push_str(&json_string("channel"));
    out.push(':');
    out.push_str(&report.channel.to_string());
    out.push(',');
    out.push_str(&json_string("sdk_root"));
    out.push(':');
    out.push_str(&json_string(&report.sdk_root));
    out.push(',');

    out.push_str(&json_string("login_device"));
    out.push(':');
    out.push('{');
    out.push_str(&json_string("serial"));
    out.push(':');
    out.push_str(&json_string(&report.login_serial));
    out.push(',');
    out.push_str(&json_string("device_type"));
    out.push(':');
    out.push_str(&report.login_device_type.to_string());
    out.push(',');
    out.push_str(&json_string("analog_channels"));
    out.push(':');
    out.push_str(&report.login_analog_channels.to_string());
    out.push(',');
    out.push_str(&json_string("ip_channels_max"));
    out.push(':');
    out.push_str(&report.login_ip_channels_max.to_string());
    out.push('}');
    out.push(',');

    out.push_str(&json_string("records"));
    out.push(':');
    out.push('[');
    for (idx, record) in report.records.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push('{');
        out.push_str(&json_string("category"));
        out.push(':');
        out.push_str(&json_string(&record.category));
        out.push(',');
        out.push_str(&json_string("name"));
        out.push(':');
        out.push_str(&json_string(&record.name));
        out.push(',');
        out.push_str(&json_string("target"));
        out.push(':');
        out.push_str(&json_string(&record.target));
        out.push(',');
        out.push_str(&json_string("success"));
        out.push(':');
        out.push_str(if record.success { "true" } else { "false" });
        out.push(',');
        out.push_str(&json_string("sdk_error"));
        out.push(':');
        match record.sdk_error {
            Some(v) => out.push_str(&v.to_string()),
            None => out.push_str("null"),
        }
        out.push(',');
        write_json_opt_string(&mut out, "sdk_error_name", &record.sdk_error_name, true);
        out.push_str(&json_string("summary"));
        out.push(':');
        out.push_str(&json_string(&record.summary));
        out.push(',');
        out.push_str(&json_string("payload_bytes"));
        out.push(':');
        out.push_str(&record.payload_bytes.to_string());
        out.push(',');
        write_json_opt_string(&mut out, "status_code", &record.status_code, true);
        write_json_opt_string(&mut out, "status_string", &record.status_string, true);
        write_json_opt_string(&mut out, "sub_status_code", &record.sub_status_code, true);
        write_json_string_array(&mut out, "tags", &record.tags, true);
        write_json_string_array(&mut out, "hints", &record.hints, true);
        out.push_str(&json_string("payload_sample"));
        out.push(':');
        out.push_str(&json_string(&record.payload_sample));
        out.push(',');
        write_json_opt_string(&mut out, "error_text", &record.error_text, false);
        out.push('}');
    }
    out.push(']');
    out.push('}');
    out.push('\n');
    out
}

fn targeted_sweep_paths(channel: i32) -> Vec<(String, String, String)> {
    let mut channels = Vec::new();
    for ch in [channel.max(1), 1, 2] {
        if !channels.contains(&ch) {
            channels.push(ch);
        }
    }

    let thermal_templates = [
        ("channel", "/ISAPI/Thermal/channels/{ch}"),
        (
            "channel_capabilities",
            "/ISAPI/Thermal/channels/{ch}/capabilities",
        ),
        (
            "thermometry_basic_param",
            "/ISAPI/Thermal/channels/{ch}/thermometry/basicParam",
        ),
        (
            "thermometry_mode",
            "/ISAPI/Thermal/channels/{ch}/thermometry/mode",
        ),
        (
            "thermometry_capabilities",
            "/ISAPI/Thermal/channels/{ch}/thermometry/capabilities",
        ),
        (
            "thermometry_preset_info",
            "/ISAPI/Thermal/channels/{ch}/thermometry/presetInfo",
        ),
        (
            "thermometry_alarm_rule",
            "/ISAPI/Thermal/channels/{ch}/thermometry/alarmRule",
        ),
        (
            "thermometry_diff_comparison",
            "/ISAPI/Thermal/channels/{ch}/thermometry/diffComparison",
        ),
        (
            "thermometry_trigger",
            "/ISAPI/Thermal/channels/{ch}/thermometry/trigger",
        ),
        (
            "thermometry_schedule",
            "/ISAPI/Thermal/channels/{ch}/thermometry/schedule",
        ),
        ("thermal_pip", "/ISAPI/Thermal/channels/{ch}/pip"),
        (
            "manual_thermometry",
            "/ISAPI/Thermal/channels/{ch}/manualThermometry",
        ),
        (
            "manual_thermometry_basic_param",
            "/ISAPI/Thermal/channels/{ch}/manualThermometry/basicParam",
        ),
        (
            "manual_thermometry_capabilities",
            "/ISAPI/Thermal/channels/{ch}/manualThermometry/capabilities",
        ),
        ("thermal_intell", "/ISAPI/Thermal/channels/{ch}/thermIntell"),
        (
            "thermal_intell_capabilities",
            "/ISAPI/Thermal/channels/{ch}/thermIntell/capabilities",
        ),
    ];

    let imaging_templates = [
        ("image_channel", "/ISAPI/Image/channels/{ch}"),
        (
            "image_capabilities",
            "/ISAPI/Image/channels/{ch}/capabilities",
        ),
        ("image_focus", "/ISAPI/Image/channels/{ch}/focus"),
        ("image_exposure", "/ISAPI/Image/channels/{ch}/exposure"),
        (
            "image_white_balance",
            "/ISAPI/Image/channels/{ch}/whiteBalance",
        ),
        (
            "image_wide_dynamic_range",
            "/ISAPI/Image/channels/{ch}/wideDynamicRange",
        ),
        ("image_dehaze", "/ISAPI/Image/channels/{ch}/dehaze"),
        (
            "image_noise_reduce",
            "/ISAPI/Image/channels/{ch}/noiseReduce",
        ),
        ("image_scene", "/ISAPI/Image/channels/{ch}/scene"),
    ];

    let mut out = Vec::new();
    for ch in channels {
        for (suffix, template) in thermal_templates {
            let path = template.replace("{ch}", &ch.to_string());
            out.push((
                "isapi_thermal_sweep".to_string(),
                format!("thermal_ch{ch}_{suffix}"),
                path,
            ));
        }
        for (suffix, template) in imaging_templates {
            let path = template.replace("{ch}", &ch.to_string());
            out.push((
                "isapi_imaging_sweep".to_string(),
                format!("imaging_ch{ch}_{suffix}"),
                path,
            ));
        }
    }
    out
}

fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_f32_le(buf: &[u8], offset: usize) -> Option<f32> {
    let bytes = buf.get(offset..offset + 4)?;
    Some(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn build_thermometry_cond_bytes(channel: i32) -> Vec<u8> {
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

fn write_c_char_buf(buf: &mut [c_char], value: &str) -> Result<()> {
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

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn default_sdk_root() -> PathBuf {
    if let Ok(path) = std::env::var("HIK_SDK_ROOT") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_VENDOR_SDK_ROOT)
}

fn ensure_loader_path_reexec(sdk_lib_dir: &Path) -> Result<()> {
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

type WindowSoftbufferContext = SoftbufferContext<Arc<Window>>;
type WindowSoftbufferSurface = SoftbufferSurface<Arc<Window>, Arc<Window>>;

struct ViewerApp {
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    stop_flag: Arc<AtomicBool>,
    window: Option<Arc<Window>>,
    context: Option<WindowSoftbufferContext>,
    surface: Option<WindowSoftbufferSurface>,
    last_seq: u64,
    last_frame: Option<DecodedFrame>,
    surface_size: (u32, u32),
    blank: Vec<u32>,
    fatal_error: Option<String>,
}

impl ViewerApp {
    fn new(latest_frame: Arc<Mutex<Option<DecodedFrame>>>, stop_flag: Arc<AtomicBool>) -> Self {
        let blank_len = (DEFAULT_WINDOW_WIDTH.max(1) * DEFAULT_WINDOW_HEIGHT.max(1)) as usize;
        Self {
            latest_frame,
            stop_flag,
            window: None,
            context: None,
            surface: None,
            last_seq: 0,
            last_frame: None,
            surface_size: (0, 0),
            blank: vec![0xFF10_1010_u32; blank_len],
            fatal_error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, message: impl Into<String>) {
        let msg = message.into();
        eprintln!("{msg}");
        self.fatal_error = Some(msg);
        self.stop_flag.store(true, Ordering::Relaxed);
        event_loop.exit();
    }

    fn ensure_blank_len(&mut self, len: usize) {
        if self.blank.len() != len {
            self.blank.resize(len, 0xFF10_1010);
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().map(Arc::clone) else {
            return;
        };
        if self.surface.is_none() {
            return;
        }

        if let Ok(guard) = self.latest_frame.lock()
            && let Some(frame) = guard.as_ref()
            && frame.sequence != self.last_seq
        {
            self.last_seq = frame.sequence;
            self.last_frame = Some(frame.clone());
        }

        let (w, h) = if let Some(frame) = self.last_frame.as_ref() {
            (frame.width, frame.height)
        } else {
            (DEFAULT_WINDOW_WIDTH.max(1), DEFAULT_WINDOW_HEIGHT.max(1))
        };

        if w == 0 || h == 0 {
            return;
        }

        let target_len = (w * h) as usize;
        self.ensure_blank_len(target_len);

        let mut surface = self
            .surface
            .take()
            .expect("surface was checked to be Some above");

        let draw_result = (|| -> std::result::Result<(), String> {
            if self.surface_size != (w, h)
                && let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h))
            {
                surface
                    .resize(nw, nh)
                    .map_err(|err| format!("softbuffer resize failed: {err}"))?;
                let _ = window.request_inner_size(LogicalSize::new(w as f64, h as f64));
                self.surface_size = (w, h);
            }

            let mut buffer = surface
                .buffer_mut()
                .map_err(|err| format!("softbuffer buffer_mut failed: {err}"))?;

            if let Some(frame) = self
                .last_frame
                .as_ref()
                .filter(|f| f.pixels_argb.len() == target_len)
            {
                if buffer.len() == frame.pixels_argb.len() {
                    buffer.copy_from_slice(&frame.pixels_argb);
                } else if buffer.len() == self.blank.len() {
                    buffer.copy_from_slice(&self.blank);
                } else {
                    for px in buffer.iter_mut() {
                        *px = 0xFF10_1010;
                    }
                }
            } else if buffer.len() == self.blank.len() {
                buffer.copy_from_slice(&self.blank);
            } else {
                for px in buffer.iter_mut() {
                    *px = 0xFF10_1010;
                }
            }

            buffer
                .present()
                .map_err(|err| format!("softbuffer present failed: {err}"))?;
            Ok(())
        })();

        self.surface = Some(surface);

        if let Err(err) = draw_result {
            self.fail(event_loop, err);
        }
    }
}

impl ApplicationHandler for ViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title(DEFAULT_WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(
                DEFAULT_WINDOW_WIDTH as f64,
                DEFAULT_WINDOW_HEIGHT as f64,
            ));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(err) => {
                self.fail(event_loop, format!("failed to create window: {err}"));
                return;
            }
        };

        let context = match SoftbufferContext::new(Arc::clone(&window)) {
            Ok(c) => c,
            Err(err) => {
                self.fail(
                    event_loop,
                    format!("failed to create softbuffer context: {err}"),
                );
                return;
            }
        };

        let surface = match SoftbufferSurface::new(&context, Arc::clone(&window)) {
            Ok(s) => s,
            Err(err) => {
                self.fail(
                    event_loop,
                    format!("failed to create softbuffer surface: {err}"),
                );
                return;
            }
        };

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.last_seq = 0;
        self.last_frame = None;
        self.surface_size = (0, 0);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.stop_flag.load(Ordering::Relaxed) {
            event_loop.exit();
            return;
        }

        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                self.stop_flag.store(true, Ordering::Relaxed);
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let is_escape = matches!(event.logical_key, Key::Named(NamedKey::Escape));
                    let is_q = matches!(&event.logical_key, Key::Character(ch) if ch.eq_ignore_ascii_case("q"));
                    if is_escape || is_q {
                        self.stop_flag.store(true, Ordering::Relaxed);
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.stop_flag.load(Ordering::Relaxed) {
            event_loop.exit();
            return;
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

fn run_wayland_window(
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ViewerApp::new(latest_frame, stop_flag);
    event_loop
        .run_app(&mut app)
        .map_err(|err| anyhow!("winit event loop failed: {err}"))?;

    if let Some(err) = app.fatal_error.take() {
        bail!("{err}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FocusStyleOption {
    #[value(name = "AUTO")]
    Auto,
    #[value(name = "MANUAL")]
    Manual,
    #[value(name = "SEMIAUTOMATIC")]
    Semiautomatic,
}

impl FocusStyleOption {
    fn as_isapi_value(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Manual => "MANUAL",
            Self::Semiautomatic => "SEMIAUTOMATIC",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum NoiseReductionModeOption {
    Close,
    General,
    Advanced,
}

impl NoiseReductionModeOption {
    fn as_isapi_value(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::General => "general",
            Self::Advanced => "advanced",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DigitalZoomOption {
    #[value(name = "1x")]
    X1,
    #[value(name = "2x")]
    X2,
    #[value(name = "4x")]
    X4,
    #[value(name = "8x")]
    X8,
}

impl DigitalZoomOption {
    fn as_isapi_value(self) -> &'static str {
        match self {
            Self::X1 => "1x",
            Self::X2 => "2x",
            Self::X4 => "4x",
            Self::X8 => "8x",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PaletteOption {
    #[value(name = "WhiteHot")]
    WhiteHot,
    #[value(name = "BlackHot")]
    BlackHot,
    #[value(name = "RedHot")]
    RedHot,
    #[value(name = "Fusion1")]
    Fusion1,
}

impl PaletteOption {
    fn as_isapi_value(self) -> &'static str {
        match self {
            Self::WhiteHot => "WhiteHot",
            Self::BlackHot => "BlackHot",
            Self::RedHot => "RedHot",
            Self::Fusion1 => "Fusion1",
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(name = "hikvision-thermal-wayland-viewer")]
#[command(about = "Minimal Hikvision viewer in Rust (Wayland-only rendering path).")]
struct Args {
    #[arg(long)]
    sdk_root: Option<PathBuf>,
    #[arg(long, default_value = "10.15.12.1")]
    ip: String,
    #[arg(long, default_value_t = 8000)]
    device_port: u16,
    #[arg(long, env = "HIK_USER", default_value = "admin")]
    username: String,
    #[arg(long, env = "HIK_PASSWORD")]
    password: String,
    #[arg(long, default_value_t = 1)]
    channel: i32,
    #[arg(long, value_enum, ignore_case = true)]
    focus_style: Option<FocusStyleOption>,
    #[arg(long, value_enum, ignore_case = true)]
    noise_reduction_mode: Option<NoiseReductionModeOption>,
    #[arg(long, value_enum, ignore_case = true)]
    digital_zoom: Option<DigitalZoomOption>,
    #[arg(long, value_enum, ignore_case = true)]
    palette: Option<PaletteOption>,
    #[arg(long, default_value_t = 0)]
    stream_type: u32,
    #[arg(long, default_value_t = 0)]
    link_mode: u32,
    #[arg(long, default_value_t = 0)]
    proto_type: u8,
    #[arg(long, default_value_t = 0)]
    login_mode: u8,
    #[arg(long, default_value_t = 2 * 1024 * 1024)]
    playm4_buffer_bytes: u32,
    #[arg(long, default_value_t = 3000)]
    connect_timeout_ms: u32,
    #[arg(long, default_value_t = 2)]
    connect_retries: u32,
    #[arg(long, default_value_t = 10000)]
    reconnect_interval_ms: u32,
    #[arg(long, default_value = "runtime_logs/sdk_logs_rust")]
    log_dir: PathBuf,
    #[arg(long, default_value_t = false)]
    discover: bool,
    #[arg(long, default_value = "runtime_logs/discovery_reports")]
    discover_report_dir: PathBuf,
}

fn main() -> Result<()> {
    let mut args = Args::parse();
    if args.password.is_empty() {
        bail!("password is required (set --password or HIK_PASSWORD)");
    }

    let sdk_root = args.sdk_root.clone().unwrap_or_else(default_sdk_root);
    args.sdk_root = Some(sdk_root.clone());

    let sdk_lib_dir = sdk_root.join("lib");
    if !sdk_lib_dir.exists() {
        bail!("SDK lib directory not found: {}", sdk_lib_dir.display());
    }
    ensure_loader_path_reexec(&sdk_lib_dir)?;

    println!(
        "Starting Rust Wayland viewer using SDK at {} and camera {}:{}...",
        sdk_root.display(),
        args.ip,
        args.device_port
    );

    let _runtime_deps = preload_sdk_dependencies(&sdk_lib_dir)
        .context("failed while preloading vendored SDK dependencies")?;
    let hcnetsdk = Arc::new(HcNetSdkApi::load(&sdk_lib_dir.join("libhcnetsdk.so"))?);
    let playctrl = Arc::new(PlayCtrlApi::load(&sdk_lib_dir.join("libPlayCtrl.so"))?);
    let latest_frame: Arc<Mutex<Option<DecodedFrame>>> = Arc::new(Mutex::new(None));
    let stop_flag = Arc::new(AtomicBool::new(false));

    let _session = HikSession::new(
        &args,
        &sdk_lib_dir,
        Arc::clone(&hcnetsdk),
        Arc::clone(&playctrl),
        Arc::clone(&latest_frame),
    )
    .context("failed to initialize Hikvision SDK preview session")?;

    println!("Login and preview started. Press Esc, q, or close the window to stop.");
    run_wayland_window(latest_frame, stop_flag)?;
    println!("Viewer stopped.");
    Ok(())
}
