use std::ffi::{CString, c_char, c_void};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
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

const STREAME_REALTIME: u32 = 0;
const PLAYM4_BUF_OVER: u32 = 11;
const T_YV12: i32 = 3;
const REEXEC_MARKER: &str = "HIK_SDK_LD_READY";
const DEFAULT_VENDOR_SDK_ROOT: &str = "vendor/linux/x86_64/hik_sdk";

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
        session.login(args)?;
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
                    self.sdk_last_error()
                );
            }
            if (self.hcnetsdk.set_sdk_init_cfg)(
                NET_SDK_INIT_CFG_LIBEAY_PATH,
                libcrypto.as_ptr().cast(),
            ) == 0
            {
                bail!(
                    "NET_DVR_SetSDKInitCfg(LIBEAY_PATH) failed (error {})",
                    self.sdk_last_error()
                );
            }
            if (self.hcnetsdk.set_sdk_init_cfg)(
                NET_SDK_INIT_CFG_SSLEAY_PATH,
                libssl.as_ptr().cast(),
            ) == 0
            {
                bail!(
                    "NET_DVR_SetSDKInitCfg(SSLEAY_PATH) failed (error {})",
                    self.sdk_last_error()
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
                bail!("NET_DVR_Init failed (error {})", self.sdk_last_error());
            }
            self.sdk_initialized = true;

            (self.hcnetsdk.set_connect_time)(args.connect_timeout_ms, args.connect_retries);
            (self.hcnetsdk.set_reconnect)(args.reconnect_interval_ms, 1);
            (self.hcnetsdk.set_log_to_file)(3, log_dir.as_ptr(), 0);
        }
        Ok(())
    }

    fn login(&mut self, args: &Args) -> Result<()> {
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
                self.sdk_last_error()
            );
        }
        Ok(())
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
                self.sdk_last_error()
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
                    eprintln!(
                        "[RealData] PlayM4_OpenStream failed (err={})",
                        (ctx.playctrl.get_last_error)(port)
                    );
                    (ctx.playctrl.free_port)(port);
                    return;
                }
                if (ctx.playctrl.set_dec_callback_mend)(port, Some(dec_data_callback), user_data)
                    == 0
                {
                    eprintln!(
                        "[RealData] PlayM4_SetDecCallBackMend failed (err={})",
                        (ctx.playctrl.get_last_error)(port)
                    );
                    (ctx.playctrl.close_stream)(port);
                    (ctx.playctrl.free_port)(port);
                    return;
                }
                if (ctx.playctrl.play)(port, 0) == 0 {
                    eprintln!(
                        "[RealData] PlayM4_Play failed (err={})",
                        (ctx.playctrl.get_last_error)(port)
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
                        eprintln!("[RealData] PlayM4_InputData failed (err={err})");
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
    args: Args,
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
    fn new(
        args: Args,
        latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Self {
        let blank_len = (args.window_width.max(1) * args.window_height.max(1)) as usize;
        Self {
            args,
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
            (
                self.args.window_width.max(1),
                self.args.window_height.max(1),
            )
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
            .with_title(self.args.window_title.clone())
            .with_inner_size(LogicalSize::new(
                self.args.window_width as f64,
                self.args.window_height as f64,
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
    args: &Args,
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ViewerApp::new(args.clone(), latest_frame, stop_flag);
    event_loop
        .run_app(&mut app)
        .map_err(|err| anyhow!("winit event loop failed: {err}"))?;

    if let Some(err) = app.fatal_error.take() {
        bail!("{err}");
    }
    Ok(())
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
    #[arg(long, default_value = "Hikvision Thermal Viewer (Wayland)")]
    window_title: String,
    #[arg(long, default_value_t = 960)]
    window_width: u32,
    #[arg(long, default_value_t = 720)]
    window_height: u32,
    #[arg(long, default_value = "runtime_logs/sdk_logs_rust")]
    log_dir: PathBuf,
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
    run_wayland_window(&args, latest_frame, stop_flag)?;
    println!("Viewer stopped.");
    Ok(())
}
