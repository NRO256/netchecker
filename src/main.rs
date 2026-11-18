//! NetChecker Portable - ネットワーク接続確認ツール
//! 完全ポータブル・依存ゼロ・一般ユーザー権限動作
//! バージョン: 0.5.1

mod analysis;
mod cli;
mod diagnostics;
mod ffi;
mod report;

use std::io::Write;

use analysis::{StepStatuses, evaluate_diagnosis};
use cli::{parse_args, print_help, print_version, resolve_target_config};
use diagnostics::{
    StepCollector, check_dns_resolution, check_external_ping, check_network_adapter,
    check_router_ping, check_web_access, detect_system_proxy,
};
use ffi::{format_system_time, get_now_system_time, init_console_utf8, wait_for_keypress};
use report::{
    JsonContext, SaveReportResult, build_header, build_overall, generate_json_output,
    save_report_with_fallback,
};

fn main() {
    init_console_utf8();

    let args: Vec<String> = std::env::args().collect();
    let version_str = env!("CARGO_PKG_VERSION");

    let opts = match parse_args(&args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("エラー: {err}");
            std::process::exit(1);
        }
    };

    if opts.show_version {
        print_version(version_str);
        return;
    }

    if opts.show_help {
        print_help(version_str);
        return;
    }

    let target_cfg = resolve_target_config(opts.target.as_deref());
    let proxy_info = detect_system_proxy();

    let now_st = get_now_system_time();
    let (time_display, time_file) = format_system_time(&now_st);

    // ヘッダー生成・コンソール表示
    let custom_target_display = target_cfg
        .is_custom
        .then_some(target_cfg.display_name.as_str());
    let mut report_lines = build_header(
        version_str,
        &time_display,
        custom_target_display,
        proxy_info.as_deref(),
        opts.json,
    );

    // 診断ステップ実行（1〜5）
    let mut collector = StepCollector::new(opts.json);
    let timeout_ms = opts.timeout_ms;

    let (step1_ok, step1_detail, adapter_info, step1_ms) =
        check_network_adapter(target_cfg.ping_primary);
    collector.record_step(1, "ネットワークアダプタ", step1_ok, step1_detail, step1_ms);

    let (step2_ok, step2_detail, step2_ms) = check_router_ping(
        adapter_info.as_ref().and_then(|ad| ad.gateway_addr),
        timeout_ms,
    );
    collector.record_step(2, "ルーター (宅内機器)", step2_ok, step2_detail, step2_ms);

    let (step3_ok, step3_detail, step3_ms) = check_external_ping(
        target_cfg.ping_primary,
        target_cfg.ping_secondary,
        &target_cfg.dns_primary,
        timeout_ms,
    );
    collector.record_step(3, "インターネット出口", step3_ok, step3_detail, step3_ms);

    let (step4_ok, step4_detail, step4_ms) =
        check_dns_resolution(&target_cfg.dns_primary, target_cfg.dns_secondary.as_deref());
    collector.record_step(4, "名前解決 (DNS)", step4_ok, step4_detail, step4_ms);

    let (step5_ok, step5_detail, step5_ms) = check_web_access(
        &target_cfg.web_primary,
        target_cfg.web_secondary.as_deref(),
        timeout_ms,
        proxy_info.as_deref(),
    );
    collector.record_step(5, "Webアクセス", step5_ok, step5_detail, step5_ms);

    if !opts.json {
        println!();
    }
    report_lines.extend(collector.report_lines);
    report_lines.push(String::new());
    let step_results = collector.results;

    // 不具合診断マトリクス・ガイダンス判定
    let outcome = evaluate_diagnosis(StepStatuses {
        adapter_ok: step1_ok,
        router_ok: step2_ok,
        external_ping_ok: step3_ok,
        dns_ok: step4_ok,
        web_ok: step5_ok,
    });
    let overall_lines = build_overall(outcome.title, outcome.advice, opts.json);
    report_lines.extend(overall_lines);

    // レポートファイル自動保存
    let save_res = (!opts.no_report).then(|| save_report_with_fallback(&time_file, &report_lines));

    // 出力形式の分岐 (JSON または 通常コンソール表示)
    if opts.json {
        let json_ctx = JsonContext {
            version: version_str,
            time_display: &time_display,
            target_name: &target_cfg.display_name,
            proxy_info: proxy_info.as_deref(),
            is_healthy: outcome.is_healthy,
            step_results: &step_results,
            overall_title: outcome.title,
            overall_advice: outcome.advice,
            report_path: save_res.as_ref().and_then(SaveReportResult::saved_path),
            adapter_info: adapter_info.as_ref(),
        };
        let json_str = generate_json_output(&json_ctx);
        print!("{json_str}");
    } else {
        if let Some(ref res) = save_res {
            match res {
                SaveReportResult::Saved {
                    path,
                    loc_desc,
                    fallback_used,
                } => {
                    let path_disp = path.display();
                    if *fallback_used {
                        println!(
                            "※ 配置先への書き込み権限がないため、{loc_desc}に退避保存しました:\n   {path_disp}"
                        );
                    } else {
                        println!("※ 診断レポートを保存しました: {path_disp}");
                    }
                }
                SaveReportResult::Failed => {
                    println!(
                        "※ レポートファイルの保存に失敗しました（保存先への書き込み権限がありません）。"
                    );
                }
            }
            println!();
        }

        // 終了時の待機 (キー入力待ち)
        if !opts.no_wait {
            print!("何かキーを押すと終了します...");
            let _ = std::io::stdout().flush();
            wait_for_keypress();
            println!();
        }
    }

    if outcome.exit_code != 0 {
        std::process::exit(outcome.exit_code);
    }
}
