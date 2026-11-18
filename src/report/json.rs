use std::fmt::Write;
use std::path::Path;

use crate::diagnostics::{AdapterInfo, StepResult};

/// JSON出力用コンテキスト構造体
pub struct JsonContext<'a> {
    pub version: &'a str,
    pub time_display: &'a str,
    pub target_name: &'a str,
    pub proxy_info: Option<&'a str>,
    pub is_healthy: bool,
    pub step_results: &'a [StepResult],
    pub overall_title: &'a str,
    pub overall_advice: &'a str,
    pub report_path: Option<&'a Path>,
    pub adapter_info: Option<&'a AdapterInfo>,
}

/// JSONエスケープ文字列を既存バッファへ直接書き込む（中間アロケーションゼロ）
fn write_json_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 32 => {
                let code = c as u32;
                let _ = write!(out, "\\u{code:04x}");
            }
            c => out.push(c),
        }
    }
}

fn write_json_string_field(
    out: &mut String,
    indent: &str,
    key: &str,
    val: &str,
    trailing_comma: bool,
) {
    let _ = write!(out, "{indent}\"{key}\": \"");
    write_json_escaped(out, val);
    out.push('"');
    if trailing_comma {
        out.push(',');
    }
    out.push('\n');
}

fn write_json_opt_string_field(
    out: &mut String,
    indent: &str,
    key: &str,
    val: Option<&str>,
    trailing_comma: bool,
) {
    if let Some(v) = val {
        write_json_string_field(out, indent, key, v, trailing_comma);
    } else {
        let comma = if trailing_comma { "," } else { "" };
        let _ = writeln!(out, "{indent}\"{key}\": null{comma}");
    }
}

/// JSON文字列を生成
pub fn generate_json_output(ctx: &JsonContext) -> String {
    let mut json = String::with_capacity(2048);

    json.push_str("{\n");
    write_json_string_field(&mut json, "  ", "version", ctx.version, true);
    write_json_string_field(&mut json, "  ", "timestamp", ctx.time_display, true);
    write_json_string_field(&mut json, "  ", "target", ctx.target_name, true);
    write_json_opt_string_field(&mut json, "  ", "proxy", ctx.proxy_info, true);
    let _ = writeln!(json, "  \"success\": {},", ctx.is_healthy);

    // overall_judgment
    json.push_str("  \"overall_judgment\": {\n");
    write_json_string_field(&mut json, "    ", "title", ctx.overall_title, true);
    write_json_string_field(&mut json, "    ", "advice", ctx.overall_advice, false);
    json.push_str("  },\n");

    // adapter
    json.push_str("  \"adapter\": ");
    if let Some(ad) = ctx.adapter_info {
        json.push_str("{\n");
        write_json_string_field(&mut json, "    ", "name", &ad.friendly_name, true);
        let _ = writeln!(json, "    \"ip_address\": \"{}\",", ad.ip_addr);
        if let Some(gw) = ad.gateway_addr {
            let _ = writeln!(json, "    \"gateway\": \"{gw}\",");
        } else {
            json.push_str("    \"gateway\": null,\n");
        }
        json.push_str("    \"dns_servers\": [");
        for (i, dns) in ad.dns_servers.iter().enumerate() {
            if i > 0 {
                json.push_str(", ");
            }
            let _ = write!(json, "\"{dns}\"");
        }
        json.push_str("],\n");
        if let Some(mbps) = ad.link_speed_mbps {
            let _ = writeln!(json, "    \"link_speed_mbps\": {mbps}");
        } else {
            json.push_str("    \"link_speed_mbps\": null\n");
        }
        json.push_str("  },\n");
    } else {
        json.push_str("null,\n");
    }

    // steps
    json.push_str("  \"steps\": [\n");
    for (i, step) in ctx.step_results.iter().enumerate() {
        let step_num = i + 1;
        json.push_str("    {\n");
        let _ = writeln!(json, "      \"step\": {step_num},");
        write_json_string_field(&mut json, "      ", "name", step.name, true);
        let _ = writeln!(json, "      \"success\": {},", step.success);
        write_json_string_field(&mut json, "      ", "detail", &step.detail, true);
        let _ = writeln!(json, "      \"elapsed_ms\": {}", step.elapsed_ms);
        if i + 1 < ctx.step_results.len() {
            json.push_str("    },\n");
        } else {
            json.push_str("    }\n");
        }
    }
    json.push_str("  ],\n");

    // report_path
    let report_path_str = ctx.report_path.map(|p| p.to_string_lossy());
    write_json_opt_string_field(
        &mut json,
        "  ",
        "report_path",
        report_path_str.as_deref(),
        false,
    );

    json.push_str("}\n");
    json
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn escape_str(s: &str) -> String {
        let mut out = String::new();
        write_json_escaped(&mut out, s);
        out
    }

    #[test]
    fn test_json_escape_plain() {
        assert_eq!(escape_str("hello_world-123"), "hello_world-123");
        assert_eq!(escape_str(""), "");
    }

    #[test]
    fn test_json_escape_special_chars() {
        let input = "quote: \" backslash: \\ newline: \n cr: \r tab: \t bs: \u{08} ff: \u{0C}";
        let expected = "quote: \\\" backslash: \\\\ newline: \\n cr: \\r tab: \\t bs: \\b ff: \\f";
        assert_eq!(escape_str(input), expected);
    }

    #[test]
    fn test_json_escape_control_chars() {
        let input = "\u{0001}\u{001F}";
        let expected = "\\u0001\\u001f";
        assert_eq!(escape_str(input), expected);
    }

    #[test]
    fn test_json_escape_japanese_and_multibyte() {
        let input = "診断結果: 【正常】 - 接続OK 🎉";
        assert_eq!(escape_str(input), "診断結果: 【正常】 - 接続OK 🎉");
    }

    #[test]
    fn test_generate_json_output_icmp_restricted_healthy() {
        let steps = vec![
            StepResult {
                name: "インターネット出口",
                success: false,
                detail: "タイムアウト".to_string(),
                elapsed_ms: 2500,
            },
            StepResult {
                name: "Webアクセス",
                success: true,
                detail: "HTTP/HTTPS通信 正常 (ステータス: 200)".to_string(),
                elapsed_ms: 120,
            },
        ];
        let ctx = JsonContext {
            version: "0.5.0",
            time_display: "2026-10-31 16:00:00",
            target_name: "既定 (Google / Cloudflare)",
            proxy_info: None,
            is_healthy: true,
            step_results: &steps,
            overall_title: "【総合判定】: 正常 (ICMP制限環境)",
            overall_advice: "Web通信および名前解決は正常です。",
            report_path: None,
            adapter_info: None,
        };
        let out = generate_json_output(&ctx);
        assert!(out.contains("\"success\": true,"));
        assert!(out.contains("\"title\": \"【総合判定】: 正常 (ICMP制限環境)\""));
        assert!(out.contains("\"proxy\": null,"));
        assert!(out.contains("\"adapter\": null,"));
        assert!(out.contains("\"report_path\": null\n"));
    }

    #[test]
    fn test_generate_json_output_with_adapter_proxy_and_path() {
        let adapter = AdapterInfo {
            friendly_name: "Wi-Fi \"Main\"".to_string(),
            ip_addr: Ipv4Addr::new(192, 168, 1, 15),
            gateway_addr: Some(Ipv4Addr::new(192, 168, 1, 1)),
            dns_servers: vec![Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(8, 8, 8, 8)],
            link_speed_mbps: Some(866),
        };
        let steps = vec![StepResult {
            name: "ネットワークアダプタ",
            success: true,
            detail: "正常".to_string(),
            elapsed_ms: 5,
        }];
        let path = Path::new(r"C:\Temp\NetChecker_test.txt");
        let ctx = JsonContext {
            version: "0.5.0",
            time_display: "2026-11-13 19:00:00",
            target_name: "既定",
            proxy_info: Some("proxy.local:8080"),
            is_healthy: true,
            step_results: &steps,
            overall_title: "【総合判定】: 正常",
            overall_advice: "正常です。",
            report_path: Some(path),
            adapter_info: Some(&adapter),
        };
        let out = generate_json_output(&ctx);
        assert!(out.contains("\"proxy\": \"proxy.local:8080\","));
        assert!(out.contains("\"name\": \"Wi-Fi \\\"Main\\\"\","));
        assert!(out.contains("\"ip_address\": \"192.168.1.15\","));
        assert!(out.contains("\"gateway\": \"192.168.1.1\","));
        assert!(out.contains("\"dns_servers\": [\"192.168.1.1\", \"8.8.8.8\"],"));
        assert!(out.contains("\"link_speed_mbps\": 866"));
        assert!(out.contains(r#""report_path": "C:\\Temp\\NetChecker_test.txt""#));
    }
}
