use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::rtsp::{DEFAULT_RTSP_ADDRESS, DEFAULT_RTSP_PATH};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum FocusStyleOption {
    #[value(name = "AUTO")]
    Auto,
    #[value(name = "MANUAL")]
    Manual,
    #[value(name = "SEMIAUTOMATIC")]
    Semiautomatic,
}

impl FocusStyleOption {
    pub(crate) fn as_isapi_value(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Manual => "MANUAL",
            Self::Semiautomatic => "SEMIAUTOMATIC",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum NoiseReductionModeOption {
    Close,
    General,
    Advanced,
}

impl NoiseReductionModeOption {
    pub(crate) fn as_isapi_value(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::General => "general",
            Self::Advanced => "advanced",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum DigitalZoomOption {
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
    pub(crate) fn as_isapi_value(self) -> &'static str {
        match self {
            Self::X1 => "1x",
            Self::X2 => "2x",
            Self::X4 => "4x",
            Self::X8 => "8x",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum PaletteOption {
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
    pub(crate) fn as_isapi_value(self) -> &'static str {
        match self {
            Self::WhiteHot => "WhiteHot",
            Self::BlackHot => "BlackHot",
            Self::RedHot => "RedHot",
            Self::Fusion1 => "Fusion1",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ImageModeOption {
    #[value(name = "standard")]
    Standard,
    #[value(name = "indoor")]
    Indoor,
    #[value(name = "outdoor")]
    Outdoor,
    #[value(name = "dimLight")]
    DimLight,
}

impl ImageModeOption {
    pub(crate) fn as_isapi_value(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Indoor => "indoor",
            Self::Outdoor => "outdoor",
            Self::DimLight => "dimLight",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum PtzZoomLabelOption {
    #[value(name = "2sec")]
    Sec2,
    #[value(name = "5sec")]
    Sec5,
    #[value(name = "10sec")]
    Sec10,
    #[value(name = "alwaysclose")]
    AlwaysClose,
    #[value(name = "alwaysopen")]
    AlwaysOpen,
}

impl PtzZoomLabelOption {
    pub(crate) fn as_isapi_value(self) -> &'static str {
        match self {
            Self::Sec2 => "2sec",
            Self::Sec5 => "5sec",
            Self::Sec10 => "10sec",
            Self::AlwaysClose => "alwaysclose",
            Self::AlwaysOpen => "alwaysopen",
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(name = "hikvision-thermal-wayland-viewer")]
#[command(about = "Minimal Hikvision viewer in Rust (Wayland-only rendering path).")]
pub(crate) struct Args {
    #[arg(long)]
    pub(crate) sdk_root: Option<PathBuf>,
    #[arg(long, default_value = "10.15.12.1")]
    pub(crate) ip: String,
    #[arg(long, default_value_t = 8000)]
    pub(crate) device_port: u16,
    #[arg(long, env = "HIK_USER", default_value = "admin")]
    pub(crate) username: String,
    #[arg(long, env = "HIK_PASSWORD")]
    pub(crate) password: String,
    #[arg(long, default_value_t = 1)]
    pub(crate) channel: i32,
    #[arg(long, value_enum, ignore_case = true)]
    pub(crate) focus_style: Option<FocusStyleOption>,
    #[arg(long, value_enum, ignore_case = true)]
    pub(crate) noise_reduction_mode: Option<NoiseReductionModeOption>,
    #[arg(long, value_enum, ignore_case = true)]
    pub(crate) digital_zoom: Option<DigitalZoomOption>,
    #[arg(long, value_enum, ignore_case = true)]
    pub(crate) palette: Option<PaletteOption>,
    #[arg(long, value_enum, ignore_case = true)]
    pub(crate) image_mode: Option<ImageModeOption>,
    #[arg(long)]
    pub(crate) scene: Option<String>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_date_time_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_date_display_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_channel_name_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_text_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_battery_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_device_status_battery_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_device_status_charging_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_device_status_bluetooth_enabled: Option<bool>,
    #[arg(long, help_heading = "OSD Controls")]
    pub(crate) osd_device_status_dial_enabled: Option<bool>,
    #[arg(long, value_enum, ignore_case = true, help_heading = "OSD Controls")]
    pub(crate) osd_ptz_zoom_label: Option<PtzZoomLabelOption>,
    #[arg(long, default_value_t = false, help_heading = "OSD Snapshot")]
    pub(crate) osd_snapshot: bool,
    #[arg(
        long,
        default_value = "runtime_logs/osd_snapshots",
        help_heading = "OSD Snapshot"
    )]
    pub(crate) osd_snapshot_dir: PathBuf,
    #[arg(long, default_value_t = false, help_heading = "Headless Publish")]
    pub(crate) headless_publish_rtsp: bool,
    #[arg(
        long,
        default_value = DEFAULT_RTSP_ADDRESS,
        help_heading = "Headless Publish"
    )]
    pub(crate) publish_rtsp_address: String,
    #[arg(long, default_value_t = 8554, help_heading = "Headless Publish")]
    pub(crate) publish_rtsp_port: u16,
    #[arg(
        long,
        default_value = DEFAULT_RTSP_PATH,
        help_heading = "Headless Publish"
    )]
    pub(crate) publish_rtsp_path: String,
    #[arg(long, default_value_t = 25, help_heading = "Headless Publish")]
    pub(crate) publish_fps: u32,
    #[arg(long, default_value_t = 2048, help_heading = "Headless Publish")]
    pub(crate) publish_bitrate_kbps: u32,
    #[arg(long, default_value_t = 10000, help_heading = "Headless Publish")]
    pub(crate) publish_wait_for_first_frame_ms: u64,
    #[arg(long, default_value_t = 0)]
    pub(crate) stream_type: u32,
    #[arg(long, default_value_t = 0)]
    pub(crate) link_mode: u32,
    #[arg(long, default_value_t = 0)]
    pub(crate) proto_type: u8,
    #[arg(long, default_value_t = 0)]
    pub(crate) login_mode: u8,
    #[arg(long, default_value_t = 2 * 1024 * 1024)]
    pub(crate) playm4_buffer_bytes: u32,
    #[arg(long, default_value_t = 3000)]
    pub(crate) connect_timeout_ms: u32,
    #[arg(long, default_value_t = 2)]
    pub(crate) connect_retries: u32,
    #[arg(long, default_value_t = 10000)]
    pub(crate) reconnect_interval_ms: u32,
    #[arg(long, default_value = "runtime_logs/sdk_logs_rust")]
    pub(crate) log_dir: PathBuf,
    #[arg(long, default_value_t = false)]
    pub(crate) discover: bool,
    #[arg(long, default_value = "runtime_logs/discovery_reports")]
    pub(crate) discover_report_dir: PathBuf,
}
