use anyhow::{Context, Result, anyhow, bail};

use crate::util::truncate_for_report;

pub(crate) fn extract_xml_tags(xml: &str, max_tags: usize) -> Vec<String> {
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

pub(crate) fn summarize_xml_payload(xml: &str) -> String {
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

pub(crate) fn extract_hints(xml: &str) -> Vec<String> {
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

pub(crate) fn extract_xml_value(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)?;
    let value_start = start + open.len();
    let end = text[value_start..].find(&close)?;
    Some(text[value_start..value_start + end].trim().to_string())
}

pub(crate) fn extract_xml_value_in_section(
    text: &str,
    section_tag: &str,
    target_tag: &str,
) -> Option<String> {
    let (section_value_start, section_value_end, _) = find_xml_tag_bounds(text, section_tag, 0)?;
    let section = &text[section_value_start..section_value_end];
    let (value_start, value_end, _) = find_xml_tag_bounds(section, target_tag, 0)?;
    Some(section[value_start..value_end].trim().to_string())
}

pub(crate) fn extract_xml_values_in_sections(
    text: &str,
    section_tag: &str,
    target_tag: &str,
) -> Vec<String> {
    let mut values = Vec::new();
    let mut search_start = 0;
    while let Some((section_value_start, section_value_end, section_close_end)) =
        find_xml_tag_bounds(text, section_tag, search_start)
    {
        let section = &text[section_value_start..section_value_end];
        if let Some((value_start, value_end, _)) = find_xml_tag_bounds(section, target_tag, 0) {
            values.push(section[value_start..value_end].trim().to_string());
        }
        search_start = section_close_end;
    }
    values
}

pub(crate) fn ensure_isapi_response_ok(method: &str, path: &str, payload: &str) -> Result<()> {
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

pub(crate) fn find_xml_tag_bounds(
    text: &str,
    tag: &str,
    start_idx: usize,
) -> Option<(usize, usize, usize)> {
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

pub(crate) fn find_xml_section_bounds(
    text: &str,
    section_tag: &str,
    start_idx: usize,
) -> Option<(usize, usize)> {
    let open_prefix = format!("<{section_tag}");
    let close = format!("</{section_tag}>");
    let mut search_start = start_idx.min(text.len());

    while let Some(rel_idx) = text[search_start..].find(&open_prefix) {
        let section_start = search_start + rel_idx;
        let after_tag = section_start + open_prefix.len();
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
        let section_content_start = after_tag + open_end_rel + 1;
        let section_close_rel = text[section_content_start..].find(&close)?;
        let section_end = section_content_start + section_close_rel + close.len();
        return Some((section_start, section_end));
    }

    None
}

pub(crate) fn replace_xml_tag_value(xml: &str, tag: &str, value: &str) -> Result<String> {
    let (value_start, value_end, _) =
        find_xml_tag_bounds(xml, tag, 0).ok_or_else(|| anyhow!("tag <{tag}> not found"))?;
    let mut out = String::with_capacity(xml.len() + value.len());
    out.push_str(&xml[..value_start]);
    out.push_str(value);
    out.push_str(&xml[value_end..]);
    Ok(out)
}

pub(crate) fn replace_xml_tag_value_in_section(
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

pub(crate) fn replace_xml_tag_value_in_all_sections(
    xml: &str,
    section_tag: &str,
    target_tag: &str,
    value: &str,
) -> Result<(String, usize)> {
    let mut out = String::with_capacity(xml.len() + value.len());
    let mut cursor = 0;
    let mut count = 0;

    while let Some((section_start, section_end)) = find_xml_section_bounds(xml, section_tag, cursor)
    {
        out.push_str(&xml[cursor..section_start]);
        let section_xml = &xml[section_start..section_end];
        let replaced_section =
            replace_xml_tag_value(section_xml, target_tag, value).with_context(|| {
                format!("tag <{target_tag}> not found inside <{section_tag}> section")
            })?;
        out.push_str(&replaced_section);
        cursor = section_end;
        count += 1;
    }

    out.push_str(&xml[cursor..]);
    if count == 0 {
        bail!("section <{section_tag}> not found");
    }
    Ok((out, count))
}
