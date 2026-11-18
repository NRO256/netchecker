use std::net::Ipv4Addr;

// --- 既定の診断ターゲット定数 ---

pub const PRIMARY_PING_TARGET: Ipv4Addr = Ipv4Addr::new(8, 8, 8, 8);
const SECONDARY_PING_TARGET: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);
const PRIMARY_DNS_TARGET: &str = "www.google.com";
const SECONDARY_DNS_TARGET: &str = "cloudflare.com";
const PRIMARY_WEB_URL: &str = "https://www.google.com";
const SECONDARY_WEB_URL: &str = "https://www.cloudflare.com";
const DEFAULT_TIMEOUT_MS: u32 = 2500;
const MIN_TIMEOUT_MS: u32 = 100;
const MAX_TIMEOUT_MS: u32 = 30_000;

// --- CLI オプションと診断ターゲット設定 ---

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CliOptions {
    pub show_version: bool,
    pub show_help: bool,
    pub no_wait: bool,
    pub json: bool,
    pub no_report: bool,
    pub target: Option<String>,
    pub timeout_ms: u32,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            show_version: false,
            show_help: false,
            no_wait: false,
            json: false,
            no_report: false,
            target: None,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

fn parse_timeout_ms(val: &str) -> Result<u32, String> {
    match val.parse::<u32>() {
        Ok(ms) if (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&ms) => Ok(ms),
        _ => Err(format!(
            "--timeout (-w) オプションには {MIN_TIMEOUT_MS}〜{MAX_TIMEOUT_MS} のミリ秒（整数）を指定してください: {val}"
        )),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetConfig {
    pub is_custom: bool,
    pub display_name: String,
    pub ping_primary: Option<Ipv4Addr>,
    pub ping_secondary: Option<Ipv4Addr>,
    pub dns_primary: String,
    pub dns_secondary: Option<String>,
    pub web_primary: String,
    pub web_secondary: Option<String>,
}

fn parse_target_value(val: &str) -> Result<String, String> {
    if val.is_empty() {
        Err("--target オプションにはホスト名またはURLの指定が必要です。".to_string())
    } else {
        Ok(val.to_string())
    }
}

/// コマンドライン引数をパース
pub fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let mut opts = CliOptions::default();
    let mut iter = args.iter().skip(1);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-v" | "--version" | "/v" | "/version" => {
                opts.show_version = true;
            }
            "-h" | "--help" | "/?" | "/h" => {
                opts.show_help = true;
            }
            "--no-wait" => {
                opts.no_wait = true;
            }
            "--json" => {
                opts.json = true;
                opts.no_wait = true;
            }
            "--no-report" => {
                opts.no_report = true;
            }
            "-t" | "--target" => {
                let val = iter.next().ok_or_else(|| {
                    "--target オプションにはホスト名またはURLの指定が必要です。".to_string()
                })?;
                opts.target = Some(parse_target_value(val)?);
            }
            "-w" | "--timeout" => {
                let val = iter.next().ok_or_else(|| {
                    "--timeout (-w) オプションにはミリ秒（数値）の指定が必要です。".to_string()
                })?;
                opts.timeout_ms = parse_timeout_ms(val)?;
            }
            s => {
                if let Some(val) = s.strip_prefix("--target=") {
                    opts.target = Some(parse_target_value(val)?);
                } else if let Some(val) = s.strip_prefix("--timeout=") {
                    opts.timeout_ms = parse_timeout_ms(val)?;
                } else {
                    return Err(format!(
                        "不明なオプションです: {s}\n'NetChecker.exe --help' で使い方を確認してください。"
                    ));
                }
            }
        }
    }
    Ok(opts)
}

/// 診断対象ターゲットの設定を解決（ネットワークI/Oを伴わない純粋関数）
pub fn resolve_target_config(custom_target: Option<&str>) -> TargetConfig {
    let Some(raw_target) = custom_target else {
        return TargetConfig {
            is_custom: false,
            display_name: "既定 (Google / Cloudflare)".to_string(),
            ping_primary: Some(PRIMARY_PING_TARGET),
            ping_secondary: Some(SECONDARY_PING_TARGET),
            dns_primary: PRIMARY_DNS_TARGET.to_string(),
            dns_secondary: Some(SECONDARY_DNS_TARGET.to_string()),
            web_primary: PRIMARY_WEB_URL.to_string(),
            web_secondary: Some(SECONDARY_WEB_URL.to_string()),
        };
    };

    let raw = raw_target.trim();
    let (web_url, host_part) = if let Some((scheme, rest)) = raw.split_once("://")
        && (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
    {
        (raw.to_string(), rest)
    } else {
        (format!("https://{raw}"), raw)
    };

    let host_slice = host_part
        .split(['/', ':', '?', '#'])
        .next()
        .unwrap_or(host_part);
    let host_str = host_slice.to_string();
    // IPv4リテラルの場合のみ即時設定し、ドメイン名の場合は診断フェーズで解決する
    let ping_ip = host_slice.parse::<Ipv4Addr>().ok();

    TargetConfig {
        is_custom: true,
        display_name: format!("カスタム対象 ({raw})"),
        ping_primary: ping_ip,
        ping_secondary: None,
        dns_primary: host_str,
        dns_secondary: None,
        web_primary: web_url,
        web_secondary: None,
    }
}

pub fn print_version(version: &str) {
    println!("NetChecker Portable v{version}");
}

pub fn print_help(version: &str) {
    println!(
        "NetChecker Portable v{version} - ネットワーク接続確認ツール\n\
        \n\
        使い方: NetChecker.exe [オプション]\n\
        \n\
        オプション:\n\
        \x20 -v, --version           バージョン情報を表示して終了します\n\
        \x20 -h, --help              このヘルプメッセージを表示して終了します\n\
        \x20 -t, --target <対象>     診断対象をカスタム指定します（ドメイン名、IPアドレス、またはURL）\n\
        \x20                         例: -t example.com / -t 192.168.1.1 / -t https://intranet.local\n\
        \x20 -w, --timeout <ミリ秒>  PingおよびWeb通信のタイムアウト時間を指定します（既定: 2500、範囲: 100〜30000）\n\
        \x20 --json                  診断結果をJSON形式で標準出力に出力します（スクリプト・監視用）\n\
        \x20 --no-wait               診断完了後のキー入力待ちをスキップします（自動実行・バッチ用）\n\
        \x20 --no-report             診断結果レポートファイルの保存をスキップします"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_default() {
        let args = vec!["NetChecker.exe".to_string()];
        let opts = parse_args(&args).expect("引数なしで正常パースされること");
        assert_eq!(opts, CliOptions::default());
    }

    #[test]
    fn test_parse_args_version_flags() {
        for flag in &["-v", "--version", "/v", "/version"] {
            let args = vec!["NetChecker.exe".to_string(), flag.to_string()];
            let opts =
                parse_args(&args).unwrap_or_else(|_| panic!("{} のパースに成功すること", flag));
            assert!(
                opts.show_version,
                "{} で show_version が true になること",
                flag
            );
            assert!(!opts.show_help);
            assert!(!opts.no_wait);
        }
    }

    #[test]
    fn test_parse_args_help_flags() {
        for flag in &["-h", "--help", "/?", "/h"] {
            let args = vec!["NetChecker.exe".to_string(), flag.to_string()];
            let opts =
                parse_args(&args).unwrap_or_else(|_| panic!("{} のパースに成功すること", flag));
            assert!(opts.show_help, "{} で show_help が true になること", flag);
            assert!(!opts.show_version);
        }
    }

    #[test]
    fn test_parse_args_no_wait() {
        let args = vec!["NetChecker.exe".to_string(), "--no-wait".to_string()];
        let opts = parse_args(&args).expect("パースに成功すること");
        assert!(opts.no_wait);
        assert!(!opts.json);
    }

    #[test]
    fn test_parse_args_json() {
        let args = vec!["NetChecker.exe".to_string(), "--json".to_string()];
        let opts = parse_args(&args).expect("パースに成功すること");
        assert!(opts.json);
        assert!(
            opts.no_wait,
            "--json 指定時は自動で no_wait が true になること"
        );
    }

    #[test]
    fn test_parse_args_no_report() {
        let args = vec!["NetChecker.exe".to_string(), "--no-report".to_string()];
        let opts = parse_args(&args).expect("パースに成功すること");
        assert!(opts.no_report);
    }

    #[test]
    fn test_parse_args_target_separate() {
        for flag in &["-t", "--target"] {
            let args = vec![
                "NetChecker.exe".to_string(),
                flag.to_string(),
                "example.com".to_string(),
            ];
            let opts = parse_args(&args)
                .unwrap_or_else(|_| panic!("{} example.com のパースに成功すること", flag));
            assert_eq!(opts.target.as_deref(), Some("example.com"));
        }
    }

    #[test]
    fn test_parse_args_target_inline() {
        let args = vec![
            "NetChecker.exe".to_string(),
            "--target=https://1.1.1.1".to_string(),
        ];
        let opts = parse_args(&args).expect("パースに成功すること");
        assert_eq!(opts.target.as_deref(), Some("https://1.1.1.1"));

        let args_prefix = vec![
            "NetChecker.exe".to_string(),
            "--target=--target=example.com".to_string(),
        ];
        let opts_prefix =
            parse_args(&args_prefix).expect("strip_prefixで値先頭を過剰除去しないこと");
        assert_eq!(opts_prefix.target.as_deref(), Some("--target=example.com"));
    }

    #[test]
    fn test_parse_args_target_missing_arg() {
        let args_short = vec!["NetChecker.exe".to_string(), "-t".to_string()];
        assert!(parse_args(&args_short).is_err());

        let args_long = vec!["NetChecker.exe".to_string(), "--target".to_string()];
        assert!(parse_args(&args_long).is_err());

        let args_empty_inline = vec!["NetChecker.exe".to_string(), "--target=".to_string()];
        assert!(parse_args(&args_empty_inline).is_err());
    }

    #[test]
    fn test_parse_args_unknown_option() {
        let args = vec!["NetChecker.exe".to_string(), "--foo-bar".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn test_parse_args_combinations() {
        let args = vec![
            "NetChecker.exe".to_string(),
            "--json".to_string(),
            "-t".to_string(),
            "https://my-router.local".to_string(),
            "--no-report".to_string(),
        ];
        let opts = parse_args(&args).expect("複数オプションパースに成功すること");
        assert!(opts.json);
        assert!(opts.no_wait);
        assert!(opts.no_report);
        assert_eq!(opts.target.as_deref(), Some("https://my-router.local"));
    }

    #[test]
    fn test_resolve_target_config_default() {
        let config = resolve_target_config(None);
        assert!(!config.is_custom);
        assert_eq!(config.display_name, "既定 (Google / Cloudflare)");
        assert_eq!(config.ping_primary, Some(PRIMARY_PING_TARGET));
        assert_eq!(config.ping_secondary, Some(SECONDARY_PING_TARGET));
        assert_eq!(config.dns_primary, PRIMARY_DNS_TARGET);
        assert_eq!(config.dns_secondary, Some(SECONDARY_DNS_TARGET.to_string()));
        assert_eq!(config.web_primary, PRIMARY_WEB_URL);
    }

    #[test]
    fn test_resolve_target_config_https_url() {
        let config = resolve_target_config(Some("https://example.com/api/test?query=1#frag"));
        assert!(config.is_custom);
        assert!(config.display_name.contains("カスタム対象"));
        assert_eq!(
            config.web_primary,
            "https://example.com/api/test?query=1#frag"
        );
        assert_eq!(config.dns_primary, "example.com");
        assert!(config.ping_primary.is_none());
        assert!(config.ping_secondary.is_none());
        assert!(config.dns_secondary.is_none());
        assert!(config.web_secondary.is_none());
    }

    #[test]
    fn test_resolve_target_config_http_url_with_port() {
        let config = resolve_target_config(Some("http://192.168.1.254:8080/setup"));
        assert!(config.is_custom);
        assert_eq!(config.web_primary, "http://192.168.1.254:8080/setup");
        assert_eq!(config.dns_primary, "192.168.1.254");
        assert_eq!(config.ping_primary, Some(Ipv4Addr::new(192, 168, 1, 254)));
    }

    #[test]
    fn test_resolve_target_config_hostname_only() {
        let config = resolve_target_config(Some("my.gateway.internal"));
        assert!(config.is_custom);
        assert_eq!(config.dns_primary, "my.gateway.internal");
        assert_eq!(config.web_primary, "https://my.gateway.internal");
    }

    #[test]
    fn test_resolve_target_config_ipv4_literal() {
        let config = resolve_target_config(Some("10.0.0.1"));
        assert!(config.is_custom);
        assert_eq!(config.ping_primary, Some(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(config.dns_primary, "10.0.0.1");
        assert_eq!(config.web_primary, "https://10.0.0.1");
    }

    #[test]
    fn test_parse_args_timeout_valid() {
        let args1 = vec![
            "NetChecker.exe".to_string(),
            "-w".to_string(),
            "1000".to_string(),
        ];
        let opts1 = parse_args(&args1).expect("-w 1000 のパースに成功すること");
        assert_eq!(opts1.timeout_ms, 1000);

        let args2 = vec!["NetChecker.exe".to_string(), "--timeout=5000".to_string()];
        let opts2 = parse_args(&args2).expect("--timeout=5000 のパースに成功すること");
        assert_eq!(opts2.timeout_ms, 5000);
    }

    #[test]
    fn test_parse_args_timeout_invalid() {
        let args_missing = vec!["NetChecker.exe".to_string(), "-w".to_string()];
        assert!(parse_args(&args_missing).is_err());

        let args_too_small = vec![
            "NetChecker.exe".to_string(),
            "--timeout".to_string(),
            "50".to_string(),
        ];
        assert!(parse_args(&args_too_small).is_err());

        let args_too_large = vec!["NetChecker.exe".to_string(), "--timeout=60000".to_string()];
        assert!(parse_args(&args_too_large).is_err());

        let args_non_num = vec![
            "NetChecker.exe".to_string(),
            "-w".to_string(),
            "abc".to_string(),
        ];
        assert!(parse_args(&args_non_num).is_err());
    }
}
