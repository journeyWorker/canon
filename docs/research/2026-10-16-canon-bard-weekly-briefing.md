# Canon·Bard 주간 리서치 브리핑 — 2026년 10월 16일 (검증·선별)

## 출처와 검증 (2026-10-16)

원문은 ChatGPT 주간 리서치 대화다([공유 링크](https://chatgpt.com/s/t_6ad25ab709888191b07696ee863932c3)). 공유 페이지에는 arXiv 4건의 링크만 걸려 있었고, GitHub Changelog 4건은 링크 없이 제목만 있었다. 그 4건은 제목으로 검색해 원문을 찾았다. 8건 모두 원문을 직접 열어 수치와 주장을 대조했다.

| # | 원문 | 원문에서 확인한 내용 | 브리핑과 다른 점 |
|---|---|---|---|
| 1 | [arXiv:2610.16870](https://arxiv.org/abs/2610.16870) (10월 12일) | 저장소 13개, 작업 291개, 검증 로직 8유형. 에이전트 4종 모두 faithful harness 비율 63% 이하. 병목은 시나리오 탐색이 아니라 명세를 충실히 반영하는 check를 만드는 일. check가 명세 대신 참조 구현을 따라가고, 그래서 명세가 허용하는 구현을 거부한다. | 브리핑은 "참조 구현의 잘못된 동작을 기준으로 테스트를 작성한다"고 했다. 초록의 주장은 그게 아니라 참조 구현을 그대로 따라가서(mirror) 허용되는 대안 구현을 거부한다는 것이다. |
| 2 | [arXiv:2610.17488](https://arxiv.org/abs/2610.17488) (10월 12일, Yu·Cui) | 통제 시나리오 85개, 질의 158개. behavioral fingerprint + historical localization + recursive tracing으로 96.8% exact attribution. 재귀 없는 exhaustive fingerprint matching은 77.2%. 같은 설정으로 추가 질의 34개를 평가했고, approval-boundary·resource-owner 오류로도 확장했다. | "프리프린트에 10월 15일 표기"는 확인하지 못했다. arXiv 제출 기록은 v1 10월 12일 하나뿐이다. |
| 3 | [arXiv:2610.18933](https://arxiv.org/abs/2610.18933) (10월 13일, Lange·Grillotti·Yamada·Charakorn·Ha) | 작업 초안부터 보고서까지 8단계. meta-controller가 병렬 에이전트를 관리하고 단계마다 rubric 검증을 강제한다. co-scientist 모드로 쓴 논문 1편이 ALIFE 2026에 채택됐다. ablation 결론: 신뢰성은 기반 모델만이 아니라 cross-agent orchestration 계층에도 달려 있다. | 없음. 브리핑의 Input/Output contract 표는 원문이 아니라 브리핑의 설계 추론이다. |
| 4 | [arXiv:2610.15765](https://arxiv.org/abs/2610.15765) (10월 11일) | DeskIF-Bench는 데스크톱 앱 6개, 작업 255개. 속성 세 가지: discoverability, granularity, observability. ANIS 확장 평가는 앱 8개, 작업 368개. atomic interface가 native SDK 대비 성공률을 +3.8%p(DeepSeek V4 Flash), +3.5%p(Claude Haiku 4.5) 올리고 온라인 토큰을 약 73% 줄였다. goal-level command는 74.5%, 67.4%까지 올렸다. | 대상은 데스크톱 애플리케이션이다. CLI·도구 API에 그대로 적용된다는 것은 추론이다. |
| 5 | [GitHub Changelog](https://github.blog/changelog/2026-10-13-enterprise-managed-settings-enforcement-updates/) (10월 13일) | `forceRemoteSettingsRefresh: true`이면 Copilot CLI·VS Code·Copilot 앱이 시작할 때 캐시가 아닌 최신 서버 정책을 받아야 한다. 최신 정책을 확인하지 못하면 fail closed. `sandbox` 키에는 최소 클라이언트 버전이 붙는다. | 원문은 "시작 시점"의 최신성이다. "실행 도중 정책이 바뀐 경우", "민감한 도구 호출 직전 재확인"은 브리핑의 추론이다. |
| 6 | [GitHub Changelog](https://github.blog/changelog/2026-10-12-client-telemetry-user-attribution-and-configurable-capture/) (10월 12일) | `telemetry.capture.identity`가 `user.name`, `process.user.name`, `host.name`을 OTel에 추가한다. 내용 수집 범주는 `prompts`, `responses`, `toolArguments`, `toolOutput`, `policyDetail` 다섯 개이고 모두 기본값 off다. managed 값이 로컬 설정, SDK 설정, 환경변수보다 우선한다. 레거시 `captureContent`는 identity를 켜지 않는다. | 없음 |
| 7 | [GitHub Changelog](https://github.blog/changelog/2026-10-15-agentic-autofix-for-code-scanning-alerts-general-availability/) (10월 15일) | 경고를 Copilot에 할당하면 파일 탐색 → 수정안 → CodeQL 재실행으로 검증 → 필요하면 반복 → draft PR. PR에는 수정 요약, 경고가 닫히는 이유, 검증 단계가 들어간다. 경고는 한 번에 최대 25개, 기본 브랜치의 것만 처리한다. 엔터프라이즈 관리자의 "Agentic autofix" 정책이 필요하다. | 없음 |
| 8 | [GitHub Changelog](https://github.blog/changelog/2026-10-14-github-actions-workflow-lockfiles-in-public-preview/) (10월 14일) | `gh actions-lock` 확장이 `.github/workflows/actions.lock`을 만든다. 액션, 전이 의존성, 재사용 워크플로를 커밋에 고정하고, CI에서 lockfile이 최신인지 검사한다. "Require lockfile" 정책과 Dependabot 재생성을 지원한다. 원문: "Locking does not determine whether the dependency itself is safe." | 없음 |

---

## Canon 기준의 재검토

브리핑의 판정은 Canon이 이미 갖춘 것을 고려하지 않았다. 아래는 저장소 코드와 대조한 결과다. 이 저장소에는 Bard가 없으므로 Bard 쪽 제안은 범위 밖이다.

### 1. SpecGuardBench → 부분 채택 (0.23, 리뷰어 단계)

- **이미 있음.** 0.15가 선언된 테스트 결과와 감사 결과를 분리했다. `canon evidence add --kind audit`이 외부 evaluator의 재감사와 반례 보고서를 묶고, `refuted`이면 `false-success`가 ship을 막는다(`crates/canon-model/src/records.rs` `EvidenceAudit`). evidence 작성자와 리뷰어는 다른 actor·session이어야 한다(`review_gate.rs` `distinct_actor`).
- **새 교훈.** 이 논문이 지적하는 실패는 Canon 루프에서도 생긴다. 구현자가 코드와 테스트를 함께 쓰고, 테스트가 시나리오(명세) 대신 구현을 따라가는 경우다. 리뷰어 지침에는 이 실패를 찾으라는 단계가 없다.
- **채택.** 리뷰어 지침에 "check를 check한다" 단계를 넣는다. 테스트의 단언을 시나리오의 Then 단계와 대조하고, 시나리오가 요구하지 않는 구현 세부까지 고정한 check(대안 구현 거부)와 Then 단계를 빠뜨린 check를 finding으로 남긴다.
- **기각.** 브리핑의 6개 필드(`checker_version`, `valid_implementation_acceptance` 등)는 버린다. Canon은 checker를 실행하지 않는다(ARCHITECTURE.md "verification boundary, not an agent executor"). 외부 evaluator의 결과는 이미 `--kind audit`으로 들어온다.

### 2. Origin Attribution → watch

- Canon에는 `Finding.introduced_by`(출처가 확인된 커밋)가 있다. 추측하지 않는 것이 의도된 설계다(`finding.rs` 모듈 문서).
- 논문의 방법은 행동 지문과 재귀 추적이다. 에이전트 간 코드 재사용을 실행하는 런타임(Bard)이나 분석 도구가 할 일이다. Canon이 할 수 있는 일은 그런 도구가 낸 귀속 결과를 기록하는 것 정도이고, 생산자가 아직 없다. #15와 함께 지켜본다.

### 3. The Agentic Scientist → 새 작업 없음

- Canon의 `proposed → specced → building → verifying → shipped` 전이와 guard가 이미 단계별 전이 조건 역할을 한다. 브리핑의 "검증 실패가 다음 단계로 조용히 전파되지 않음"에 해당하는 구체적인 결함은 #70이다. 브리프가 빈 골격인데 `status`가 사람에게 넘기라고 안내한다. 이건 0.22에서 고친다.
- 병렬 에이전트 관리와 meta-controller는 런타임의 일이다(#19, #54).

### 4. A Decade in a Day → 대부분 이미 있음, 빈칸 하나는 0.22에

- discoverability: `canon schema`, 역할별 명령 목록(0.21). granularity: `--from` NDJSON 배치, `--all-scenarios`. observability: 쓰기 명령마다 `meta.next`, `context-pack show|verify`.
- **빈칸.** `canon policy approve`에 대응하는 읽기 명령이 없다. 정책 승인 상태는 `status`와 `gate check`의 advisory 문장으로만 보인다. 0.22에서 `canon policy show`를 추가한다.

### 5. Managed settings 최신성 → 채택 (0.22, P0)

이번 주 항목 중 Canon에서 실제로 재현되는 결함은 이것 하나다.

```text
policy.yaml: require_review 포함 → canon policy approve --by human
subject status checkout --to shipped     → SUBJECT_GATE_VIOLATION (리뷰 없음)  ✓
policy.yaml에서 require_review 삭제 (재승인 없음)
subject status checkout --to shipped     → shipped, exit 0                     ✗
canon gate check                         → clean, exit 0, advisory 한 줄          ✗
```

(0.21.1 debug 빌드로 `conformance/cases/subject-ship-allowed` 사본에서 재현했다.)

에이전트가 승인된 정책을 약하게 고쳐도 ship guard와 CI gate가 통과한다. 남는 것은 advisory 한 줄이다(`policy_approval.rs`: "the advisory never changes an exit code"). 스킬은 "Never edit the policy to make the gate pass"라고 말하지만 강제하지는 않는다. GitHub의 해법과 같은 방향으로 고친다. 승인 이후 정책 bytes가 바뀌었으면(`Changed`), 또는 승인 파일을 읽을 수 없으면(`Unreadable`), 정책으로 판정하는 경로가 fail closed한다. 승인 기록이 아예 없는 저장소(`Unapproved`)는 기준점이 없으므로 지금처럼 advisory로 둔다.

### 6. Client telemetry → 이미 맞춰져 있음, watch

- 세션 ingest는 기본이 metadata-only다(`canon-ingest/src/normalize.rs` `DirectivePrivacyPolicy::METADATA_ONLY`). 사용자 지시문 수집은 `canon.yaml`의 `ingest.sessions.privacy.capture_user_directives`로만 켤 수 있고, 켜면 길이 상한을 반드시 지정해야 하며 redaction을 거친다. CLI 플래그로는 켤 수 없다. 저장소 설정이 로컬 우회보다 우선한다는 GitHub의 원칙과 같다.
- 도구 출력과 응답 원문은 애초에 수집하지 않는다(`UnifiedRow`에는 토큰·비용 필드만 있다).
- **빈칸.** 어떤 수집 정책으로 만들어진 행인지가 행 자체에 기록되지 않는다(cursor identity에만 들어간다). 소비자가 생기면 다룬다.

### 7. Agentic autofix → 채택 (0.23)

- **이미 있음.** `canon finding add|close`, disposition 전이, `resolution_sha`. 열린 blocker finding이 ship을 막는다(`require_review.block_on_findings`).
- **빈칸.** blocker를 `fixed`로 닫으면 재검증 없이 차단이 풀린다. Canon은 `resolution_sha`를 읽지 않는다(의도된 설계). finding 이후에 테스트를 다시 돌렸는지도 보지 않는다. GitHub의 루프는 "수정 → 검출기 재실행 → 사람 리뷰"다. Canon에서 같은 일을 하려면, `fixed`로 닫힌 blocker는 그 change의 시나리오에 finding 이후의 evidence가 생겨야 해소된 것으로 친다. 새 evidence가 기존 리뷰를 stale로 만드는 규칙(0.15 D1)이 이미 있으므로, 재리뷰는 따로 손대지 않아도 따라온다.

### 8. Workflow lockfiles → watch

- Canon의 워크플로는 이미 모든 `uses:`를 커밋 SHA로 고정한다. `scripts/check-release-workflow-safety.py`가 이를 검사한다(fixture: `test-fixtures/release-safety/`).
- lockfile이 더해 주는 것은 composite action 안의 전이 의존성 고정이다. 기능이 public preview이고 `gh` 확장 설치가 필요하므로 GA 후에 다시 본다.

---

## 결정

| 항목 | 판정 | 릴리스 | 이슈 |
|---|---|---|---|
| 5. 정책 최신성: 바뀐 정책은 fail closed, `policy show` | 채택 | 0.22.0 | #71 |
| 3·#70. 브리프가 골격이면 다음 단계는 planner | 채택 | 0.22.0 | #70 |
| 7. fixed blocker는 재검증 evidence가 있어야 해소 | 채택 | 0.23.0 | #72 |
| 1. 리뷰어가 check를 시나리오와 대조 | 채택 | 0.23.0 | #73 |
| 2. 출처 귀속 | watch | — | #74 |
| 6. 수집 정책을 행에 기록 | watch | — | #75 |
| 8. Actions lockfile | watch | — | #76 |
| 4. 에이전트 친화 CLI | 0.22의 `policy show`로 흡수 | 0.22.0 | — |

0.21이 예고한 대로 0.22는 deprecated 명령 형태(`context-pack create`, `prompt register`, `divergence stage`, `demo attest`, `subject status <id> <state>`)도 제거한다(`meta.deprecated.removedIn: "0.22.0"`).

## 한 줄 결론

Canon은 이미 테스트 통과와 감사 결과를 분리하고 있다. 이번 주 자료가 드러낸 실제 구멍은 그 판정의 기준인 정책과 finding의 "고쳤음"을 아무 증거 없이 바꿀 수 있다는 점이다. 0.22와 0.23은 이 두 구멍을 막는다.
