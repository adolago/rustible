# Bolt's Journal

## 2024-05-22 - [Lazy Initialization of Template Engines]
**Learning:** Instantiating `TemplateEngine` (wrapping `minijinja::Environment`) is expensive.
**Action:** Use `once_cell` or `lazy_static` to create instances once and reuse them, especially in modules that are called frequently.
