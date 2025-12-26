## 2024-05-23 - Emojis in CLI Output

**Learning:** Even when constraints specify 'no emojis', they can slip into less common output paths like 'plan mode'. Always search for unicode characters/emojis when auditing for accessibility/compatibility.
**Action:** Use `grep -P "[^\x00-\x7F]"` to scan codebase for hidden non-ASCII characters.
