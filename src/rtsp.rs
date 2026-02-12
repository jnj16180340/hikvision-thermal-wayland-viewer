use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_rtsp_server as gst_rtsp_server;
use gstreamer_rtsp_server::prelude::*;

use crate::callbacks::frame_argb_bytes;
use crate::cli::Args;
use crate::ffi::DecodedFrame;
use crate::util::unix_millis;

pub(crate) const DEFAULT_RTSP_ADDRESS: &str = "0.0.0.0";
pub(crate) const DEFAULT_RTSP_PATH: &str = "/hik";

pub(crate) fn normalize_rtsp_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        DEFAULT_RTSP_PATH.to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

pub(crate) fn rtsp_client_host_hint(bind_address: &str) -> &str {
    match bind_address {
        "0.0.0.0" => "127.0.0.1",
        "::" | "[::]" => "::1",
        other => other,
    }
}

fn wait_for_first_decoded_frame(
    latest_frame: &Arc<Mutex<Option<DecodedFrame>>>,
    stop_flag: &Arc<AtomicBool>,
    timeout: Duration,
) -> Result<DecodedFrame> {
    let start = Instant::now();
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            bail!("stopped before first decoded frame arrived");
        }
        if let Ok(guard) = latest_frame.lock()
            && let Some(frame) = guard.as_ref()
        {
            return Ok(frame.clone());
        }
        if start.elapsed() >= timeout {
            bail!(
                "timed out after {} ms waiting for first decoded frame",
                timeout.as_millis()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn run_headless_rtsp_publisher(
    args: &Args,
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let timeout = Duration::from_millis(args.publish_wait_for_first_frame_ms.max(100));
    let first_frame = wait_for_first_decoded_frame(&latest_frame, &stop_flag, timeout)?;
    let width = first_frame.width.max(1);
    let height = first_frame.height.max(1);
    let fps = args.publish_fps.max(1);
    let bitrate_kbps = args.publish_bitrate_kbps.max(128);
    let rtsp_path = normalize_rtsp_path(&args.publish_rtsp_path);

    gst::init().context("failed to initialize GStreamer runtime")?;

    let appsrc_slot: Arc<Mutex<Option<gst_app::AppSrc>>> = Arc::new(Mutex::new(None));
    let server = gst_rtsp_server::RTSPServer::new();
    server.set_address(&args.publish_rtsp_address);
    server.set_service(&args.publish_rtsp_port.to_string());

    let mounts = server
        .mount_points()
        .ok_or_else(|| anyhow!("failed to acquire RTSP mount points"))?;

    let launch = format!(
        "( appsrc name=hiksrc is-live=true block=false format=time do-timestamp=true \
         caps=video/x-raw,format=BGRA,width={width},height={height},framerate={fps}/1 ! \
         queue leaky=downstream max-size-buffers=2 ! \
         videoconvert ! \
         x264enc tune=zerolatency speed-preset=ultrafast bitrate={bitrate_kbps} key-int-max={fps} ! \
         rtph264pay name=pay0 pt=96 config-interval=1 )"
    );

    let factory = gst_rtsp_server::RTSPMediaFactory::new();
    // Per-client pipelines are more resilient to disconnect/reconnect churn.
    factory.set_shared(false);
    factory.set_stop_on_disconnect(true);
    factory.set_suspend_mode(gst_rtsp_server::RTSPSuspendMode::Reset);
    factory.set_eos_shutdown(true);
    factory.set_launch(&launch);
    let appsrc_slot_for_cb = Arc::clone(&appsrc_slot);
    factory.connect_media_configure(move |_, media| {
        media.set_stop_on_disconnect(true);
        media.set_suspend_mode(gst_rtsp_server::RTSPSuspendMode::Reset);
        media.set_eos_shutdown(true);

        let element = media.element();
        if let Ok(bin) = element.dynamic_cast::<gst::Bin>()
            && let Some(src_element) = bin.by_name("hiksrc")
            && let Ok(appsrc) = src_element.dynamic_cast::<gst_app::AppSrc>()
        {
            appsrc.set_is_live(true);
            appsrc.set_format(gst::Format::Time);
            appsrc.set_block(false);
            appsrc.set_do_timestamp(true);
            appsrc.set_max_bytes((width as u64 * height as u64 * 4 * 2).max(512 * 1024));
            if let Ok(mut guard) = appsrc_slot_for_cb.lock() {
                *guard = Some(appsrc);
            }
        }

        let appsrc_slot_for_unprepared = Arc::clone(&appsrc_slot_for_cb);
        media.connect_unprepared(move |_| {
            if let Ok(mut guard) = appsrc_slot_for_unprepared.lock() {
                *guard = None;
            }
        });
    });
    mounts.add_factory(&rtsp_path, factory);

    let _server_source = server
        .attach(None)
        .map_err(|err| anyhow!("failed to attach RTSP server to default GLib context: {err}"))?;

    let main_loop = gst::glib::MainLoop::new(None, false);
    let main_loop_for_thread = main_loop.clone();
    let loop_thread = std::thread::Builder::new()
        .name("gst-rtsp-mainloop".to_string())
        .spawn(move || main_loop_for_thread.run())
        .context("failed to spawn RTSP GLib main loop thread")?;

    let stop_for_signal = Arc::clone(&stop_flag);
    if let Err(err) = ctrlc::set_handler(move || {
        stop_for_signal.store(true, Ordering::Relaxed);
    }) {
        eprintln!("warning: failed to install Ctrl-C handler: {err}");
    }

    println!(
        "Headless RTSP publisher listening on rtsp://{}:{}{} ({}x{}, {} fps).",
        args.publish_rtsp_address, args.publish_rtsp_port, rtsp_path, width, height, fps
    );
    println!(
        "Client URL hint: rtsp://{}:{}{}",
        rtsp_client_host_hint(&args.publish_rtsp_address),
        args.publish_rtsp_port,
        rtsp_path
    );
    println!("Waiting for clients. Press Ctrl-C to stop.");

    let mut last_seq = first_frame.sequence.saturating_sub(1);
    let mut last_shape_warn_ms = 0_u64;
    let mut last_push_err_warn_ms = 0_u64;
    while !stop_flag.load(Ordering::Relaxed) {
        let frame = {
            if let Ok(guard) = latest_frame.lock() {
                guard.as_ref().filter(|f| f.sequence != last_seq).cloned()
            } else {
                None
            }
        };
        let Some(frame) = frame else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        last_seq = frame.sequence;

        if frame.width != width || frame.height != height {
            let now_ms = unix_millis();
            if now_ms.saturating_sub(last_shape_warn_ms) > 1000 {
                last_shape_warn_ms = now_ms;
                eprintln!(
                    "frame shape changed from {}x{} to {}x{}; dropping frame",
                    width, height, frame.width, frame.height
                );
            }
            continue;
        }

        let appsrc = {
            if let Ok(guard) = appsrc_slot.lock() {
                guard.as_ref().cloned()
            } else {
                None
            }
        };
        let Some(appsrc) = appsrc else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };

        let payload = frame_argb_bytes(&frame);
        let mut buffer = match gst::Buffer::with_size(payload.len()) {
            Ok(buf) => buf,
            Err(err) => {
                eprintln!("failed to allocate GStreamer buffer: {err}");
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
        };

        let Some(buffer_mut) = buffer.get_mut() else {
            eprintln!("failed to get mutable GStreamer buffer");
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        match buffer_mut.map_writable() {
            Ok(mut map) => map.as_mut_slice().copy_from_slice(payload),
            Err(err) => {
                eprintln!("failed to map GStreamer buffer writable: {err}");
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
        }
        if let Err(err) = appsrc.push_buffer(buffer) {
            // These are expected around client disconnect/reconnect windows.
            if matches!(
                err,
                gst::FlowError::Flushing | gst::FlowError::Eos | gst::FlowError::NotLinked
            ) {
                if let Ok(mut guard) = appsrc_slot.lock() {
                    *guard = None;
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            let now_ms = unix_millis();
            if now_ms.saturating_sub(last_push_err_warn_ms) > 1000 {
                last_push_err_warn_ms = now_ms;
                eprintln!("appsrc push_buffer failed: {err:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    if let Ok(mut guard) = appsrc_slot.lock()
        && let Some(appsrc) = guard.take()
        && let Err(err) = appsrc.end_of_stream()
    {
        eprintln!("appsrc end_of_stream failed: {err:?}");
    }
    main_loop.quit();
    let _ = loop_thread.join();
    Ok(())
}
