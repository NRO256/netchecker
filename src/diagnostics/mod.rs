pub mod adapter;
pub mod dns;
pub mod ping;
pub mod types;
pub mod web;

pub use adapter::check_network_adapter;
pub use dns::check_dns_resolution;
pub use ping::{check_external_ping, check_router_ping};
pub use types::{AdapterInfo, StepResult};
pub use web::{check_web_access, detect_system_proxy};

/// プライマリ・セカンダリのフォールバック試行結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackOutcome<T> {
    PrimaryOk {
        val: T,
        elapsed_ms: u64,
    },
    SecondaryOk {
        val: T,
        elapsed_ms: u64,
    },
    FailedSingle {
        err: String,
        elapsed_ms: u64,
    },
    FailedBoth {
        primary_err: String,
        secondary_err: String,
        elapsed_ms: u64,
    },
}

/// プライマリ対象を試行し、失敗時にセカンダリ対象があれば自動フォールバックして経過時間を合算する共通ヘルパー
pub fn execute_with_fallback<P, T, F>(
    primary: P,
    secondary: Option<P>,
    mut check_fn: F,
) -> FallbackOutcome<T>
where
    F: FnMut(P) -> Result<(T, u64), (String, u64)>,
{
    match check_fn(primary) {
        Ok((val, ms1)) => FallbackOutcome::PrimaryOk {
            val,
            elapsed_ms: ms1,
        },
        Err((err1, ms1)) => {
            if let Some(sec) = secondary {
                match check_fn(sec) {
                    Ok((val, ms2)) => FallbackOutcome::SecondaryOk {
                        val,
                        elapsed_ms: ms1 + ms2,
                    },
                    Err((err2, ms2)) => FallbackOutcome::FailedBoth {
                        primary_err: err1,
                        secondary_err: err2,
                        elapsed_ms: ms1 + ms2,
                    },
                }
            } else {
                FallbackOutcome::FailedSingle {
                    err: err1,
                    elapsed_ms: ms1,
                }
            }
        }
    }
}

const STEP_COUNT: usize = 5;

/// 診断ステップの進行記録・コンソール表示・結果蓄積を管理するコレクター
pub struct StepCollector {
    pub results: Vec<StepResult>,
    pub report_lines: Vec<String>,
    json_mode: bool,
}

impl StepCollector {
    pub fn new(json_mode: bool) -> Self {
        Self {
            results: Vec::with_capacity(STEP_COUNT),
            report_lines: Vec::with_capacity(STEP_COUNT),
            json_mode,
        }
    }

    /// 診断ステップ結果を記録し、必要に応じてコンソールに出力する
    pub fn record_step(
        &mut self,
        step_num: usize,
        name: &'static str,
        success: bool,
        detail: String,
        elapsed_ms: u64,
    ) {
        let status = if success { "[OK]" } else { "[NG]" };
        let line = format!("[{step_num}/{STEP_COUNT}] {name:<16} ... {status} ({detail})");

        if !self.json_mode {
            println!("{line}");
        }

        self.report_lines
            .push(format!("{line} (所要時間: {elapsed_ms}ms)"));

        self.results.push(StepResult {
            name,
            success,
            detail,
            elapsed_ms,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_collector_record_step() {
        let mut collector = StepCollector::new(true); // json_mode: true (標準出力抑制)
        collector.record_step(1, "テスト項目", true, "正常終了".to_string(), 42);

        assert_eq!(collector.results.len(), 1);
        let res = &collector.results[0];
        assert_eq!(res.name, "テスト項目");
        assert!(res.success);
        assert_eq!(res.detail, "正常終了");
        assert_eq!(res.elapsed_ms, 42);

        assert_eq!(collector.report_lines.len(), 1);
        assert!(collector.report_lines[0].contains("[1/5] テスト項目"));
        assert!(collector.report_lines[0].contains("[OK]"));
        assert!(collector.report_lines[0].contains("42ms"));
    }

    #[test]
    fn test_execute_with_fallback_primary_ok() {
        let outcome = execute_with_fallback("primary", Some("secondary"), |target| {
            assert_eq!(target, "primary");
            Ok((200u32, 15))
        });
        assert_eq!(
            outcome,
            FallbackOutcome::PrimaryOk {
                val: 200,
                elapsed_ms: 15
            }
        );
    }

    #[test]
    fn test_execute_with_fallback_secondary_ok() {
        let outcome = execute_with_fallback("primary", Some("secondary"), |target| {
            if target == "primary" {
                Err(("primary timeout".to_string(), 100))
            } else {
                Ok((200u32, 25))
            }
        });
        assert_eq!(
            outcome,
            FallbackOutcome::SecondaryOk {
                val: 200,
                elapsed_ms: 125
            }
        );
    }

    #[test]
    fn test_execute_with_fallback_both_failed() {
        let outcome: FallbackOutcome<u32> =
            execute_with_fallback("primary", Some("secondary"), |target| {
                Err((format!("{target} failed"), 50))
            });
        assert_eq!(
            outcome,
            FallbackOutcome::FailedBoth {
                primary_err: "primary failed".to_string(),
                secondary_err: "secondary failed".to_string(),
                elapsed_ms: 100,
            }
        );
    }

    #[test]
    fn test_execute_with_fallback_single_failed() {
        let outcome: FallbackOutcome<u32> = execute_with_fallback("primary", None, |_| {
            Err(("only primary failed".to_string(), 30))
        });
        assert_eq!(
            outcome,
            FallbackOutcome::FailedSingle {
                err: "only primary failed".to_string(),
                elapsed_ms: 30,
            }
        );
    }
}
