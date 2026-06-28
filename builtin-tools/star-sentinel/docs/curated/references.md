> 흡수 출처: `custom_dev_verification_platform_design_v4_curated/references.md`
> 정리 상태: Star Sentinel 정규 명칭으로 변환해 흡수한 상세 설계 문서.

# references

## SARIF 2.1.0

- 조직/출처: OASIS
- 설계 반영: 정적분석 결과 교환 포맷. 자체 diagnostic/report 모델의 구조 참고.
- URL: https://www.oasis-open.org/standard/sarif-v2-1-0/

## CodeQL

- 조직/출처: GitHub
- 설계 반영: 코드를 DB처럼 모델링하고 AST/CFG/DFG를 질의하는 구조 참고. 도구 자체를 붙이지 않고 분석 DB 개념만 참조.
- URL: https://codeql.github.com/docs/codeql-overview/about-codeql/

## Tree-sitter

- 조직/출처: Tree-sitter
- 설계 반영: 증분 파싱/언어 어댑터 구조 참고. 초기는 경량 파서, 장기는 언어별 어댑터.
- URL: https://tree-sitter.github.io/

## Semgrep rules

- 조직/출처: Semgrep
- 설계 반영: 패턴 룰 + 메타변수 + dataflow 룰 작성 방식 참고. 자체 룰 DSL 설계에 반영.
- URL: https://docs.semgrep.dev/writing-rules/overview

## OPA

- 조직/출처: Open Policy Agent
- 설계 반영: Policy-as-Code와 중앙 의사결정 엔진 개념 참고.
- URL: https://openpolicyagent.org/docs

## Cedar

- 조직/출처: Cedar Policy
- 설계 반영: RBAC/ABAC 권한 정책 언어 구조 참고. agent/tool capability 정책 설계에 반영.
- URL: https://cedarpolicy.com/

## NIST SSDF SP 800-218

- 조직/출처: NIST
- 설계 반영: 보안 소프트웨어 개발 실천 항목. secure development 요구사항의 상위 기준.
- URL: https://csrc.nist.gov/pubs/sp/800/218/final

## OWASP ASVS

- 조직/출처: OWASP
- 설계 반영: 웹/서버 보안 요구사항 분류 기준. 모든 보안 rule을 다 구현하지 않고 위험경로 중심으로 선별.
- URL: https://owasp.org/www-project-application-security-verification-standard/

## OWASP Top 10 2025

- 조직/출처: OWASP
- 설계 반영: 웹 애플리케이션 주요 보안 위험 분류.
- URL: https://owasp.org/www-project-top-ten/

## OWASP LLM Top 10 2025

- 조직/출처: OWASP GenAI Security
- 설계 반영: AI/RAG/tool 호출 계층의 prompt injection, excessive agency, sensitive disclosure 위험 분류.
- URL: https://genai.owasp.org/llm-top-10/

## CycloneDX

- 조직/출처: OWASP/Ecma
- 설계 반영: SBOM/BOM 객체 모델. 자체 SBOM-lite 설계 참고.
- URL: https://cyclonedx.org/specification/overview/

## SPDX

- 조직/출처: Linux Foundation
- 설계 반영: SBOM, 라이선스, AI/data/security reference 표현 참고.
- URL: https://spdx.dev/

## OSV Schema

- 조직/출처: OpenSSF
- 설계 반영: 취약점이 패키지 버전/커밋에 매핑되는 스키마. 자체 vulnerability matcher 데이터 모델 참고.
- URL: https://github.com/ossf/osv-schema

## OpenVEX

- 조직/출처: OpenSSF
- 설계 반영: 취약점 영향 여부/VEX 상태 표현. 장기 공급망 검증의 조건부 기능.
- URL: https://openssf.org/projects/openvex/

## SLSA v1.2

- 조직/출처: SLSA
- 설계 반영: 공급망 수준, provenance, attestation 참고. 개인 개발자는 release integrity subset만 우선.
- URL: https://slsa.dev/spec/v1.2/

## in-toto

- 조직/출처: in-toto
- 설계 반영: 빌드/배포 단계가 계획대로 수행됐는지 metadata로 검증하는 구조 참고. 전체 도입은 공개 배포 시점.
- URL: https://in-toto.io/

## TUF

- 조직/출처: The Update Framework
- 설계 반영: 업데이트 채널 보호. 자동 업데이트 기능이 생길 때 조건부 도입.
- URL: https://theupdateframework.io/

## OpenTelemetry

- 조직/출처: CNCF
- 설계 반영: trace/metric/log와 context propagation 개념. run/task/validation ledger와 연결.
- URL: https://opentelemetry.io/docs/

## Stryker Mutation Testing

- 조직/출처: Stryker
- 설계 반영: 테스트가 결함을 실제로 잡는지 확인하는 mutation testing 개념.
- URL: https://stryker-mutator.io/docs/

## libFuzzer

- 조직/출처: LLVM
- 설계 반영: coverage-guided fuzzing. parser/serializer/compiler/format 처리에서 장기 필요.
- URL: https://llvm.org/docs/LibFuzzer.html

## WCAG 2.2

- 조직/출처: W3C
- 설계 반영: GUI/Web을 만들 때 접근성 검증 기준. 모든 프로젝트의 필수는 아님.
- URL: https://www.w3.org/WAI/standards-guidelines/wcag/
