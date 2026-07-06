use super::super::super::super::ChangeType;
use super::KeywordRule;

pub(super) const SAFETY_KEYWORD_RULES: &[KeywordRule] = &[
    KeywordRule::new(
        ChangeType::ValidatorSensitiveChange,
        &[
            "validator",
            "star sentinel",
            "policy",
            "scripts/ci",
            "검증기",
        ],
        "validator-sensitive contract changed",
    ),
    KeywordRule::new(
        ChangeType::ValidatorSelfBypass,
        &[
            "disable validator",
            "skip ci",
            "bypass validator",
            "검증 우회",
        ],
        "validator self-bypass requested",
    ),
    KeywordRule::new(
        ChangeType::FileDeletion,
        &["delete file", "remove file", "파일 삭제", "삭제"],
        "file deletion mentioned",
    ),
    KeywordRule::new(
        ChangeType::BulkMove,
        &["bulk move", "move all", "대량 이동"],
        "bulk move mentioned",
    ),
    KeywordRule::new(
        ChangeType::RiskPathChange,
        &["security path", "permission", "권한"],
        "risk-sensitive path mentioned",
    ),
    KeywordRule::new(
        ChangeType::ExternalAccountChange,
        &["external account", "github settings", "외부 계정"],
        "external account change mentioned",
    ),
    KeywordRule::new(
        ChangeType::BudgetExceeded,
        &["budget exceeded", "quota exceeded", "예산 초과"],
        "budget or quota risk mentioned",
    ),
    KeywordRule::new(
        ChangeType::UnknownHighRisk,
        &["unknown risk", "risky", "위험"],
        "unknown high risk mentioned",
    ),
];
