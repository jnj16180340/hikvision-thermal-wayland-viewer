mod callbacks;
mod cli;
mod controls;
mod discovery;
mod ffi;
mod report;
mod rtsp;
mod session;
mod util;
mod viewer;
mod xml;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use clap::Parser;

use cli::Args;
use ffi::{DecodedFrame, HcNetSdkApi, PlayCtrlApi, default_sdk_root, ensure_loader_path_reexec, preload_sdk_dependencies};
use rtsp::run_headless_rtsp_publisher;
use session::HikSession;
use viewer::run_wayland_window;

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

    if args.headless_publish_rtsp {
        println!("Login and preview started. Running headless GStreamer appsrc RTSP publisher.");
        run_headless_rtsp_publisher(&args, latest_frame, stop_flag)?;
        println!("Headless publisher stopped.");
    } else {
        println!("Login and preview started. Press Esc, q, or close the window to stop.");
        run_wayland_window(latest_frame, stop_flag)?;
        println!("Viewer stopped.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::discovery::targeted_sweep_paths;
    use crate::rtsp::{normalize_rtsp_path, rtsp_client_host_hint};
    use crate::util::{
        bool_xml_value, extract_sdk_error_code, net_dvr_error_name, playm4_error_name,
        sanitize_filename_token, truncate_for_report,
    };
    use crate::xml::{
        ensure_isapi_response_ok, extract_xml_tags, extract_xml_value,
        extract_xml_value_in_section, extract_xml_values_in_sections, find_xml_tag_bounds,
        replace_xml_tag_value, replace_xml_tag_value_in_all_sections,
        replace_xml_tag_value_in_section, summarize_xml_payload,
    };

    #[test]
    fn normalize_rtsp_path_normalizes_empty_and_missing_slash() {
        assert_eq!(normalize_rtsp_path(""), "/hik");
        assert_eq!(normalize_rtsp_path("hik"), "/hik");
        assert_eq!(normalize_rtsp_path("/hik"), "/hik");
        assert_eq!(normalize_rtsp_path("  /stream  "), "/stream");
    }

    #[test]
    fn rtsp_client_host_hint_rewrites_wildcard_bind_addresses() {
        assert_eq!(rtsp_client_host_hint("0.0.0.0"), "127.0.0.1");
        assert_eq!(rtsp_client_host_hint("::"), "::1");
        assert_eq!(rtsp_client_host_hint("[::]"), "::1");
        assert_eq!(rtsp_client_host_hint("10.15.12.1"), "10.15.12.1");
    }

    #[test]
    fn extract_xml_value_variants_work_for_basic_cases() {
        let xml = "<ResponseStatus><statusCode>4</statusCode><statusString>Invalid Operation</statusString><subStatusCode>notSupport</subStatusCode></ResponseStatus>";
        assert_eq!(
            extract_xml_value(xml, "statusCode").as_deref(),
            Some("4")
        );
        assert_eq!(
            extract_xml_value(xml, "statusString").as_deref(),
            Some("Invalid Operation")
        );

        let overlays = "<TextOverlayList><TextOverlay><id>1</id><enabled>true</enabled></TextOverlay><TextOverlay><id>2</id><enabled>false</enabled></TextOverlay></TextOverlayList>";
        assert_eq!(
            extract_xml_value_in_section(overlays, "TextOverlay", "enabled").as_deref(),
            Some("true")
        );
        assert_eq!(
            extract_xml_values_in_sections(overlays, "TextOverlay", "enabled"),
            vec!["true".to_string(), "false".to_string()]
        );
    }

    #[test]
    fn find_xml_tag_bounds_ignores_prefix_collisions() {
        let xml = "<statusCodeExtra>999</statusCodeExtra><statusCode>1</statusCode>";
        let (start, end, _) = find_xml_tag_bounds(xml, "statusCode", 0).expect("statusCode");
        assert_eq!(&xml[start..end], "1");
    }

    #[test]
    fn replace_xml_tag_value_helpers_update_expected_scope() {
        let xml = "<root><enabled>false</enabled></root>";
        let replaced = replace_xml_tag_value(xml, "enabled", "true").expect("replace");
        assert_eq!(replaced, "<root><enabled>true</enabled></root>");

        let section_xml =
            "<root><A><enabled>false</enabled></A><B><enabled>false</enabled></B></root>";
        let replaced_section =
            replace_xml_tag_value_in_section(section_xml, "B", "enabled", "true")
                .expect("replace section");
        assert_eq!(
            replaced_section,
            "<root><A><enabled>false</enabled></A><B><enabled>true</enabled></B></root>"
        );

        let list_xml = "<TextOverlayList><TextOverlay><enabled>false</enabled></TextOverlay><TextOverlay><enabled>false</enabled></TextOverlay></TextOverlayList>";
        let (replaced_all, count) =
            replace_xml_tag_value_in_all_sections(list_xml, "TextOverlay", "enabled", "true")
                .expect("replace all sections");
        assert_eq!(count, 2);
        assert_eq!(
            extract_xml_values_in_sections(&replaced_all, "TextOverlay", "enabled"),
            vec!["true".to_string(), "true".to_string()]
        );
    }

    #[test]
    fn replace_xml_tag_value_in_all_sections_errors_when_section_missing() {
        let err = replace_xml_tag_value_in_all_sections("<root/>", "TextOverlay", "enabled", "x")
            .expect_err("expected missing section error");
        assert!(err.to_string().contains("section <TextOverlay> not found"));
    }

    #[test]
    fn ensure_isapi_response_ok_accepts_success_and_rejects_failure() {
        let ok_xml = "<ResponseStatus><statusCode>1</statusCode><statusString>OK</statusString></ResponseStatus>";
        ensure_isapi_response_ok("GET", "/ok", ok_xml).expect("statusCode=1 should pass");

        let fail_xml = "<ResponseStatus><statusCode>4</statusCode><statusString>Invalid Operation</statusString><subStatusCode>notSupport</subStatusCode></ResponseStatus>";
        let err = ensure_isapi_response_ok("GET", "/bad", fail_xml)
            .expect_err("statusCode=4 should fail")
            .to_string();
        assert!(err.contains("statusCode=4"));
        assert!(err.contains("subStatusCode=notSupport"));
    }

    #[test]
    fn xml_tag_and_summary_helpers_extract_expected_signals() {
        let xml = "<a:StreamingChannel><ControlProtocol><streamingTransport>RTSP</streamingTransport></ControlProtocol><mode>normal</mode></a:StreamingChannel>";
        let tags = extract_xml_tags(xml, 8);
        assert_eq!(
            tags,
            vec![
                "StreamingChannel".to_string(),
                "ControlProtocol".to_string(),
                "streamingTransport".to_string(),
                "mode".to_string()
            ]
        );

        let summary = summarize_xml_payload(xml);
        assert!(summary.contains("bytes;"));
        assert!(summary.contains("tags=StreamingChannel,ControlProtocol,streamingTransport,mode"));
        assert!(summary.contains("hints=stream,rtsp,mode"));
    }

    #[test]
    fn misc_string_and_error_helpers_behave_as_expected() {
        assert_eq!(bool_xml_value(true), "true");
        assert_eq!(bool_xml_value(false), "false");
        assert_eq!(sanitize_filename_token("10.15.12.1:8000 test"), "10_15_12_1_8000_test");
        assert_eq!(
            extract_sdk_error_code("request failed: SDK error 23 (NET_DVR_NOSUPPORT)"),
            Some(23)
        );
        assert_eq!(extract_sdk_error_code("request failed"), None);
        assert_eq!(truncate_for_report("abcdef", 4), "abcd...(truncated)");
    }

    #[test]
    fn error_name_tables_cover_known_codes() {
        assert_eq!(net_dvr_error_name(17), Some("NET_DVR_PARAMETER_ERROR"));
        assert_eq!(net_dvr_error_name(1000), Some("XML_ABILITY_NOTSUPPORT"));
        assert_eq!(net_dvr_error_name(999_999), None);

        assert_eq!(playm4_error_name(11), Some("PLAYM4_BUF_OVER"));
        assert_eq!(playm4_error_name(0), Some("PLAYM4_NOERROR"));
        assert_eq!(playm4_error_name(123_456), None);
    }

    #[test]
    fn targeted_sweep_paths_include_primary_and_fallback_channels() {
        let paths = targeted_sweep_paths(1);
        assert_eq!(paths.len(), 50);

        assert!(paths
            .iter()
            .any(|(_, name, path)| name == "thermal_ch1_channel"
                && path == "/ISAPI/Thermal/channels/1"));
        assert!(paths
            .iter()
            .any(|(_, name, path)| name == "thermal_ch2_channel"
                && path == "/ISAPI/Thermal/channels/2"));
        assert!(paths.iter().any(|(category, _, _)| category == "isapi_imaging_sweep"));
        assert!(paths.iter().any(|(category, _, _)| category == "isapi_thermal_sweep"));
    }
}
