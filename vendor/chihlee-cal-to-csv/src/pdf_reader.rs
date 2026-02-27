use std::collections::BTreeMap;
use std::path::Path;

use encoding_rs::{BIG5, UTF_16BE};
use lopdf::Document;
use lopdf::Object;
use lopdf::content::Content;

use crate::error::ExtractError;
use crate::model::PageText;
use crate::options::PageSelection;
use crate::table_parse::{soft_split_line_into_cells, split_line_into_cells};

fn split_text_into_pages(raw_text: &str) -> Vec<String> {
    let mut pages = raw_text
        .split('\u{000C}')
        .map(str::to_string)
        .collect::<Vec<_>>();
    if pages.last().is_some_and(String::is_empty) {
        pages.pop();
    }
    pages
}

fn looks_decoding_broken(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    if text.contains("?Identity-H Unimplemented?") {
        return true;
    }

    let total = text.chars().count();
    if total == 0 {
        return false;
    }

    let replacement = text.matches('\u{FFFD}').count();
    let control = text
        .chars()
        .filter(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
        .count();
    let cjk_count = text
        .chars()
        .filter(|ch| {
            ('\u{4E00}'..='\u{9FFF}').contains(ch) || ('\u{3400}'..='\u{4DBF}').contains(ch)
        })
        .count();
    let ext_a_count = text
        .chars()
        .filter(|ch| ('\u{3400}'..='\u{4DBF}').contains(ch))
        .count();

    replacement * 8 > total
        || control * 5 > total
        || (cjk_count > 20 && ext_a_count * 4 > cjk_count)
}

fn decode_pdf_bytes(encoding: Option<&str>, bytes: &[u8]) -> String {
    let decoded = Document::decode_text(encoding, bytes);
    if !looks_decoding_broken(&decoded) {
        return decoded;
    }

    if bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE]) {
        let bytes = if bytes.len() > 2 { &bytes[2..] } else { bytes };
        let (utf16, had_errors) = UTF_16BE.decode_without_bom_handling(bytes);
        if !had_errors && !utf16.is_empty() {
            return utf16.into_owned();
        }
    }

    if let Some(name) = encoding {
        let lower = name.to_ascii_lowercase();

        if lower.contains("utf16")
            || lower.contains("ucs2")
            || lower.contains("identity-h")
            || lower.contains("unicode")
        {
            let (utf16, had_errors) = UTF_16BE.decode_without_bom_handling(bytes);
            if !had_errors && !utf16.is_empty() {
                return utf16.into_owned();
            }
        }

        if lower.contains("big5")
            || lower.contains("b5")
            || lower.contains("eten")
            || lower.contains("cns")
        {
            let (big5, _, had_errors) = BIG5.decode(bytes);
            if !had_errors && !big5.is_empty() {
                return big5.into_owned();
            }
        }
    }

    String::from_utf8_lossy(bytes).to_string()
}

fn extraction_quality_score(text: &str) -> i64 {
    if text.trim().is_empty() {
        return i64::MIN / 4;
    }

    let mut non_empty_lines = 0_i64;
    let mut multi_cell_lines = 0_i64;
    let mut date_like_lines = 0_i64;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        non_empty_lines += 1;

        if split_line_into_cells(line).len() >= 2 || soft_split_line_into_cells(line).len() >= 3 {
            multi_cell_lines += 1;
        }

        let has_digit = line.chars().any(|ch| ch.is_ascii_digit());
        if has_digit && line.contains('/') {
            date_like_lines += 1;
        }
    }

    let broken_penalty = if looks_decoding_broken(text) { 800 } else { 0 };
    multi_cell_lines * 50 + date_like_lines * 15 + non_empty_lines - broken_penalty
}

fn choose_best_text(candidates: &[String]) -> String {
    candidates
        .iter()
        .max_by_key(|text| extraction_quality_score(text))
        .cloned()
        .unwrap_or_default()
}

fn extract_text_from_page_content(document: &Document, page_id: lopdf::ObjectId) -> Option<String> {
    fn collect_text(text: &mut String, encoding: Option<&str>, operands: &[Object]) {
        for operand in operands {
            match operand {
                Object::String(bytes, _) => {
                    text.push_str(&decode_pdf_bytes(encoding, bytes));
                }
                Object::Array(items) => {
                    collect_text(text, encoding, items);
                    text.push(' ');
                }
                Object::Integer(value) => {
                    if *value < -100 {
                        text.push(' ');
                    }
                }
                _ => {}
            }
        }
    }

    let raw_content = document.get_page_content(page_id).ok()?;
    let content = Content::decode(&raw_content).ok()?;
    let encodings = document
        .get_page_fonts(page_id)
        .into_iter()
        .map(|(name, font)| (name, font.get_font_encoding()))
        .collect::<BTreeMap<Vec<u8>, &str>>();

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_encoding = None;
    for operation in content.operations {
        match operation.operator.as_str() {
            "Tf" => {
                if let Some(font_name) = operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                {
                    current_encoding = encodings.get(font_name).copied();
                }
            }
            "Tj" | "TJ" | "'" | "\"" => {
                collect_text(&mut current, current_encoding, &operation.operands);
            }
            "T*" | "Td" | "TD" | "ET" => {
                if !current.trim().is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
            }
            _ => {}
        }
    }

    if !current.trim().is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

pub(crate) fn read_pdf_pages(
    input_pdf: &Path,
    page_selection: Option<&PageSelection>,
) -> Result<Vec<PageText>, ExtractError> {
    let document = Document::load(input_pdf)?;
    let pages_map = document.get_pages();

    let (pdf_extract_pages, pdf_extract_whole) = match pdf_extract::extract_text(input_pdf) {
        Ok(text) => {
            let pages = split_text_into_pages(&text);
            if pages.len() == pages_map.len() {
                (Some(pages), None)
            } else {
                (None, Some(text))
            }
        }
        Err(_) => (None, None),
    };

    let mut pages = Vec::new();
    for (index, (page_no, page_id)) in pages_map.iter().enumerate() {
        if let Some(selection) = page_selection {
            if !selection.contains(*page_no) {
                continue;
            }
        }

        let mut candidates = Vec::new();
        if let Some(text) = pdf_extract_pages
            .as_ref()
            .and_then(|fallback| fallback.get(index).cloned())
            .filter(|text| !text.trim().is_empty())
        {
            candidates.push(text);
        }
        if let Some(text) = extract_text_from_page_content(&document, *page_id) {
            candidates.push(text);
        }
        if let Some(text) = document
            .extract_text(&[*page_no])
            .ok()
            .filter(|text| !text.trim().is_empty())
        {
            candidates.push(text);
        }

        let local_best_score = candidates
            .iter()
            .map(|text| extraction_quality_score(text))
            .max()
            .unwrap_or(i64::MIN / 4);
        if index == 0
            && local_best_score < 80
            && let Some(text) = pdf_extract_whole
                .as_ref()
                .filter(|text| !text.trim().is_empty())
                .cloned()
        {
            candidates.push(text);
        }

        let text = choose_best_text(&candidates);

        pages.push(PageText {
            page_number: *page_no,
            text,
        });
    }

    if pages.is_empty() {
        return Err(ExtractError::NoPagesSelected);
    }

    Ok(pages)
}

pub(crate) fn read_pdf_pages_from_bytes(
    input_pdf: &[u8],
    page_selection: Option<&PageSelection>,
) -> Result<Vec<PageText>, ExtractError> {
    let document = Document::load_mem(input_pdf)?;
    let pages_map = document.get_pages();

    let (pdf_extract_pages, pdf_extract_whole) = match pdf_extract::extract_text_from_mem(input_pdf)
    {
        Ok(text) => {
            let pages = split_text_into_pages(&text);
            if pages.len() == pages_map.len() {
                (Some(pages), None)
            } else {
                (None, Some(text))
            }
        }
        Err(_) => (None, None),
    };

    let mut pages = Vec::new();
    for (index, (page_no, page_id)) in pages_map.iter().enumerate() {
        if let Some(selection) = page_selection {
            if !selection.contains(*page_no) {
                continue;
            }
        }

        let mut candidates = Vec::new();
        if let Some(text) = pdf_extract_pages
            .as_ref()
            .and_then(|fallback| fallback.get(index).cloned())
            .filter(|text| !text.trim().is_empty())
        {
            candidates.push(text);
        }
        if let Some(text) = extract_text_from_page_content(&document, *page_id) {
            candidates.push(text);
        }
        if let Some(text) = document
            .extract_text(&[*page_no])
            .ok()
            .filter(|text| !text.trim().is_empty())
        {
            candidates.push(text);
        }

        let local_best_score = candidates
            .iter()
            .map(|text| extraction_quality_score(text))
            .max()
            .unwrap_or(i64::MIN / 4);
        if index == 0
            && local_best_score < 80
            && let Some(text) = pdf_extract_whole
                .as_ref()
                .filter(|text| !text.trim().is_empty())
                .cloned()
        {
            candidates.push(text);
        }

        let text = choose_best_text(&candidates);

        pages.push(PageText {
            page_number: *page_no,
            text,
        });
    }

    if pages.is_empty() {
        return Err(ExtractError::NoPagesSelected);
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use crate::pdf_reader::{decode_pdf_bytes, split_text_into_pages};

    #[test]
    fn splits_form_feed_delimited_pages() {
        let pages = split_text_into_pages("p1\u{000C}p2\u{000C}");
        assert_eq!(pages, vec!["p1", "p2"]);
    }

    #[test]
    fn decodes_big5_when_encoding_hint_is_present() {
        let (bytes, _, had_errors) = encoding_rs::BIG5.encode("測試");
        assert!(!had_errors);
        let decoded = decode_pdf_bytes(Some("ETen-B5-H"), &bytes);
        assert_eq!(decoded, "測試");
    }
}
