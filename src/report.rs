use crate::discovery::{DiscoveryProbeRecord, DiscoveryReport, OsdSnapshotReport};

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

fn write_record(out: &mut String, record: &DiscoveryProbeRecord) {
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
    write_json_opt_string(out, "sdk_error_name", &record.sdk_error_name, true);
    out.push_str(&json_string("summary"));
    out.push(':');
    out.push_str(&json_string(&record.summary));
    out.push(',');
    out.push_str(&json_string("payload_bytes"));
    out.push(':');
    out.push_str(&record.payload_bytes.to_string());
    out.push(',');
    write_json_opt_string(out, "status_code", &record.status_code, true);
    write_json_opt_string(out, "status_string", &record.status_string, true);
    write_json_opt_string(out, "sub_status_code", &record.sub_status_code, true);
    write_json_string_array(out, "tags", &record.tags, true);
    write_json_string_array(out, "hints", &record.hints, true);
    out.push_str(&json_string("payload_sample"));
    out.push(':');
    out.push_str(&json_string(&record.payload_sample));
    out.push(',');
    write_json_opt_string(out, "error_text", &record.error_text, false);
    out.push('}');
}

pub(crate) fn discovery_report_to_json(report: &DiscoveryReport) -> String {
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
        write_record(&mut out, record);
    }
    out.push(']');
    out.push('}');
    out.push('\n');
    out
}

pub(crate) fn osd_snapshot_report_to_json(report: &OsdSnapshotReport) -> String {
    let mut out = String::new();
    out.push('{');
    out.push_str(&json_string("generated_at_ms"));
    out.push(':');
    out.push_str(&report.generated_at_ms.to_string());
    out.push(',');
    out.push_str(&json_string("stage"));
    out.push(':');
    out.push_str(&json_string(&report.stage));
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

    out.push_str(&json_string("records"));
    out.push(':');
    out.push('[');
    for (idx, record) in report.records.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        write_record(&mut out, record);
    }
    out.push(']');
    out.push('}');
    out.push('\n');
    out
}
