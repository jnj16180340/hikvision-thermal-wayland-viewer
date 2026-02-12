use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::cli::Args;
use crate::ffi::{
    NetDvrDeviceInfoV40, DEVICE_ABILITY_INFO, DEVICE_NETWORK_ABILITY,
    NET_DVR_GET_MANUALTHERM_CAPABILITIES, NET_DVR_GET_PANORAMAIMAGE_CAPABILITIES,
    NET_DVR_GET_PREVIEW_MODE_CAPABILITIES, NET_DVR_GET_STREAMING_CAPABILITIES,
    NET_DVR_GET_THERMAL_CAPABILITIES, NET_DVR_GET_THERMAL_PIP,
    NET_DVR_GET_THERMAL_PIP_CAPABILITIES, NET_DVR_GET_THERMINTELL_CAPABILITIES,
    NET_DVR_GET_THERMOMETRY_BASICPARAM, NET_DVR_GET_THERMOMETRY_BASICPARAM_CAPABILITIES,
    NET_DVR_GET_THERMOMETRY_MODE, NET_DVR_GET_THERMOMETRY_MODE_CAPABILITIES, STREAM_ABILITY,
    SYSTEM_MANAGEMENT_ABILITY,
};
use crate::report::{discovery_report_to_json, osd_snapshot_report_to_json};
use crate::session::HikSession;
use crate::util::{
    bool_u8_text, build_thermometry_cond_bytes, c_byte_array_to_string, distance_unit_name,
    extract_sdk_error_code, hex_sample, net_dvr_error_name, read_f32_le,
    sanitize_filename_token, stream_proto_name, thermal_overlap_type_name, thermal_pip_mode_name,
    thermometry_mode_name, thermometry_range_name, thermometry_roi_mode_name,
    thermometry_unit_name, truncate_for_report, unix_millis, yes_no,
};
use crate::xml::{
    extract_hints, extract_xml_tags, extract_xml_value, summarize_xml_payload,
};

#[derive(Clone)]
pub(crate) struct DiscoveryProbeRecord {
    pub(crate) category: String,
    pub(crate) name: String,
    pub(crate) target: String,
    pub(crate) success: bool,
    pub(crate) sdk_error: Option<u32>,
    pub(crate) sdk_error_name: Option<String>,
    pub(crate) summary: String,
    pub(crate) payload_bytes: usize,
    pub(crate) status_code: Option<String>,
    pub(crate) status_string: Option<String>,
    pub(crate) sub_status_code: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) hints: Vec<String>,
    pub(crate) payload_sample: String,
    pub(crate) error_text: Option<String>,
}

#[derive(Clone)]
pub(crate) struct DiscoveryReport {
    pub(crate) generated_at_ms: u64,
    pub(crate) camera_ip: String,
    pub(crate) camera_port: u16,
    pub(crate) channel: i32,
    pub(crate) sdk_root: String,
    pub(crate) login_serial: String,
    pub(crate) login_device_type: u16,
    pub(crate) login_analog_channels: u8,
    pub(crate) login_ip_channels_max: u32,
    pub(crate) records: Vec<DiscoveryProbeRecord>,
}

#[derive(Clone)]
pub(crate) struct OsdSnapshotReport {
    pub(crate) generated_at_ms: u64,
    pub(crate) stage: String,
    pub(crate) camera_ip: String,
    pub(crate) camera_port: u16,
    pub(crate) channel: i32,
    pub(crate) sdk_root: String,
    pub(crate) records: Vec<DiscoveryProbeRecord>,
}

impl HikSession {
    pub(crate) fn capture_osd_snapshot(&self, args: &Args, stage: &str) -> Result<PathBuf> {
        let channel = args.channel.max(1);
        let report = OsdSnapshotReport {
            generated_at_ms: unix_millis(),
            stage: stage.to_string(),
            camera_ip: args.ip.clone(),
            camera_port: args.device_port,
            channel,
            sdk_root: args
                .sdk_root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unset>".to_string()),
            records: self.collect_osd_snapshot_records(channel),
        };
        self.write_osd_snapshot_report_json(args, &report)
    }

    fn collect_osd_snapshot_records(&self, channel: i32) -> Vec<DiscoveryProbeRecord> {
        let mut records = Vec::new();
        for (name, path) in osd_snapshot_paths(channel) {
            let mut record = Self::discovery_probe_from_text_result(
                "osd_snapshot",
                &name,
                &path,
                self.query_isapi_xml(&path),
            );
            Self::mark_record_failed_on_response_status(&mut record);
            records.push(record);
        }
        records
    }

    fn mark_record_failed_on_response_status(record: &mut DiscoveryProbeRecord) {
        if !record.success {
            return;
        }
        let Some(code) = record.status_code.as_deref() else {
            return;
        };
        if code == "0" || code == "1" {
            return;
        }

        record.success = false;
        let status = record.status_string.as_deref().unwrap_or("<missing>");
        let sub = record.sub_status_code.as_deref().unwrap_or("<missing>");
        record.summary = format!("statusCode={code} statusString={status} subStatusCode={sub}");
        if record.error_text.is_none() {
            record.error_text = Some(record.payload_sample.clone());
        }
    }

    fn write_osd_snapshot_report_json(
        &self,
        args: &Args,
        report: &OsdSnapshotReport,
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(&args.osd_snapshot_dir).with_context(|| {
            format!(
                "failed to create OSD snapshot dir {}",
                args.osd_snapshot_dir.display()
            )
        })?;
        let filename = format!(
            "osd_snapshot_{}_{}_{}_{}.json",
            sanitize_filename_token(&args.ip),
            args.device_port,
            sanitize_filename_token(&report.stage),
            report.generated_at_ms
        );
        let path = args.osd_snapshot_dir.join(filename);
        std::fs::write(&path, osd_snapshot_report_to_json(report))
            .with_context(|| format!("failed to write OSD snapshot report {}", path.display()))?;
        Ok(path)
    }

    pub(crate) fn log_startup_introspection(
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
}

pub(crate) fn targeted_sweep_paths(channel: i32) -> Vec<(String, String, String)> {
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

pub(crate) fn osd_snapshot_paths(channel: i32) -> Vec<(String, String)> {
    let ch = channel.max(1);
    vec![
        (
            "overlays".to_string(),
            format!("/ISAPI/System/Video/inputs/channels/{ch}/overlays"),
        ),
        (
            "overlays_capabilities".to_string(),
            format!("/ISAPI/System/Video/inputs/channels/{ch}/overlays/capabilities"),
        ),
        (
            "overlay_datetime".to_string(),
            format!("/ISAPI/System/Video/inputs/channels/{ch}/overlays/dateTimeOverlay"),
        ),
        (
            "overlay_datetime_capabilities".to_string(),
            format!(
                "/ISAPI/System/Video/inputs/channels/{ch}/overlays/dateTimeOverlay/capabilities"
            ),
        ),
        (
            "overlay_channel_name".to_string(),
            format!("/ISAPI/System/Video/inputs/channels/{ch}/overlays/channelNameOverlay"),
        ),
        (
            "overlay_battery_power".to_string(),
            format!("/ISAPI/System/Video/inputs/channels/{ch}/overlays/BatteryPowerOverlay"),
        ),
        (
            "overlay_battery_power_capabilities".to_string(),
            format!(
                "/ISAPI/System/Video/inputs/channels/{ch}/overlays/BatteryPowerOverlay/capabilities"
            ),
        ),
        (
            "overlay_text_list".to_string(),
            format!("/ISAPI/System/Video/inputs/channels/{ch}/overlays/text"),
        ),
        (
            "overlay_text_1".to_string(),
            format!("/ISAPI/System/Video/inputs/channels/{ch}/overlays/text/1"),
        ),
        (
            "osd_language".to_string(),
            "/ISAPI/System/Video/inputs/OSDLanguage".to_string(),
        ),
        (
            "ptz_osd_display".to_string(),
            format!("/ISAPI/PTZCtrl/channels/{ch}/PTZOSDDisplay"),
        ),
    ]
}
