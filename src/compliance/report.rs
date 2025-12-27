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

    /// Calculate overall compliance score (percentage)
    pub fn compliance_score(&self) -> f64 {
        self.overall_stats.compliance_percentage()
    }

    /// Get letter grade
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

    fn render_text(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("=== {} ===\n", self.title));
        output.push_str(&format!("Generated: {}\n", self.timestamp));
        if let Some(ref target) = self.target {
            output.push_str(&format!("Target: {}\n", target));
        }
        output.push_str(&format!(
            "\nOverall Score: {:.1}% (Grade: {})\n",
            self.compliance_score(),
            self.grade()
        ));
        output.push_str(&format!(
            "Total: {} | Pass: {} | Fail: {} | Warning: {} | Skipped: {}\n\n",
            self.overall_stats.total_checks,
            self.overall_stats.passed,
            self.overall_stats.failed,
            self.overall_stats.warnings,
            self.overall_stats.skipped
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
        html.push_str(&format!("<h1>{}</h1>", self.title));
        html.push_str(&format!(
            "<p>Score: {:.1}% | Grade: {}</p>",
            self.compliance_score(),
            self.grade()
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
                class, finding.status, finding.check_id, finding.title, finding.severity
            ));
        }

        html.push_str("</table></body></html>");
        html
    }

    fn render_csv(&self) -> String {
        let mut csv = String::new();
        csv.push_str("Status,Check ID,Title,Severity,Framework\n");
        for finding in &self.findings {
            csv.push_str(&format!(
                "{},{},{},{},{}\n",
                finding.status,
                finding.check_id,
                finding.title.replace(',', ";"),
                finding.severity,
                finding.framework
            ));
        }
        csv
    }
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
