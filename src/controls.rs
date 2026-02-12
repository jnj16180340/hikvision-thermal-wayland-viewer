use anyhow::{Context, Result, bail};

use crate::cli::{
    Args, DigitalZoomOption, FocusStyleOption, ImageModeOption, NoiseReductionModeOption,
    PaletteOption, PtzZoomLabelOption,
};
use crate::session::HikSession;
use crate::util::bool_xml_value;
use crate::xml::{
    ensure_isapi_response_ok, extract_xml_value, extract_xml_value_in_section,
    extract_xml_values_in_sections, replace_xml_tag_value, replace_xml_tag_value_in_all_sections,
    replace_xml_tag_value_in_section,
};

impl HikSession {
    pub(crate) fn apply_requested_controls(&self, args: &Args) -> Result<()> {
        self.apply_requested_image_controls(args)?;
        self.apply_requested_osd_controls(args)?;
        Ok(())
    }

    fn apply_requested_image_controls(&self, args: &Args) -> Result<()> {
        if args.focus_style.is_none()
            && args.noise_reduction_mode.is_none()
            && args.digital_zoom.is_none()
            && args.palette.is_none()
            && args.image_mode.is_none()
            && args.scene.is_none()
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

        if let Some(image_mode) = args.image_mode {
            self.apply_image_mode(channel, image_mode)?;
            applied.push(format!("image_mode={}", image_mode.as_isapi_value()));
        }

        if let Some(scene) = args.scene.as_ref() {
            self.apply_scene(channel, scene)?;
            applied.push(format!("scene={scene}"));
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

    fn apply_image_mode(&self, channel: i32, image_mode: ImageModeOption) -> Result<()> {
        let path = format!("/ISAPI/Image/channels/{channel}");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read image channel config from {path}"))?;
        let expected = image_mode.as_isapi_value();
        let updated_xml = replace_xml_tag_value(&current_xml, "ImageMode", expected)
            .with_context(|| format!("image mode not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write image channel config to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify image channel config from {path}"))?;
        let actual = extract_xml_value(&verify_xml, "ImageMode").unwrap_or_default();
        if actual != expected {
            bail!(
                "image mode did not stick (expected {}, got {})",
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

    fn apply_scene(&self, channel: i32, scene: &str) -> Result<()> {
        let path = format!("/ISAPI/Image/channels/{channel}");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read image channel config from {path}"))?;

        let updated_xml = replace_xml_tag_value_in_section(&current_xml, "Scene", "mode", scene)
            .or_else(|_| replace_xml_tag_value_in_section(&current_xml, "Scene", "scene", scene))
            .or_else(|_| replace_xml_tag_value(&current_xml, "Scene", scene))
            .with_context(|| format!("scene setting not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write image channel config to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify image channel config from {path}"))?;
        let actual = extract_xml_value_in_section(&verify_xml, "Scene", "mode")
            .or_else(|| extract_xml_value_in_section(&verify_xml, "Scene", "scene"))
            .or_else(|| extract_xml_value(&verify_xml, "Scene"))
            .unwrap_or_default();
        if actual != scene {
            bail!(
                "scene setting did not stick (expected {}, got {})",
                scene,
                if actual.is_empty() {
                    "<missing>"
                } else {
                    &actual
                }
            );
        }
        Ok(())
    }

    fn apply_requested_osd_controls(&self, args: &Args) -> Result<()> {
        if args.osd_date_time_enabled.is_none()
            && args.osd_date_display_enabled.is_none()
            && args.osd_channel_name_enabled.is_none()
            && args.osd_text_enabled.is_none()
            && args.osd_battery_enabled.is_none()
            && args.osd_device_status_battery_enabled.is_none()
            && args.osd_device_status_charging_enabled.is_none()
            && args.osd_device_status_bluetooth_enabled.is_none()
            && args.osd_device_status_dial_enabled.is_none()
            && args.osd_ptz_zoom_label.is_none()
        {
            return Ok(());
        }

        let channel = args.channel.max(1);
        println!("Applying OSD controls on channel {channel}...");
        let mut applied = Vec::new();

        if let Some(enabled) = args.osd_date_time_enabled {
            let path =
                format!("/ISAPI/System/Video/inputs/channels/{channel}/overlays/dateTimeOverlay");
            self.apply_bool_setting(&path, None, "enabled", enabled, "DateTimeOverlay.enabled")?;
            applied.push(format!("date_time_overlay={}", bool_xml_value(enabled)));
        }

        if let Some(enabled) = args.osd_date_display_enabled {
            let path =
                format!("/ISAPI/System/Video/inputs/channels/{channel}/overlays/dateTimeOverlay");
            self.apply_bool_setting(
                &path,
                None,
                "displayDate",
                enabled,
                "DateTimeOverlay.displayDate",
            )?;
            applied.push(format!("date_display_overlay={}", bool_xml_value(enabled)));
        }

        if let Some(enabled) = args.osd_channel_name_enabled {
            let path = format!(
                "/ISAPI/System/Video/inputs/channels/{channel}/overlays/channelNameOverlay"
            );
            self.apply_bool_setting(
                &path,
                None,
                "enabled",
                enabled,
                "channelNameOverlay.enabled",
            )?;
            applied.push(format!("channel_name_overlay={}", bool_xml_value(enabled)));
        }

        if let Some(enabled) = args.osd_battery_enabled {
            let path = format!(
                "/ISAPI/System/Video/inputs/channels/{channel}/overlays/BatteryPowerOverlay"
            );
            self.apply_bool_setting(
                &path,
                None,
                "enabled",
                enabled,
                "BatteryPowerOverlay.enabled",
            )?;
            applied.push(format!("battery_overlay={}", bool_xml_value(enabled)));
        }

        if let Some(enabled) = args.osd_text_enabled {
            let count = self.apply_text_overlays_enabled(channel, enabled)?;
            applied.push(format!(
                "text_overlay={} ({} item{})",
                bool_xml_value(enabled),
                count,
                if count == 1 { "" } else { "s" }
            ));
        }

        let overlays_path = format!("/ISAPI/System/Video/inputs/channels/{channel}/overlays");
        if let Some(enabled) = args.osd_device_status_battery_enabled {
            self.apply_bool_setting(
                &overlays_path,
                Some("DeviceStatusDisplay"),
                "batteryCapacityDisplay",
                enabled,
                "DeviceStatusDisplay.batteryCapacityDisplay",
            )?;
            applied.push(format!("device_status_battery={}", bool_xml_value(enabled)));
        }
        if let Some(enabled) = args.osd_device_status_charging_enabled {
            self.apply_bool_setting(
                &overlays_path,
                Some("DeviceStatusDisplay"),
                "chargingStatusDisplay",
                enabled,
                "DeviceStatusDisplay.chargingStatusDisplay",
            )?;
            applied.push(format!(
                "device_status_charging={}",
                bool_xml_value(enabled)
            ));
        }
        if let Some(enabled) = args.osd_device_status_bluetooth_enabled {
            self.apply_bool_setting(
                &overlays_path,
                Some("DeviceStatusDisplay"),
                "bluetoothStatusDisplay",
                enabled,
                "DeviceStatusDisplay.bluetoothStatusDisplay",
            )?;
            applied.push(format!(
                "device_status_bluetooth={}",
                bool_xml_value(enabled)
            ));
        }
        if let Some(enabled) = args.osd_device_status_dial_enabled {
            self.apply_bool_setting(
                &overlays_path,
                Some("DeviceStatusDisplay"),
                "dialStatusDisplay",
                enabled,
                "DeviceStatusDisplay.dialStatusDisplay",
            )?;
            applied.push(format!("device_status_dial={}", bool_xml_value(enabled)));
        }

        if let Some(mode) = args.osd_ptz_zoom_label {
            self.apply_ptz_zoom_label(channel, mode)?;
            applied.push(format!("ptz_zoom_label={}", mode.as_isapi_value()));
        }

        if !applied.is_empty() {
            println!("  osd_control: {}", applied.join(", "));
        }
        Ok(())
    }

    fn apply_bool_setting(
        &self,
        path: &str,
        section_tag: Option<&str>,
        target_tag: &str,
        enabled: bool,
        setting_name: &str,
    ) -> Result<()> {
        let current_xml = self
            .query_isapi_xml(path)
            .with_context(|| format!("failed to read {setting_name} from {path}"))?;
        let expected = bool_xml_value(enabled);
        let updated_xml = if let Some(section) = section_tag {
            replace_xml_tag_value_in_section(&current_xml, section, target_tag, expected)
                .with_context(|| format!("{setting_name} not exposed at {path}"))?
        } else {
            replace_xml_tag_value(&current_xml, target_tag, expected)
                .with_context(|| format!("{setting_name} not exposed at {path}"))?
        };

        let response = self
            .put_isapi_xml(path, &updated_xml)
            .with_context(|| format!("failed to write {setting_name} to {path}"))?;
        ensure_isapi_response_ok("PUT", path, &response)?;

        let verify_xml = self
            .query_isapi_xml(path)
            .with_context(|| format!("failed to verify {setting_name} from {path}"))?;
        let actual = if let Some(section) = section_tag {
            extract_xml_value_in_section(&verify_xml, section, target_tag)
        } else {
            extract_xml_value(&verify_xml, target_tag)
        }
        .unwrap_or_default();
        if actual != expected {
            bail!(
                "{setting_name} did not stick (expected {}, got {})",
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

    fn apply_text_overlays_enabled(&self, channel: i32, enabled: bool) -> Result<usize> {
        let path = format!("/ISAPI/System/Video/inputs/channels/{channel}/overlays/text");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read text overlays from {path}"))?;
        let expected = bool_xml_value(enabled);
        let (updated_xml, count) =
            replace_xml_tag_value_in_all_sections(&current_xml, "TextOverlay", "enabled", expected)
                .with_context(|| format!("text overlay entries not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write text overlays to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify text overlays from {path}"))?;
        let values = extract_xml_values_in_sections(&verify_xml, "TextOverlay", "enabled");
        if values.is_empty() {
            bail!("text overlay verification failed: no <TextOverlay><enabled> values returned");
        }
        if values.iter().any(|v| v != expected) {
            bail!(
                "text overlay enabled state did not stick for all entries (expected {}, got {})",
                expected,
                values.join(",")
            );
        }
        Ok(count)
    }

    fn apply_ptz_zoom_label(&self, channel: i32, mode: PtzZoomLabelOption) -> Result<()> {
        let path = format!("/ISAPI/PTZCtrl/channels/{channel}/PTZOSDDisplay");
        let current_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to read PTZ OSD settings from {path}"))?;
        let expected = mode.as_isapi_value();
        let updated_xml = replace_xml_tag_value(&current_xml, "zoomlable", expected)
            .or_else(|_| replace_xml_tag_value(&current_xml, "zoomLabel", expected))
            .with_context(|| format!("zoom label OSD setting not exposed at {path}"))?;

        let response = self
            .put_isapi_xml(&path, &updated_xml)
            .with_context(|| format!("failed to write PTZ OSD settings to {path}"))?;
        ensure_isapi_response_ok("PUT", &path, &response)?;

        let verify_xml = self
            .query_isapi_xml(&path)
            .with_context(|| format!("failed to verify PTZ OSD settings from {path}"))?;
        let actual = extract_xml_value(&verify_xml, "zoomlable")
            .or_else(|| extract_xml_value(&verify_xml, "zoomLabel"))
            .unwrap_or_default();
        if actual != expected {
            bail!(
                "PTZ zoom-label OSD mode did not stick (expected {}, got {})",
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
}
