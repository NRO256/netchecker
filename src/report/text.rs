use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

fn print_lines_if_text(lines: &[String], is_json: bool) {
    if !is_json {
        for line in lines {
            println!("{line}");
        }
    }
}

/// BOM付きUTF-8でレポートファイルを書き込み
fn try_write_report(path: &Path, lines: &[String]) -> Result<(), std::io::Error> {
    let f = File::create(path)?;
    let mut writer = BufWriter::new(f);
    let bom: [u8; 3] = [0xEF, 0xBB, 0xBF];
    writer.write_all(&bom)?;
    for line in lines {
        writeln!(writer, "{line}")?;
    }
    writer.flush()?;
    Ok(())
}

/// レポートのヘッダー部分を構築
pub fn build_header(
    version_str: &str,
    time_display: &str,
    target_name: Option<&str>,
    proxy_info: Option<&str>,
    is_json: bool,
) -> Vec<String> {
    let header_border = "============================================================";
    let mut lines = Vec::with_capacity(7);

    lines.push(header_border.to_string());
    lines.push(format!(
        " NetChecker Portable v{version_str} - ネットワーク接続確認ツール"
    ));
    lines.push(format!(" 実行日時: {time_display}"));
    if let Some(target) = target_name {
        lines.push(format!(" 診断対象: {target}"));
    }
    if let Some(proxy) = proxy_info {
        lines.push(format!(" プロキシ設定: {proxy}"));
    }
    lines.push(header_border.to_string());
    lines.push(String::new());

    print_lines_if_text(&lines, is_json);
    lines
}

/// 総合判定部分を構築
pub fn build_overall(overall_title: &str, overall_advice: &str, is_json: bool) -> Vec<String> {
    let sep_line = "------------------------------------------------------------";
    let lines = vec![
        sep_line.to_string(),
        overall_title.to_string(),
        overall_advice.to_string(),
        sep_line.to_string(),
        String::new(),
    ];

    print_lines_if_text(&lines, is_json);
    lines
}

/// レポート保存結果
pub enum SaveReportResult {
    Saved {
        path: PathBuf,
        loc_desc: &'static str,
        fallback_used: bool,
    },
    Failed,
}

impl SaveReportResult {
    pub fn saved_path(&self) -> Option<&Path> {
        match self {
            Self::Saved { path, .. } => Some(path.as_path()),
            Self::Failed => None,
        }
    }
}

/// 権限に応じた自動退避（フォールバック）保存
pub fn save_report_with_fallback(time_file: &str, report_lines: &[String]) -> SaveReportResult {
    let report_filename = format!("NetChecker_{time_file}.txt");
    let mut attempt = 0usize;
    let mut try_save = |path: PathBuf, loc_desc: &'static str| -> Option<SaveReportResult> {
        let fallback_used = attempt > 0;
        attempt += 1;
        try_write_report(&path, report_lines)
            .ok()
            .map(|()| SaveReportResult::Saved {
                path,
                loc_desc,
                fallback_used,
            })
    };

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        && let Some(res) = try_save(exe_dir.join(&report_filename), "実行ファイル配置フォルダ")
    {
        return res;
    }
    if let Some(res) = try_save(PathBuf::from(&report_filename), "カレントフォルダ") {
        return res;
    }
    if let Some(res) = try_save(std::env::temp_dir().join(&report_filename), "TEMPフォルダ") {
        return res;
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let base = Path::new(&user_profile);
        if let Some(res) = try_save(base.join("Desktop").join(&report_filename), "デスクトップ")
        {
            return res;
        }
        if let Some(res) = try_save(base.join(&report_filename), "ユーザーホームフォルダ")
        {
            return res;
        }
    }

    SaveReportResult::Failed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_header_with_target_and_proxy() {
        let lines = build_header(
            "0.5.0",
            "2026-11-13 19:00:00",
            Some("カスタム対象 (example.com)"),
            Some("proxy.local:8080"),
            true,
        );
        assert_eq!(lines.len(), 7);
        assert!(lines[1].contains("v0.5.0"));
        assert!(lines[2].contains("2026-11-13 19:00:00"));
        assert!(lines[3].contains("カスタム対象 (example.com)"));
        assert!(lines[4].contains("proxy.local:8080"));
        assert_eq!(lines[6], "");
    }

    #[test]
    fn test_build_overall() {
        let lines = build_overall("【総合判定】: 正常", "接続良好です。", true);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[1], "【総合判定】: 正常");
        assert_eq!(lines[2], "接続良好です。");
        assert_eq!(lines[4], "");
    }

    #[test]
    fn test_try_write_report_writes_utf8_bom() {
        let temp_path = std::env::temp_dir().join("netchecker_unit_test_bom.txt");
        let lines = vec!["1行目".to_string(), "2行目".to_string()];
        try_write_report(&temp_path, &lines).expect("レポート書き込みに成功すること");

        let bytes = std::fs::read(&temp_path).expect("書き込んだファイルを読み取れること");
        let _ = std::fs::remove_file(&temp_path);

        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let text = std::str::from_utf8(&bytes[3..]).expect("UTF-8であること");
        assert_eq!(text, "1行目\n2行目\n");
    }
}
