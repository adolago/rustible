//! In-memory checks for report serialization and aggregate score semantics.
use rustible::compliance::{
    CheckStatus, ComplianceFramework, ComplianceReport, ComplianceReportBuilder, ComplianceStats,
    Finding, ReportFormat,
};

fn report_with(statuses: &[CheckStatus]) -> ComplianceReport {
    statuses
        .iter()
        .fold(ComplianceReportBuilder::new(), |builder, status| {
            builder.with_finding(
                Finding::new("fixture", "Synthetic check", ComplianceFramework::Cis)
                    .with_status(*status),
            )
        })
        .build()
}

#[test]
fn empty_and_skipped_only_reports_are_not_applicable() {
    for statuses in [vec![], vec![CheckStatus::Skipped; 3]] {
        let report = report_with(&statuses);
        assert_eq!(report.compliance_score(), 0.0);
        assert_eq!(report.grade(), "N/A");
        assert!(report
            .render(ReportFormat::Text)
            .contains("Overall Score: N/A (Grade: N/A)"));
        assert!(report
            .render(ReportFormat::Html)
            .contains("Score: N/A | Grade: N/A"));
    }
}

#[test]
fn errors_and_unknown_results_are_incomplete() {
    for status in [CheckStatus::Error, CheckStatus::Unknown] {
        let all_unresolved = report_with(&[status, CheckStatus::Skipped]);
        assert_eq!(all_unresolved.compliance_score(), 0.0);
        assert_eq!(all_unresolved.grade(), "Incomplete");

        let partial = report_with(&[CheckStatus::Pass, status, CheckStatus::Skipped]);
        assert_eq!(partial.compliance_score(), 50.0);
        assert_eq!(partial.grade(), "Incomplete");
    }
}

#[test]
fn replacing_a_failure_with_an_error_or_unknown_cannot_improve_the_score() {
    let failed = report_with(&[CheckStatus::Pass, CheckStatus::Fail]);
    for status in [CheckStatus::Error, CheckStatus::Unknown] {
        let unresolved = report_with(&[CheckStatus::Pass, status]);
        assert_eq!(unresolved.compliance_score(), failed.compliance_score());
        assert_eq!(unresolved.grade(), "Incomplete");
    }
}

#[test]
fn completed_reports_keep_existing_grade_and_warning_semantics() {
    for (statuses, score, grade) in [
        (vec![CheckStatus::Pass], 100.0, "A+"),
        (vec![CheckStatus::Pass, CheckStatus::Skipped], 100.0, "A+"),
        (vec![CheckStatus::Pass, CheckStatus::Fail], 50.0, "F"),
        (vec![CheckStatus::Pass, CheckStatus::Warning], 50.0, "F"),
    ] {
        let report = report_with(&statuses);
        assert_eq!(report.compliance_score(), score);
        assert_eq!(report.grade(), grade);
    }
}

#[test]
fn inconsistent_public_counters_are_conservatively_incomplete() {
    for stats in [
        ComplianceStats {
            passed: 1,
            ..ComplianceStats::default()
        },
        ComplianceStats {
            total_checks: 1,
            skipped: 2,
            ..ComplianceStats::default()
        },
        ComplianceStats {
            total_checks: u32::MAX,
            passed: u32::MAX,
            errors: u32::MAX,
            ..ComplianceStats::default()
        },
    ] {
        assert_eq!(stats.compliance_percentage(), 0.0);
        assert_eq!(stats.grade(), "Incomplete");
    }
}

#[test]
fn text_and_html_summaries_show_errors_and_derived_unknown_count() {
    let report = report_with(&[CheckStatus::Pass, CheckStatus::Error, CheckStatus::Unknown]);
    for format in [ReportFormat::Text, ReportFormat::Html] {
        let rendered = report.render(format);
        assert!(rendered.contains("Error: 1"));
        assert!(rendered.contains("Unknown: 1"));
        assert!(rendered.contains("Grade: Incomplete"));
    }
}

#[test]
fn html_renders_user_strings_as_literal_text() {
    let literal = "<b>literal</b> & \"quote\" 'apostrophe'";
    let escaped = "&lt;b&gt;literal&lt;/b&gt; &amp; &quot;quote&quot; &#39;apostrophe&#39;";
    let report = ComplianceReportBuilder::new()
        .with_title(literal)
        .with_finding(
            Finding::new(literal, literal, ComplianceFramework::Cis).with_status(CheckStatus::Pass),
        )
        .build();
    let html = report.render(ReportFormat::Html);
    assert!(html.contains(&format!("<h1>{escaped}</h1>")));
    assert!(html.contains(&format!("<td>{escaped}</td><td>{escaped}</td>")));
    assert!(!html.contains("<b>literal</b>"));
    assert!(!html.contains("&amp;lt;b&amp;gt;"));
}

#[test]
fn csv_round_trips_every_user_field_without_losing_punctuation() {
    let fields = [
        "comma,value",
        "a \"quoted\" value",
        "two\nlines",
        "carriage\rreturn",
        "both\r\nlines",
        "",
        "Unicode: 界",
    ];
    let report = fields
        .iter()
        .fold(ComplianceReportBuilder::new(), |builder, value| {
            builder.with_finding(
                Finding::new(*value, *value, ComplianceFramework::Cis)
                    .with_status(CheckStatus::Pass),
            )
        })
        .build();
    let output = report.render(ReportFormat::Csv);
    let mut reader = csv::Reader::from_reader(output.as_bytes());
    assert_eq!(
        reader.headers().unwrap().iter().collect::<Vec<_>>(),
        ["Status", "Check ID", "Title", "Severity", "Framework"]
    );
    let records = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(records.len(), fields.len());
    for (record, expected) in records.iter().zip(fields) {
        assert_eq!(
            record.iter().collect::<Vec<_>>(),
            ["PASS", expected, expected, "MEDIUM", "CIS"]
        );
    }
}

#[test]
fn simple_csv_preserves_header_columns_and_line_endings() {
    let report = report_with(&[CheckStatus::Pass]);
    assert_eq!(
        report.render(ReportFormat::Csv),
        "Status,Check ID,Title,Severity,Framework\nPASS,fixture,Synthetic check,MEDIUM,CIS\n"
    );
}

#[test]
fn serialized_statistics_preserve_the_existing_field_shape() {
    let report = report_with(&[CheckStatus::Pass, CheckStatus::Unknown]);
    let json = report.render(ReportFormat::Json);
    let restored: ComplianceReport = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.compliance_score(), 50.0);
    assert_eq!(restored.grade(), "Incomplete");
    let value = serde_json::to_value(&restored.overall_stats).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 8);
    assert!(value.get("unknown").is_none());
}
