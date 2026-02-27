use std::collections::HashSet;

use crate::model::MergedOutput;

#[derive(Debug, Clone)]
struct CalendarEntry {
    date: String,
    event: String,
}

fn parse_month_day_at(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    let mut month_digits = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() && month_digits < 2 {
        index += 1;
        month_digits += 1;
    }
    if month_digits == 0 || index >= bytes.len() || bytes[index] != b'/' {
        return None;
    }

    let month = std::str::from_utf8(&bytes[start..index])
        .ok()?
        .parse::<u8>()
        .ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }

    index += 1; // '/'
    let day_start = index;
    let mut day_digits = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() && day_digits < 2 {
        index += 1;
        day_digits += 1;
    }
    if day_digits == 0 {
        return None;
    }

    let day = std::str::from_utf8(&bytes[day_start..index])
        .ok()?
        .parse::<u8>()
        .ok()?;
    if !(1..=31).contains(&day) {
        return None;
    }

    Some(index)
}

fn is_range_sep(ch: char) -> bool {
    matches!(ch, '~' | '～' | '-' | '－' | '—')
}

fn normalize_date_token(token: &str) -> String {
    token
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .map(|ch| {
            if matches!(ch, '～' | '-' | '－' | '—') {
                '~'
            } else {
                ch
            }
        })
        .collect()
}

fn find_date_tokens(line: &str) -> Vec<(usize, usize, String)> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }

        let Some(mut end) = parse_month_day_at(bytes, index) else {
            index += 1;
            continue;
        };

        if index > 0 {
            let prev = line[..index].chars().next_back().unwrap_or(' ');
            if prev.is_alphanumeric() || ('\u{4E00}'..='\u{9FFF}').contains(&prev) {
                index += 1;
                continue;
            }
        }

        if let Some(next_ch) = line[end..].chars().next()
            && next_ch == '起'
        {
            end += next_ch.len_utf8();
        }

        let mut cursor = end;
        while let Some(ch) = line[cursor..].chars().next() {
            if ch.is_whitespace() {
                cursor += ch.len_utf8();
            } else {
                break;
            }
        }

        if let Some(sep) = line[cursor..].chars().next()
            && is_range_sep(sep)
        {
            cursor += sep.len_utf8();
            while let Some(ch) = line[cursor..].chars().next() {
                if ch.is_whitespace() {
                    cursor += ch.len_utf8();
                } else {
                    break;
                }
            }

            if let Some(range_end) = parse_month_day_at(bytes, cursor) {
                end = range_end;
                if let Some(next_ch) = line[end..].chars().next()
                    && next_ch == '起'
                {
                    end += next_ch.len_utf8();
                }
            }
        }

        if let Some(next_ch) = line[end..].chars().next()
            && !next_ch.is_whitespace()
            && !matches!(
                next_ch,
                ')' | '）' | '(' | '（' | '，' | ',' | '；' | ';' | '。' | ':'
            )
            && !is_range_sep(next_ch)
        {
            index += 1;
            continue;
        }

        let raw = &line[index..end];
        out.push((index, end, normalize_date_token(raw)));
        index = end;
    }

    out
}

fn is_noise_token(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }

    if trimmed.chars().all(|ch| {
        ch.is_ascii_digit() || ch.is_ascii_whitespace() || matches!(ch, '.' | ',' | '(' | ')')
    }) {
        return true;
    }

    let weekday_chars = "日一二三四五六";
    if trimmed.chars().all(|ch| weekday_chars.contains(ch)) {
        return true;
    }

    false
}

fn looks_calendar_note(line: &str) -> bool {
    line.starts_with("※註")
        || line.starts_with("第")
        || line.contains("月    曆")
        || line.contains("致理科技大學")
}

fn clean_event_text(value: &str) -> String {
    fn is_trailing_noise_token(token: &str) -> bool {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return true;
        }

        if trimmed
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | ',' | ':' | '：'))
        {
            return true;
        }

        let weekday_chars = "日一二三四五六";
        if trimmed.chars().all(|ch| weekday_chars.contains(ch)) {
            return true;
        }

        false
    }

    let mut tokens = value
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    if let Some(cut) = tokens.iter().position(|token| {
        token.contains("週別")
            || token.contains("日期及行事計畫")
            || token.contains("民國")
            || token.contains("致理科技大學")
            || token.contains("※註")
            || token == "月"
            || token == "曆"
            || token.ends_with("月")
            || token == "1."
            || token == "2."
            || token == "3."
    }) {
        tokens.truncate(cut);
    }

    for start in 1..tokens.len() {
        if tokens[start..]
            .iter()
            .all(|token| is_trailing_noise_token(token))
        {
            tokens.truncate(start);
            break;
        }
    }

    while tokens
        .last()
        .is_some_and(|token| is_trailing_noise_token(token))
    {
        tokens.pop();
    }

    let mut out = tokens.join(" ").trim().trim_matches('，').to_string();
    if out.starts_with("上課後") && out.contains(')') && !out.starts_with('(') {
        out.insert(0, '(');
    }
    out
}

fn split_mixed_event(event: &str) -> Vec<String> {
    let marker = " 四技甄選入學實作面試";
    if let Some(pos) = event.find(marker) {
        let first = event[..pos].trim().to_string();
        let second = event[pos + 1..].trim().to_string();
        return [first, second]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect();
    }

    vec![event.to_string()]
}

pub(crate) fn clean_calendar_from_text(text: &str) -> MergedOutput {
    let mut entries = Vec::new();
    let mut current: Option<CalendarEntry> = None;

    let push_current = |entries: &mut Vec<CalendarEntry>, current: &mut Option<CalendarEntry>| {
        if let Some(entry) = current.take() {
            let event = clean_event_text(&entry.event);
            if !event.is_empty() {
                entries.push(CalendarEntry {
                    date: entry.date,
                    event,
                });
            }
        }
    };

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let tokens = find_date_tokens(line);
        if tokens.is_empty() {
            if looks_calendar_note(line) || is_noise_token(line) {
                continue;
            }

            if let Some(entry) = current.as_mut() {
                if !entry.event.is_empty() {
                    entry.event.push(' ');
                }
                entry.event.push_str(line);
            }
            continue;
        }

        if let Some((first_start, _, _)) = tokens.first() {
            let prefix = line[..*first_start].trim();
            if !prefix.is_empty()
                && !is_noise_token(prefix)
                && !looks_calendar_note(prefix)
                && let Some(entry) = current.as_mut()
            {
                if !entry.event.is_empty() {
                    entry.event.push(' ');
                }
                entry.event.push_str(prefix);
            }
        }

        for (index, (_, end, date)) in tokens.iter().enumerate() {
            push_current(&mut entries, &mut current);

            let next_start = tokens
                .get(index + 1)
                .map_or(line.len(), |(start, _, _)| *start);
            let segment = line[*end..next_start].trim();
            current = Some(CalendarEntry {
                date: date.clone(),
                event: segment.to_string(),
            });
        }
    }

    push_current(&mut entries, &mut current);

    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for entry in entries {
        for event in split_mixed_event(&entry.event) {
            let key = format!("{}|{}", entry.date, event);
            if seen.insert(key) {
                rows.push(vec![
                    "1".to_string(),
                    "1".to_string(),
                    entry.date.clone(),
                    event,
                ]);
            }
        }
    }

    MergedOutput {
        headers: vec![
            "page".to_string(),
            "table_id".to_string(),
            "col_1".to_string(),
            "col_2".to_string(),
        ],
        row_count: rows.len(),
        table_count: if rows.is_empty() { 0 } else { 1 },
        rows,
    }
}

pub(crate) fn clean_calendar_output(merged: &MergedOutput) -> MergedOutput {
    let mut rows = Vec::new();
    let mut seen = HashSet::new();

    for row in &merged.rows {
        if row.len() < 4 {
            continue;
        }

        let page = row[0].clone();
        let table_id = row[1].clone();
        let payload = &row[2..];

        for (index, token) in payload.iter().enumerate() {
            if find_date_tokens(token).is_empty() {
                continue;
            }
            let date = normalize_date_token(token.trim());

            let mut event = None;
            for candidate in payload.iter().skip(index + 1) {
                let text = candidate.trim();
                if text.is_empty() || is_noise_token(text) {
                    continue;
                }
                if !find_date_tokens(text).is_empty() {
                    break;
                }
                event = Some(text.to_string());
                break;
            }

            let Some(event) = event else {
                continue;
            };

            let key = format!("{}|{}|{}|{}", page, table_id, date, event);
            if seen.insert(key) {
                rows.push(vec![page.clone(), table_id.clone(), date, event]);
            }
        }
    }

    let table_count = rows
        .iter()
        .map(|row| row[1].as_str())
        .collect::<HashSet<_>>()
        .len();

    MergedOutput {
        headers: vec![
            "page".to_string(),
            "table_id".to_string(),
            "col_1".to_string(),
            "col_2".to_string(),
        ],
        row_count: rows.len(),
        table_count,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use crate::clean_calendar::{
        clean_calendar_from_text, clean_calendar_output, find_date_tokens,
    };
    use crate::model::MergedOutput;

    #[test]
    fn keeps_md_and_md_range_rows_only() {
        let merged = MergedOutput {
            headers: vec![
                "page".into(),
                "table_id".into(),
                "col_1".into(),
                "col_2".into(),
                "col_3".into(),
            ],
            rows: vec![
                vec!["1".into(), "1".into(), "1".into(), "2".into(), "8/1".into()],
                vec![
                    "1".into(),
                    "1".into(),
                    "8/1".into(),
                    "開學".into(),
                    "".into(),
                ],
                vec![
                    "1".into(),
                    "2".into(),
                    "11/17~11/21".into(),
                    "期中考試週".into(),
                    "".into(),
                ],
                vec![
                    "1".into(),
                    "2".into(),
                    "備註".into(),
                    "說明".into(),
                    "".into(),
                ],
            ],
            table_count: 2,
            row_count: 4,
        };

        let cleaned = clean_calendar_output(&merged);
        assert_eq!(cleaned.headers, vec!["page", "table_id", "col_1", "col_2"]);
        assert_eq!(cleaned.row_count, 2);
        assert_eq!(cleaned.rows[0], vec!["1", "1", "8/1", "開學"]);
        assert_eq!(cleaned.rows[1], vec!["1", "2", "11/17~11/21", "期中考試週"]);
    }

    #[test]
    fn parses_date_variants() {
        let tokens = find_date_tokens("2/17-2/22 春節 12/8起 申請");
        assert_eq!(tokens[0].2, "2/17~2/22");
        assert_eq!(tokens[1].2, "12/8起");
    }

    #[test]
    fn merges_continuation_lines() {
        let text = "9/15~9/19 開學週；日間部延\n修生註冊；舊生於9/15前申請\n9/23 敬師餐會";
        let cleaned = clean_calendar_from_text(text);
        assert_eq!(cleaned.row_count, 2);
        assert_eq!(cleaned.rows[0][2], "9/15~9/19");
        assert!(cleaned.rows[0][3].contains("修生註冊"));
    }

    #[test]
    fn keeps_prefix_before_next_date_as_continuation() {
        let text = "10/27~12/7 申請休、退學\n者：退還學雜費 1/31 碩士班學位考試完畢";
        let cleaned = clean_calendar_from_text(text);
        assert!(
            cleaned
                .rows
                .iter()
                .any(|row| { row[2] == "10/27~12/7" && row[3].contains("者：退還學雜費") })
        );
    }

    #[test]
    fn splits_mixed_event_for_619_notice() {
        let text = "6/19 端午節 四技甄選入學實作面試(日期未定)遇端午連假，招策會尚未確定";
        let cleaned = clean_calendar_from_text(text);
        assert_eq!(cleaned.row_count, 2);
        assert!(
            cleaned
                .rows
                .iter()
                .any(|row| row[2] == "6/19" && row[3] == "端午節")
        );
        assert!(cleaned.rows.iter().any(|row| {
            row[2] == "6/19" && row[3].starts_with("四技甄選入學實作面試")
        }));
    }
}
