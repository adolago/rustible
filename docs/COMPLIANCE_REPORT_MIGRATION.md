# Compliance report correctness changes

This alpha change corrects report scoring and serialization. Public report
method signatures and existing serialized statistics fields are preserved.
Consumers that assume every grade is a letter or split CSV records on commas
must update their handling.

## Scores and incomplete results

Previously, empty and skipped-only reports returned 100 percent and A+.
Errors were excluded from the score denominator, so changing a failed check
to an execution error could improve a score. An all-error report also returned
100 percent and A+.

The score is now passed checks divided by all non-skipped checks. Failed,
warning, error and unknown results remain in that denominator. Replacing a
failure with an error or unknown result therefore cannot improve the score.
Warnings retain their previous treatment as non-passing applicable checks.

Empty and skipped-only results have grade `N/A`. The numeric score method
returns 0 for this unmeasured case; text and HTML show `N/A` rather than a
percentage. Callers must inspect the grade to distinguish unmeasured coverage
from a completed result with no passing checks.

Any error or unknown result gives grade `Incomplete`, even if some checks
passed. For example, one pass and one error gives 50 percent and `Incomplete`;
one pass and one failure gives 50 percent and `F`. Fully measured results keep
the existing letter-grade thresholds. Text and HTML include error and unknown
counts.

`ComplianceStats::unknown_checks()` derives the unknown count from the total
minus the existing status counters. No public field is added, so existing
Rust struct literals and serialized field layouts continue to work. The
public counters can also be populated directly or deserialized; impossible
aggregates whose classified counts exceed the total return 0 and `Incomplete`
instead of overflowing or claiming a passing grade.

## HTML and CSV text

HTML report titles, check IDs and finding titles now render as literal text.
For example, `<b>literal</b>` is shown as text instead of being interpreted as
HTML. Existing table columns and styling remain unchanged.

CSV uses the `csv` crate's writer. Header names, column order and newline
record terminators remain unchanged. Commas are preserved instead of replaced
with semicolons; quotes, carriage returns and newlines are encoded inside
properly quoted fields. Use a CSV parser instead of splitting on commas or
physical lines. JSON's existing field layout is unchanged.

CSV remains a lossless data export. It does not neutralize spreadsheet formula
prefixes; consumers opening untrusted fields in a spreadsheet must import them
as text. This change does not execute or interpret exported cell content.

## Focused verification

`cargo test --locked --test compliance_report_correctness` exercises real report
and statistics APIs using only in-memory synthetic strings and statuses. It
does not instantiate scanners, connect to a host or run commands. The tests
cover empty/skipped/incomplete results, preserved complete-result behavior,
inconsistent counters, JSON field shape, literal HTML text and CSV round trips.
They do not certify the correctness or coverage of compliance scanning.
