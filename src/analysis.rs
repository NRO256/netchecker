/// 5段階診断の各ステップ成否ステータス
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepStatuses {
    pub adapter_ok: bool,
    pub router_ok: bool,
    pub external_ping_ok: bool,
    pub dns_ok: bool,
    pub web_ok: bool,
}

/// 総合診断の判定結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosisOutcome {
    pub title: &'static str,
    pub advice: &'static str,
    pub is_healthy: bool,
    pub exit_code: i32,
}

impl DiagnosisOutcome {
    const fn ok(title: &'static str, advice: &'static str) -> Self {
        Self {
            title,
            advice,
            is_healthy: true,
            exit_code: 0,
        }
    }

    const fn err(title: &'static str, advice: &'static str) -> Self {
        Self {
            title,
            advice,
            is_healthy: false,
            exit_code: 2,
        }
    }
}

/// 診断結果の組み合わせに基づく総合判定タイトル・アドバイス・正常フラグ・終了コードの導出
pub const fn evaluate_diagnosis(statuses: StepStatuses) -> DiagnosisOutcome {
    if !statuses.adapter_ok {
        DiagnosisOutcome::err(
            "【総合判定】: 端末内部エラー",
            "LANケーブルが抜けているか、Wi-Fiがオフになっています。PCのネットワーク設定を確認してください。",
        )
    } else if !statuses.router_ok
        && !statuses.external_ping_ok
        && !statuses.dns_ok
        && !statuses.web_ok
    {
        DiagnosisOutcome::err(
            "【総合判定】: 宅内ネットワークエラー",
            "ルーターと通信できません。Wi-Fiの暗号化キー間違い、LANケーブルの接触不良、またはルーターの電源を確認してください。",
        )
    } else if !statuses.external_ping_ok && !statuses.dns_ok && !statuses.web_ok {
        DiagnosisOutcome::err(
            "【総合判定】: 回線・プロバイダ側エラー",
            "ルーターまでは届いていますが外部に出られません。光回線終端装置（ONU）やモデムのランプ状態、インターネット契約状況を確認してください。",
        )
    } else if !statuses.dns_ok {
        DiagnosisOutcome::err(
            "【総合判定】: 名前解決エラー",
            "インターネット回線自体は繋がっていますが、Webサイトの住所変換ができません。ルーターまたはPCのDNSサーバー設定を確認してください。",
        )
    } else if !statuses.web_ok {
        DiagnosisOutcome::err(
            "【総合判定】: Web通信遮断エラー",
            "インターネットには繋がっていますがWeb閲覧が遮断されています。セキュリティソフト、ファイアウォール、プロキシ設定を確認してください。",
        )
    } else if !statuses.external_ping_ok {
        DiagnosisOutcome::ok(
            "【総合判定】: 正常 (ICMP制限環境)",
            "Web通信および名前解決は正常です。（※ファイアウォールやセキュリティ設定により外部Ping/ICMP通信のみ遮断されていますが、インターネット利用に支障はありません）",
        )
    } else if !statuses.router_ok {
        DiagnosisOutcome::ok(
            "【総合判定】: 正常 (ルーターPing応答なし)",
            "インターネットに正しく接続されています。（※ルーターのセキュリティ設定等によりPing応答が拒否されていますが、通信に支障はありません）",
        )
    } else {
        DiagnosisOutcome::ok(
            "【総合判定】: 正常",
            "インターネットに正しく接続されています。",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statuses(s1: bool, s2: bool, s3: bool, s4: bool, s5: bool) -> StepStatuses {
        StepStatuses {
            adapter_ok: s1,
            router_ok: s2,
            external_ping_ok: s3,
            dns_ok: s4,
            web_ok: s5,
        }
    }

    #[test]
    fn test_evaluate_diagnosis_pattern1_internal_error() {
        let out = evaluate_diagnosis(statuses(false, false, false, false, false));
        assert_eq!(out.title, "【総合判定】: 端末内部エラー");
        assert!(out.advice.contains("LANケーブル"));
        assert!(!out.is_healthy);
        assert_eq!(out.exit_code, 2);

        let out2 = evaluate_diagnosis(statuses(false, true, true, true, true));
        assert_eq!(out2.title, "【総合判定】: 端末内部エラー");
        assert!(!out2.is_healthy);
    }

    #[test]
    fn test_evaluate_diagnosis_pattern2_lan_error() {
        let out = evaluate_diagnosis(statuses(true, false, false, false, false));
        assert_eq!(out.title, "【総合判定】: 宅内ネットワークエラー");
        assert!(out.advice.contains("ルーターと通信できません"));
        assert!(!out.is_healthy);
        assert_eq!(out.exit_code, 2);
    }

    #[test]
    fn test_evaluate_diagnosis_pattern3_wan_isp_error() {
        let out = evaluate_diagnosis(statuses(true, true, false, false, false));
        assert_eq!(out.title, "【総合判定】: 回線・プロバイダ側エラー");
        assert!(
            out.advice
                .contains("ルーターまでは届いていますが外部に出られません")
        );
        assert!(!out.is_healthy);
        assert_eq!(out.exit_code, 2);
    }

    #[test]
    fn test_evaluate_diagnosis_pattern4_dns_error() {
        let out = evaluate_diagnosis(statuses(true, true, true, false, true));
        assert_eq!(out.title, "【総合判定】: 名前解決エラー");
        assert!(out.advice.contains("Webサイトの住所変換ができません"));
        assert!(!out.is_healthy);
        assert_eq!(out.exit_code, 2);

        let out2 = evaluate_diagnosis(statuses(true, true, true, false, false));
        assert_eq!(out2.title, "【総合判定】: 名前解決エラー");
        assert!(!out2.is_healthy);
    }

    #[test]
    fn test_evaluate_diagnosis_pattern5_web_block_error() {
        let out = evaluate_diagnosis(statuses(true, true, true, true, false));
        assert_eq!(out.title, "【総合判定】: Web通信遮断エラー");
        assert!(out.advice.contains("Web閲覧が遮断されています"));
        assert!(!out.is_healthy);
        assert_eq!(out.exit_code, 2);
    }

    #[test]
    fn test_evaluate_diagnosis_pattern6_icmp_restricted() {
        let out = evaluate_diagnosis(statuses(true, true, false, true, true));
        assert_eq!(out.title, "【総合判定】: 正常 (ICMP制限環境)");
        assert!(out.advice.contains("ICMP通信のみ遮断"));
        assert!(out.is_healthy);
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn test_evaluate_diagnosis_pattern7_router_ping_rejected() {
        let out = evaluate_diagnosis(statuses(true, false, true, true, true));
        assert_eq!(out.title, "【総合判定】: 正常 (ルーターPing応答なし)");
        assert!(
            out.advice
                .contains("ルーターのセキュリティ設定等によりPing応答が拒否")
        );
        assert!(out.is_healthy);
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn test_evaluate_diagnosis_pattern8_all_ok() {
        let out = evaluate_diagnosis(statuses(true, true, true, true, true));
        assert_eq!(out.title, "【総合判定】: 正常");
        assert_eq!(out.advice, "インターネットに正しく接続されています。");
        assert!(out.is_healthy);
        assert_eq!(out.exit_code, 0);
    }
}
