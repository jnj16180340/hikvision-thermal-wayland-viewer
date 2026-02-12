use std::ffi::CString;
use std::path::Path;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};

use crate::callbacks::{CallbackContext, real_data_callback};
use crate::cli::Args;
use crate::ffi::{
    DecodedFrame, Dword, HcNetSdkApi, Long, NetDvrDeviceInfoV30, NetDvrDeviceInfoV40,
    NetDvrLocalSdkPath, NetDvrPreviewInfo, NetDvrStdAbility, NetDvrStdConfig,
    NetDvrUserLoginInfo, NetDvrXmlConfigInput, NetDvrXmlConfigOutput, PlayCtrlApi,
    NET_DVR_GET_IPPARACFG_V40, NET_SDK_INIT_CFG_LIBEAY_PATH, NET_SDK_INIT_CFG_SDK_PATH,
    NET_SDK_INIT_CFG_SSLEAY_PATH,
};
use crate::util::{buffer_to_text, format_sdk_error, read_u32_le, write_c_char_buf};

pub(crate) struct HikSession {
    pub(crate) hcnetsdk: Arc<HcNetSdkApi>,
    pub(crate) playctrl: Arc<PlayCtrlApi>,
    callback_ctx: *mut CallbackContext,
    pub(crate) user_id: Long,
    real_handle: Long,
    sdk_initialized: bool,
}

impl HikSession {
    pub(crate) fn new(
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

        if args.osd_snapshot {
            match session.capture_osd_snapshot(args, "before_controls") {
                Ok(path) => println!("OSD snapshot written to {}", path.display()),
                Err(err) => eprintln!("OSD snapshot failed (before_controls): {err}"),
            }
        }

        let controls_result = session.apply_requested_controls(args);

        if args.osd_snapshot {
            match session.capture_osd_snapshot(args, "after_controls") {
                Ok(path) => println!("OSD snapshot written to {}", path.display()),
                Err(err) => eprintln!("OSD snapshot failed (after_controls): {err}"),
            }
        }

        controls_result?;
        if args.discover
            && let Err(err) = session.log_startup_introspection(args, &device_info)
        {
            eprintln!("Discovery failed: {err}");
        }
        session.start_preview(args, latest_frame)?;
        Ok(session)
    }

    pub(crate) fn sdk_last_error(&self) -> Dword {
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

    pub(crate) fn query_device_ability_xml(&self, ability_type: Dword) -> Result<String> {
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

    pub(crate) fn query_std_ability_xml(
        &self,
        ability_type: Dword,
        cond: Option<&[u8]>,
    ) -> Result<String> {
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

    pub(crate) fn query_isapi_xml(&self, path: &str) -> Result<String> {
        self.request_isapi_xml("GET", path, None)
    }

    pub(crate) fn put_isapi_xml(&self, path: &str, body: &str) -> Result<String> {
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

        let ok =
            unsafe { (self.hcnetsdk.std_xml_config)(self.user_id, &mut input, &mut output) };
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

    pub(crate) fn query_std_config_raw(
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

    pub(crate) fn query_ipparacfg_v40_summary(&self) -> Result<(u32, u32, u32, u32, u32)> {
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
}

impl Drop for HikSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}
