pub mod json;
pub mod text;

pub use json::{JsonContext, generate_json_output};
pub use text::{SaveReportResult, build_header, build_overall, save_report_with_fallback};
