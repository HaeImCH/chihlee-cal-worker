use chrono::{DateTime, Utc};

use chihlee_cal_worker::models::{ResolvedBy, SemesterLink};
use chihlee_cal_worker::routes::{
    resolve_current_semester, resolve_selected_semester, roc_year_from_utc, target_semester_from_utc,
};
use chihlee_cal_worker::source_scraper::{extract_semester, extract_semester_links};

fn sample_links() -> Vec<SemesterLink> {
    vec![
        SemesterLink {
            semester: 115,
            url: "https://example.com/115.pdf".to_string(),
            title: "115".to_string(),
        },
        SemesterLink {
            semester: 114,
            url: "https://example.com/114.pdf".to_string(),
            title: "114".to_string(),
        },
        SemesterLink {
            semester: 113,
            url: "https://example.com/113.pdf".to_string(),
            title: "113".to_string(),
        },
    ]
}

#[test]
fn roc_year_conversion_boundaries() {
    let jan: DateTime<Utc> = "2026-01-01T00:00:00Z".parse().expect("valid datetime");
    let aug: DateTime<Utc> = "2026-08-01T00:00:00Z".parse().expect("valid datetime");

    assert_eq!(roc_year_from_utc(jan), 115);
    assert_eq!(roc_year_from_utc(aug), 115);
}

#[test]
fn target_semester_uses_august_cutover_in_taipei() {
    let before_cutover: DateTime<Utc> = "2026-07-31T15:59:59Z".parse().expect("valid datetime");
    let at_cutover: DateTime<Utc> = "2026-07-31T16:00:00Z".parse().expect("valid datetime");

    assert_eq!(target_semester_from_utc(before_cutover), 114);
    assert_eq!(target_semester_from_utc(at_cutover), 115);
}

#[test]
fn extract_semester_from_text_and_percent_escaped_filename() {
    assert_eq!(extract_semester("114學年度"), Some(114));
    assert_eq!(extract_semester("112%40school_calendar.pdf"), Some(112));
}

#[test]
fn extract_links_from_html_with_mixed_semesters() {
    let html = r#"
        <a href="/files/114.pdf">114學年度行事曆</a>
        <a href="/files/113%40abc.pdf">113學年度行事曆</a>
        <a href="/files/not-a-pdf.txt">skip me</a>
    "#;

    let links = extract_semester_links(html, "https://www.chihlee.edu.tw/p/404-1000-62149.php")
        .expect("extract links");

    assert_eq!(links.len(), 2);
    assert_eq!(links[0].semester, 114);
    assert_eq!(links[1].semester, 113);
}

#[test]
fn current_semester_returns_negative_one_when_target_missing() {
    let links = sample_links();
    assert_eq!(resolve_current_semester(112, &links), -1);
}

#[test]
fn cal_link_selection_precedence_and_default_fallback() {
    let links = sample_links();

    let explicit = resolve_selected_semester(Some(113), &links, 114).expect("explicit selection");
    assert_eq!(explicit.semester, 113);
    assert_eq!(explicit.resolved_by, ResolvedBy::Explicit);

    let current = resolve_selected_semester(None, &links, 114).expect("current selection");
    assert_eq!(current.semester, 114);
    assert_eq!(current.resolved_by, ResolvedBy::Current);

    let latest = resolve_selected_semester(None, &links, 112).expect("latest fallback");
    assert_eq!(latest.semester, 115);
    assert_eq!(latest.resolved_by, ResolvedBy::Latest);
}
