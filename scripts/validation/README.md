# 프로젝트 공통 검증 계약

모든 pilot는 Star-Control 없이 다음 진입점을 직접 실행할 수 있다.

    pwsh ./scripts/validate.ps1 -Profile target -Unit <optional-unit> -BaseRef <optional-revision> -OutputFormat json

공개 인자는 Profile, Unit, BaseRef, OutputFormat 네 개로 고정한다. Profile 기본값은 target, OutputFormat 기본값은 text다.

## 프로필과 영향도

- quick: 일반 문서·주석·설정의 UTF-8, 링크, fence, JSON, TOML, YAML, diff 검사를 수행한다.
- target: 기본 요청이다. 실제 변경이 일반 문서뿐이면 quick으로 낮아지고, 일반 코드면 영향 unit의 target 검사를 수행하며, 공개 계약·Schema·lockfile·toolchain·검증 계약이면 full로 올라간다.
- full: 요청하면 낮추지 않는다. 전체 프로젝트와 생성 계약·conformance를 검사한다.
- release: clean worktree와 full gate를 포함한다. 서명·publish·deploy 같은 외부 효과는 수행하지 않으며 준비되지 않은 release 항목은 unverified로 보고한다.

확장자 하나만으로 영향도를 낮추지 않는다. Markdown이라도 공개 계약 또는 생성 입력이면 full 대상이다. release는 자동 선택하지 않는다.

## 범위와 결과

BaseRef는 로컬에 존재하는 commit이어야 하며 runner가 fetch하지 않는다. 비교 범위는 BaseRef...HEAD, staged, unstaged, untracked 변경의 합집합이다. BaseRef 없이 dirty 변경이 없으면 요청 프로필로 전체 프로젝트를 검사한다.

Unit은 프로젝트가 선언한 Cargo package 또는 논리 unit이다. 선택 unit 밖 변경이 남으면 검사가 성공해도 partial로 보고하고 성공 exit code를 반환하지 않는다.

기계 결과는 star.project-validation-report v1이다. 최상위 status는 pass, fail, not_run, partial, unverified, flaky 중 하나이며 다음 세 축을 별도로 유지한다.

- outcome: pass, fail, not_run, error, cancelled
- completeness: complete, partial, unverified
- stability: stable, flaky, not_evaluated

각 check는 typed command, exit code, duration, failure summary와 log_ref를 포함한다. 전체 stdout/stderr는 프로젝트의 ignored validation artifact 디렉터리에 저장한다. JSON 출력일 때 stdout에는 보고서 객체 하나만 쓴다.

검증을 시작하기 전 Unit·BaseRef 오류 또는 runner 내부 오류가 발생하면 JSON 출력은 `star.project-validation-entry-error` v1 객체 하나를 쓴다. invocation 오류는 `status=fail`, runner 오류는 `status=unverified`이며 둘 다 성공 exit code를 반환하지 않는다.

## 종료 코드

- 0: pass
- 1: 검증 실패
- 2: 잘못된 인자, BaseRef 또는 Unit
- 3: not_run, partial, unverified, flaky
- 4: runner 내부 오류

runner는 stateless하며 이전 보고서나 로그를 직접 재사용하지 않는다. input_fingerprint는 revision, dirty patch, profile·unit·BaseRef·선택 파일, Rust/Cargo/Python/PyYAML/Git/PowerShell/platform, lock/config와 실제 command를 묶는다. 매 실행마다 달라지는 artifact ID와 checkout 절대 경로는 정규화한다. Star-Control은 별도 cache key로 동일 입력의 complete·stable pass와 모든 artifact hash를 확인한 경우에만 프로젝트의 ignored derived cache를 재사용한다.

YAML은 PyYAML 6.0.3을 사용한다. CI는 hash가 고정된 requirements-validation.txt를 설치한다. 로컬에 정확한 버전이 없으면 자동 설치하지 않고 해당 check를 unverified로 남긴다.

fixtures 또는 examples 아래 invalid 경로의 파일은 실패 입력 자체가 계약 증거이므로 UTF-8·NUL 검사만 하고 성공 입력 parser로 다시 해석하지 않는다. 해당 입력의 거부 여부는 소유 package test가 검증한다.

## CI 승격

PR은 `target`, main은 `full`, 수동 release는 `release` profile로 이 진입점을 한 번 호출한다. Cargo·Schema·MCP matrix·diff 검사는 `project.ps1`이 선택하며 workflow가 같은 명령을 별도로 반복하지 않는다.

Star-Control의 ValidationPlan은 이 추적된 진입점을 단일 명령으로 계획하고, 실제 세부 명령·종료 코드·소요시간·로그는 native report를 증거로 사용한다. partial·unverified·flaky·not_run은 CI 성공이나 cache 재사용 대상으로 승격하지 않는다.
