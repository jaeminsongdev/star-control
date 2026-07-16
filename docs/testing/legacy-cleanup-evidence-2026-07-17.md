# P-0011 구 체계 정리 증거 — 2026-07-17

## 결정과 경계

- 사용자는 `.스타`와 현재 체계에 필요 없는 구 체계의 삭제, 관련 정본 갱신, 최종 commit·push를 명시적으로 승인했다.
- 기존 `star-workflow`, `local-ai`, `one-project` origin은 GitHub에서 모두 `Repository not found`였다.
- 사용자는 대체 원격 저장소를 만들지 않고 Star-Workflow와 로컬_AI 로컬 프로젝트를 제거하도록 결정했다.
- 하나_프로젝트는 실제 source·fixture·문서를 보존하고 nested `.git`과 false-green workflow만 제거하도록 범위를 정정했다.
- 원격 GitHub repository 삭제와 branch protection 변경은 수행하지 않는다. 현재 존재하는 활성 workflow도 삭제하지 않는다.

## 삭제 직전 기준선

| 대상 | 기준선 | tracked dirty | staged | untracked | dirty diff SHA-256 |
|---|---:|---:|---:|---:|---|
| `D:/개발/관제/Star-Workflow` | `f45f46626b636e879c91d37e94f100f218738a5d` | 19 | 0 | 0 | `bf08115d49cb1b020645df20161c54b44efaba549e0536218a44948b16fce95d` |
| `D:/개발/검토예정/로컬_AI` | `8ac4028e8a32bd1d6c8697dc788dc268301347b0` | 2 | 0 | 0 | `29c3c84fd77ad62c0173ae93a2bd8d1abbbef0e893a3eb6bbbfa32dc502c6fdc` |
| `D:/개발/언어/래거시/하나_프로젝트` | `f6ffd0c625a67334c89ad6030548c9f60b97800e` | 3 | 0 | 0 | `f473cbdea260280d4333d03c09a407542e5723e5110ad88ba3b902dc4402ee5e` |

dirty diff는 사용자의 삭제 결정에 따라 별도 원격이나 archive로 보존하지 않는다. 위 hash는 삭제 대상의 동일성 확인용이며 내용을 복구하지 못한다.

| 경로 | 파일 | 하위 디렉터리 | 바이트 | reparse point |
|---|---:|---:|---:|---:|
| `D:/개발/.스타` | 618 | 193 | 1,290,354 | 0 |
| `D:/개발/관제/Star-Workflow` | 74,805 | 3,016 | 33,255,995,744 | 0 |
| `D:/개발/검토예정/로컬_AI` | 4,163 | 646 | 1,204,108,239 | 0 |
| `D:/개발/언어/래거시/하나_프로젝트/.git` | 50 | 17 | 11,914,652 | 0 |
| `D:/개발/관제/Star-Control/legacy` | 1,177 | 391 | 2,716,919 | 0 |
| `D:/개발/.codex` | 0 | 0 | 0 | 0 |

Star-Control의 ignored `legacy/`는 현재 제품 정본이 아니며, 대표 provenance 문자열 `0.1.0-scaffold`가 기존 Git history `7ccdce5`, `ee4090c`에 존재함을 확인했다. 삭제 후 재생성 방지를 위해 `legacy/` ignore도 제거한다.

## 적용 대상

1. `D:/개발/.스타` 삭제
2. 사용자 PATH의 `D:/도구/스타워크플로우/bin` 항목 제거; 이미 없는 구 실행파일은 별도 삭제하지 않음
3. `D:/개발/관제/Star-Workflow` 전체 삭제
4. `D:/개발/검토예정/로컬_AI` 전체 삭제
5. `D:/개발/언어/래거시/하나_프로젝트/.git`과 `.github/workflows/검증.yml` 삭제; 나머지 하나_프로젝트 source 보존
6. `D:/개발/관제/Star-Control/legacy` 삭제
7. 비어 있는 `D:/개발/.codex` 삭제
8. Star-Control allowlist와 생태계 정본의 active repository·component·RAG·dependency 항목에서 구 체계 제거

## 검증 결과

- 삭제 대상 7개 경로와 구 설치 폴더는 모두 부재하며 하나_프로젝트 `Cargo.toml`과 2,048개 legacy file은 남아 있다.
- 사용자 PATH의 구 항목은 0건이고 첫 항목과 `where.exe star`는 모두 `D:/도구/Star-Control`이다. `star --version`은 `Star-Control 0.1.0`이다.
- `config.toml` SHA-256은 전후 `f38bf29a578125af201f09bf0ccd352878459e0d562b0cebcf41eab579574fe6`로 동일하다.
- 언어 `python -B 도구/검증/p00_e0_source_validate.py`: `FINDINGS=0`, `LEGACY_FILES=2048`, exit 0.
- 생태계 정본 `scripts/validate.ps1 -Profile full`: 5 checks PASS, exit 0, evidence `.validation-artifacts/20260716T170605679Z-28760/report.json`.
- Star-Control `scripts/validate.ps1 -Profile full`: 10 checks PASS, exit 0, evidence `target/validation/20260716T170713374Z-14220/report.json`.
- active canonical root AGENTS 13개는 30~80줄이고 구 도구명·구 운영 경로·특정 모델 표현이 0건이다.
- `D:/개발` 전체의 `.스타`, Star-Workflow, 로컬_AI 이름 디렉터리는 0건이며 old process reference, cargo/rustc validation process와 표시 terminal도 모두 0건이다.
