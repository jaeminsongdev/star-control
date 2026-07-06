use super::super::super::super::ChangeType;
use super::KeywordRule;

pub(super) const ROUTINE_KEYWORD_RULES: &[KeywordRule] = &[
    KeywordRule::new(
        ChangeType::DocsOnly,
        &["readme", "docs", "documentation", "문서"],
        "documentation change requested",
    ),
    KeywordRule::new(
        ChangeType::ExampleChange,
        &["example", "fixture", "예시", "샘플"],
        "example or fixture change requested",
    ),
    KeywordRule::new(
        ChangeType::SchemaChange,
        &["schema", "specs/schemas", "스키마"],
        "schema files or schema contract mentioned",
    ),
    KeywordRule::new(
        ChangeType::SchemaBreakingChange,
        &["breaking schema", "schema breaking", "breaking change"],
        "breaking schema change mentioned",
    ),
    KeywordRule::new(
        ChangeType::RuntimeCodeChange,
        &["implement", "runtime", "code", "rust", "구현", "코드"],
        "runtime implementation requested",
    ),
    KeywordRule::new(
        ChangeType::MultiPackageChange,
        &["multi package", "workspace", "여러 package", "여러 패키지"],
        "multi-package change mentioned",
    ),
    KeywordRule::new(
        ChangeType::ProviderContractChange,
        &["provider contract", "provider registry"],
        "provider contract change mentioned",
    ),
    KeywordRule::new(
        ChangeType::DependencyAddition,
        &[
            "dependency",
            "cargo.toml",
            "package.json",
            "의존성",
            "패키지 설치",
        ],
        "dependency addition or package manager change mentioned",
    ),
    KeywordRule::new(
        ChangeType::DependencyVersionChange,
        &["dependency version", "version bump", "버전 변경"],
        "dependency version change mentioned",
    ),
    KeywordRule::new(
        ChangeType::WorkflowChange,
        &["workflow", ".github/workflows", "ci", "워크플로"],
        "workflow or CI change mentioned",
    ),
    KeywordRule::new(
        ChangeType::PublicApiChange,
        &["public api", "breaking api", "공개 api"],
        "public API change mentioned",
    ),
    KeywordRule::new(
        ChangeType::CredentialChange,
        &[
            "credential",
            "api key",
            "token",
            "password",
            "비밀번호",
            "토큰",
        ],
        "credential-sensitive change mentioned",
    ),
    KeywordRule::new(
        ChangeType::SensitiveDataExposure,
        &[
            "print secret",
            "show token",
            "dump credential",
            "민감정보 출력",
            "비밀 출력",
        ],
        "sensitive data exposure requested",
    ),
    KeywordRule::new(
        ChangeType::ReleaseChange,
        &["release", "publish", "릴리즈", "배포 준비"],
        "release change mentioned",
    ),
    KeywordRule::new(
        ChangeType::DeployChange,
        &["deploy", "deployment", "배포"],
        "deploy change mentioned",
    ),
];
