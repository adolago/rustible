//! Compliance Report Generation
//!
//! This module provides report generation capabilities for compliance scan results.

use super::{ComplianceFramework, ComplianceStats, Finding, Severity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Format for compliance reports
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// Plain text format
    Text,
    /// JSON format
    Json,
    /// HTML format
    Html,
    /// CSV format
    Csv,
}

/// A compliance report containing all findings and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Report title
    pub title: String,
    /// Report generation timestamp
    pub timestamp: String,
    /// Target system identifier
    pub target: Option<String>,
    /// All findings from the scan
    pub findings: Vec<Finding>,
    /// Statistics by framework
    pub stats_by_framework: HashMap<String, ComplianceStats>,
    /// Overall statistics
    pub overall_stats: ComplianceStats,
}

impl ComplianceReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self {
            title: "Compliance Report".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            target: None,
            findings: Vec::new(),
            stats_by_framework: HashMap::new(),
            overall_stats: ComplianceStats::default(),
        }
    }

    /// Get all findings
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Get failed findings only
    pub fn failed_findings(&self) -> Vec<&Finding> {
        self.findings.iter().filter(|f| f.is_failure()).collect()
    }

    /// Get findings by severity
    pub fn findings_by_severity(&self, severity: Severity) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .collect()
    }

    /// Get findings by framework
    pub fn findings_by_framework(&self, framework: ComplianceFramework) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|f| f.framework == framework)
            .collect()
    }

    /// Calculate the percentage of passed non-skipped checks. Inspect `grade()`
    /// to distinguish unmeasured or incomplete results from completed scans.
    pub fn compliance_score(&self) -> f64 {
        self.overall_stats.compliance_percentage()
    }

    /// Get a letter grade, `N/A`, or `Incomplete`.
    pub fn grade(&self) -> &'static str {
        self.overall_stats.grade()
    }

    /// Render report to specified format
    pub fn render(&self, format: ReportFormat) -> String {
        match format {
            ReportFormat::Text => self.render_text(),
            ReportFormat::Json => self.render_json(),
            ReportFormat::Html => self.render_html(),
            ReportFormat::Csv => self.render_csv(),
        }
    }

    fn score_label(&self) -> String {
        if self.grade() == "N/A" {
            "N/A".to_string()
        } else {
            format!("{:.1}%", self.compliance_score())
        }
    }

    fn render_text(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("=== {} ===\n", self.title));
        output.push_str(&format!("Generated: {}\n", self.timestamp));
        if let Some(ref target) = self.target {
            output.push_str(&format!("Target: {}\n", target));
        }
        output.push_str(&format!(
            "\nOverall Score: {} (Grade: {})\n",
            self.score_label(),
            self.grade()
        ));
        output.push_str(&format!(
            "Total: {} | Pass: {} | Fail: {} | Warning: {} | Skipped: {} | Error: {} | Unknown: {}\n\n",
            self.overall_stats.total_checks,
            self.overall_stats.passed,
            self.overall_stats.failed,
            self.overall_stats.warnings,
            self.overall_stats.skipped,
            self.overall_stats.errors,
            self.overall_stats.unknown_checks()
        ));

        for finding in &self.findings {
            let status_color = finding.status.color_code();
            let reset = "\x1b[0m";
            output.push_str(&format!(
                "[{}{}{}] {} - {}\n",
                status_color, finding.status, reset, finding.check_id, finding.title
            ));
            if finding.needs_attention() && !finding.remediation.is_empty() {
                output.push_str(&format!("  Remediation: {}\n", finding.remediation));
            }
        }

        output
    }

    fn render_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    fn render_html(&self) -> String {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html><html><head><title>Compliance Report</title>");
        html.push_str("<style>");
        html.push_str("body { font-family: sans-serif; margin: 20px; }");
        html.push_str(".pass { color: green; } .fail { color: red; } .warning { color: orange; }");
        html.push_str("table { border-collapse: collapse; width: 100%; }");
        html.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }");
        html.push_str("th { background-color: #4CAF50; color: white; }");
        html.push_str("</style></head><body>");
        html.push_str(&format!("<h1>{}</h1>", escape_html_text(&self.title)));
        html.push_str(&format!(
            "<p>Score: {} | Grade: {}</p>",
            self.score_label(),
            self.grade()
        ));
        html.push_str(&format!(
            "<p>Total: {} | Pass: {} | Fail: {} | Warning: {} | Skipped: {} | Error: {} | Unknown: {}</p>",
            self.overall_stats.total_checks,
            self.overall_stats.passed,
            self.overall_stats.failed,
            self.overall_stats.warnings,
            self.overall_stats.skipped,
            self.overall_stats.errors,
            self.overall_stats.unknown_checks()
        ));
        html.push_str("<table><tr><th>Status</th><th>ID</th><th>Title</th><th>Severity</th></tr>");

        for finding in &self.findings {
            let class = match finding.status {
                super::CheckStatus::Pass => "pass",
                super::CheckStatus::Fail => "fail",
                _ => "warning",
            };
            html.push_str(&format!(
                "<tr><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                class,
                finding.status,
                escape_html_text(&finding.check_id),
                escape_html_text(&finding.title),
                finding.severity
            ));
        }

        html.push_str("</table></body></html>");
        html
    }

    fn render_csv(&self) -> String {
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer
            .write_record(["Status", "Check ID", "Title", "Severity", "Framework"])
            .expect("writing the fixed CSV header to memory cannot fail");
        for finding in &self.findings {
            writer
                .write_record([
                    finding.status.to_string(),
                    finding.check_id.clone(),
                    finding.title.clone(),
                    finding.severity.to_string(),
                    finding.framework.to_string(),
                ])
                .expect("writing five CSV fields to memory cannot fail");
        }
        let bytes = writer
            .into_inner()
            .expect("flushing a CSV writer to memory cannot fail");
        String::from_utf8(bytes).expect("CSV output from UTF-8 strings remains UTF-8")
    }
}

fn escape_html_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

impl Default for ComplianceReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing compliance reports
pub struct ComplianceReportBuilder {
    report: ComplianceReport,
}

impl ComplianceReportBuilder {
    /// Create a new report builder
    pub fn new() -> Self {
        Self {
            report: ComplianceReport::new(),
        }
    }

    /// Set the report title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.report.title = title.into();
        self
    }

    /// Set the target system
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.report.target = Some(target.into());
        self
    }

    /// Add findings from a specific framework
    pub fn with_framework_findings(
        mut self,
        framework: ComplianceFramework,
        findings: Vec<Finding>,
    ) -> Self {
        let mut framework_stats = ComplianceStats::default();

        for finding in &findings {
            framework_stats.record_finding(finding);
            self.report.overall_stats.record_finding(finding);
        }

        self.report
            .stats_by_framework
            .insert(format!("{}", framework), framework_stats);
        self.report.findings.extend(findings);
        self
    }

    /// Add a single finding
    pub fn with_finding(mut self, finding: Finding) -> Self {
        self.report.overall_stats.record_finding(&finding);
        self.report.findings.push(finding);
        self
    }

    /// Build the final report
    pub fn build(self) -> ComplianceReport {
        self.report
    }
}

impl Default for ComplianceReportBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compliance::CheckStatus;

    #[test]
    fn test_report_builder() {
        let finding = Finding::new("TEST-1", "Test Check", ComplianceFramework::Cis)
            .with_status(CheckStatus::Pass);

        let report = ComplianceReportBuilder::new()
            .with_title("Test Report")
            .with_target("localhost")
            .with_finding(finding)
            .build();

        assert_eq!(report.title, "Test Report");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.overall_stats.passed, 1);
    }

    #[test]
    fn test_report_formats() {
        let report = ComplianceReport::new();

        let text = report.render(ReportFormat::Text);
        assert!(text.contains("Compliance Report"));

        let json = report.render(ReportFormat::Json);
        assert!(json.contains("\"title\""));

        let html = report.render(ReportFormat::Html);
        assert!(html.contains("<html>"));

        let csv = report.render(ReportFormat::Csv);
        assert!(csv.contains("Status,Check ID"));
    }
}
