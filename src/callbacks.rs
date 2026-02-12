use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::ffi::{
    Byte, DecodedFrame, Dword, Long, PlayCtrlApi, T_YV12,
    NET_DVR_STREAMDATA, NET_DVR_SYSHEAD, PLAYM4_BUF_OVER, STREAME_REALTIME,
};
use crate::util::{format_playm4_error, unix_millis};

pub(crate) struct CallbackContext {
    pub(crate) playctrl: Arc<PlayCtrlApi>,
    pub(crate) latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    pub(crate) frame_sequence: AtomicU64,
    pub(crate) play_port: AtomicI32,
    pub(crate) playm4_buffer_bytes: u32,
    pub(crate) last_warn_ms: AtomicU64,
}

pub(crate) unsafe extern "C" fn real_data_callback(
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
    frame_info: *mut crate::ffi::FrameInfo,
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

pub(crate) fn frame_argb_bytes(frame: &DecodedFrame) -> &[u8] {
    let len_bytes = frame.pixels_argb.len() * std::mem::size_of::<u32>();
    // The u32 frame buffer is contiguous and lives for the returned slice lifetime.
    unsafe { std::slice::from_raw_parts(frame.pixels_argb.as_ptr().cast::<u8>(), len_bytes) }
}
