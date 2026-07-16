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

runner는 이전 보고서나 로그를 읽어 성공을 재사용하지 않는다. input_fingerprint는 revision, dirty patch, profile·unit·BaseRef·선택 파일, Rust/Cargo/Python/PyYAML/Git/PowerShell/platform, lock/config와 실제 command를 묶는다. 매 실행마다 달라지는 artifact ID와 checkout 절대 경로는 정규화하며, fingerprint는 캐시 키 자료일 뿐이다. 조직 전체 캐시는 후속 Star-Control 계층이 소유한다.

YAML은 PyYAML 6.0.3을 사용한다. CI는 hash가 고정된 requirements-validation.txt를 설치한다. 로컬에 정확한 버전이 없으면 자동 설치하지 않고 해당 check를 unverified로 남긴다.

fixtures 또는 examples 아래 invalid 경로의 파일은 실패 입력 자체가 계약 증거이므로 UTF-8·NUL 검사만 하고 성공 입력 parser로 다시 해석하지 않는다. 해당 입력의 거부 여부는 소유 package test가 검증한다.

## Shadow 전환

기존 CI gate는 authority로 유지하고 `invoke-shadow-validation.ps1`이 같은 revision에서 후보 `validate.ps1`을 비차단으로 실행한다. `shadow_compare.py`는 기존 check와 후보 check의 선택 unit, 실제 명령, 결과를 `target/validation-shadow/`에 기록한다. 후보 실패·누락·명령 불일치는 경고와 비교 실패로 남지만 기존 gate 결과를 바꾸지 않는다.

shadow 비교 한 건은 승격 증거가 아니다. 비교 결과는 항상 `promotion_eligible=false`이며, PR·main의 반복 관측에서 누락 검사가 없고 서버의 required check·ruleset 상태까지 별도로 확인된 뒤에만 후속 P-ID에서 후보 gate 승격을 검토한다.
