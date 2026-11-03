---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "코드 이름은 결정 대상의 철자 자체다"
  - rule: "prose.length"
    section: "detail"
    reason: "이름 표가 결정의 본체라 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "이름 표가 결정의 본체라 줄이면 결정이 빠진다"
---

# 이름만 바뀌는 오류 코드를 <SUBJECT>_<CONDITION> 형식으로 바꾼다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 7.2, approaches/migration.md §8, canon issue #135의 매핑과 계약 저자의 1.3.0 최종 규칙 댓글

## Intent

canon의 오류 레지스트리는 179개 코드를 갖는다. 대부분이 `FINDING_INVALID_ARGUMENT`, `HUMAN_ONLY`, `NO_COMMIT`처럼 계약 1.3의 이름 규칙(대상 `_` 조건)을 따르지 않는다. 호출자는 모르는 코드의 이름에서 종료 값과 다음 행동을 읽지 못하고 코드마다 레지스트리를 찾아봐야 한다. 검사기의 `L1.code-names`는 1.3 이상을 선언하는 도구에 이 규칙을 요구한다. 이 변경은 뜻과 종료 값과 재시도 값이 그대로인 코드의 이름만 바꾼다. 뜻이 둘 이상이라 가르거나 종료 값이 바뀌는 코드는 다른 변경이 맡는다.

## Desired outcome

- 아래 표의 50개 코드는 새 이름으로만 나온다. 종료 값, 재시도 값, 뜻, 해결 안내는 그대로이다.
- 이름 규칙을 이미 따르는 48개(core 코드 11개와 그 밖의 37개)는 이름이 그대로이고, 시험이 규칙 통과를 확인한다.
- ==**옛 이름은 레지스트리, 소스, 출력, 안내, 시나리오, conformance 어디에도 남지 않는다.**== 별칭은 없다.
- 나머지 81개는 `canon-0-36-codes-exit`, `canon-0-36-codes-refusals`, `canon-0-36-codes-stored`, `canon-0-36-codes-incomplete`가 맡는다. 이 변경은 그 코드가 아직 옛 이름임을 시험에 목록으로 둔다.

## User flow

1. F1 에이전트는 이름만 바뀌는 코드의 실패를 만난다 → 새 코드가 같은 종료 값과 같은 `field`로 온다 → ==**새 이름으로 분기한다.**==
2. F2 에이전트는 레지스트리를 읽는다 → 이 변경의 코드와 유지된 코드가 모두 이름 규칙을 따른다 → 이름의 끝 조건에서 종료 값과 행동을 읽는다.
3. F3 에이전트가 옛 이름을 조회한다 → `canon schema --error <옛 코드> --json`이 모르는 코드로 거절한다 → 새 이름을 CHANGELOG에서 찾는다.
4. F4 사람은 스킬, 웹사이트, 대시보드, 평가 스크립트, 시나리오에서 코드를 읽는다 → 새 이름만 보인다 → 안내대로 따른다.
5. F5 릴리스를 읽는 사람은 CHANGELOG를 본다 → 이 변경의 대체표 행이 있다 → 자기 스크립트의 이름을 바꾼다.

## Step detail

### F1 이름 바꾸기

- 대체표는 지금 코드와 새 코드를 짝짓는다. 이유가 적힌 새 코드는 그 이유와 함께 내고, 이유는 레지스트리에 등록한다.

| 지금 | 새 코드 | 이유 | exit | 재시도 |
|---|---|---|---|---|
| `APPROVAL_INCOMPLETE` | `APPROVAL_INVALID` |  | 2 | 아니오 |
| `BRIEF_CHANGED_SINCE_PAGE_LOAD` | `BRIEF_PAGE_STALE` |  | 2 | 아니오 |
| `BRIEF_HAS_BLOCKERS` | `BRIEF_CHECK_FAILED` | BLOCKERS | 1 | 아니오 |
| `BRIEF_UNAPPROVABLE` | `BRIEF_INVALID` | UNAPPROVABLE | 2 | 아니오 |
| `CHANGE_AMBIGUOUS` | `CHANGE_REQUIRED` |  | 2 | 아니오 |
| `CHANGE_INVALID_ARGUMENT` | `CHANGE_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `CONFIG_MISSING` | `CONFIG_REQUIRED` |  | 2 | 아니오 |
| `CONTEXT_INPUT_MISSING` | `CONTEXT_INPUT_NOT_FOUND` |  | 2 | 아니오 |
| `CONTEXT_SECRET_DETECTED` | `CONTEXT_INPUT_INVALID` | SECRET | 2 | 아니오 |
| `CONTEXT_UNSAFE_PATH` | `CONTEXT_PATH_INVALID` |  | 2 | 아니오 |
| `DASHBOARD_BAD_REQUEST` | `DASHBOARD_REQUEST_INVALID` |  | 2 | 아니오 |
| `DASHBOARD_CONFIG_INVALID` | `CONFIG_INVALID` | DASHBOARD | 2 | 아니오 |
| `DASHBOARD_ORIGIN_REFUSED` | `DASHBOARD_REQUEST_DENIED` |  | 2 | 아니오 |
| `DEMO_NOT_INITIALIZED` | `DEMO_REQUIRED` |  | 2 | 아니오 |
| `DISPATCH_INVALID_ARGUMENT` | `DISPATCH_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `DISPATCH_ROLE_REGIME_MISMATCH` | `DISPATCH_ROLE_CONFLICT` |  | 2 | 아니오 |
| `DIVERGENCE_INVALID_ARGUMENT` | `DIVERGENCE_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `FEATURE_OUTSIDE_SPEC_ROOTS` | `FEATURE_PATH_INVALID` |  | 2 | 아니오 |
| `FINDING_INVALID_ARGUMENT` | `FINDING_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `FINDING_RESOLUTION_REV_UNEXPECTED` | `FINDING_RESOLUTION_REV_CONFLICT` |  | 2 | 아니오 |
| `FINDING_SEQ_OCCUPIED` | `FINDING_EXISTS` |  | 2 | 아니오 |
| `FINDING_STAGED` | `FINDING_CONFLICT` |  | 2 | 아니오 |
| `FINDING_UNKNOWN_SCENARIO` | `SCENARIO_NOT_FOUND` |  | 2 | 아니오 |
| `FORMAT_VIOLATIONS` | `FORMAT_CHECK_FAILED` |  | 1 | 아니오 |
| `GATE_TASK_INVALID_ARGUMENT` | `TASK_ID_INVALID` |  | 2 | 아니오 |
| `GATE_TASK_UNSUPPORTED` | `TASK_UNSUPPORTED` |  | 2 | 아니오 |
| `GATE_VIOLATIONS` | `GATE_CHECK_FAILED` |  | 1 | 아니오 |
| `HUMAN_ONLY` | `ACTOR_DENIED` |  | 2 | 아니오 |
| `INIT_CONFIG_EXISTS` | `CONFIG_EXISTS` |  | 2 | 아니오 |
| `INIT_INVALID_ARGUMENT` | `INIT_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `LEARN_DEMOTION_FAILED` | `STRATEGY_DEMOTION_FAILED` |  | 1 | 아니오 |
| `LEARN_INVALID_ARGUMENT` | `STRATEGY_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `LEARN_INVALID_INPUT` | `STRATEGY_FILE_INVALID` |  | 2 | 아니오 |
| `LEARN_STRATEGY_NOT_FOUND` | `STRATEGY_NOT_FOUND` |  | 2 | 아니오 |
| `LEARN_TRAJECTORY_NOT_FOUND` | `TRAJECTORY_NOT_FOUND` |  | 2 | 아니오 |
| `MIGRATE_UNCOMMITTED` | `MIGRATION_CONFLICT` |  | 2 | 아니오 |
| `NO_COMMIT` | `COMMIT_REQUIRED` |  | 2 | 아니오 |
| `PLAN_SOURCES_MISSING` | `PLAN_SOURCES_REQUIRED` |  | 2 | 아니오 |
| `QUERY_INVALID_ARGUMENT` | `QUERY_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `RELAY_DIGEST_MISMATCH` | `RELAY_DIGEST_STALE` |  | 2 | 아니오 |
| `RETRIEVE_FAILED` | `RETRIEVE_INVALID` |  | 2 | 아니오 |
| `REVIEW_INVALID_ARGUMENT` | `REVIEW_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `SCAFFOLD_INVALID_ARGUMENT` | `SCAFFOLD_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `SEARCH_QUERY_EMPTY` | `SEARCH_QUERY_REQUIRED` |  | 2 | 아니오 |
| `SKILLS_DRIFT` | `SKILLS_CHECK_FAILED` | DRIFT | 1 | 아니오 |
| `SPEC_ROOT_UNRESOLVED` | `SPEC_ROOT_REQUIRED` |  | 2 | 아니오 |
| `SUBJECT_INVALID_ARGUMENT` | `SUBJECT_ARGUMENT_INVALID` |  | 2 | 아니오 |
| `SUBJECT_INVALID_TRANSITION` | `SUBJECT_CONFLICT` | TRANSITION | 2 | 아니오 |
| `UNKNOWN_ROLE` | `ROLE_INVALID` |  | 2 | 아니오 |
| `UNKNOWN_SPEC_ROOT` | `SPEC_ROOT_NOT_FOUND` |  | 2 | 아니오 |

- ==**한 코드의 모든 방출 지점이 새 이름을 쓴다.**== 호출 지점이 종료 값을 덮어쓰지 않는 0.33의 규칙은 그대로이다.
- 대상 이름은 한 도구 안에서 같은 철자이다. `GATE_TASK_*`는 `TASK_*`로, `LEARN_*`는 `STRATEGY_*`로 모은다. 명령 이름 `strategy`는 바뀌지 않는다.
- 옛 코드 둘 이상이 새 코드 하나로 모이면 상황을 이유로 구별한다. 표에서는 `SUBJECT_INVALID_TRANSITION`이 `SUBJECT_CONFLICT`의 이유 `TRANSITION`이 된다.
- 새 이름은 canon이 전에 다른 뜻으로 쓴 이름이면 안 된다. 계약 15.3이 그렇게 정한다. 구현자가 CHANGELOG의 이전 대체표와 `removed` 목록을 읽고 확인한다. 겹치면 이름을 바꾸고 사람에게 알린다.
- 경고 코드는 바뀌지 않는다. `CONFIG_MISSING` 경고는 이름이 그대로이고 오류만 `CONFIG_REQUIRED`가 된다.

### F2 시험

- 레지스트리 시험이 이 변경의 코드와 유지된 48개 각각에 `atip::condition::code_name_problem`을 돌려 통과를 확인한다. core 코드는 규칙 밖이다.
- 아직 옛 이름인 81개는 시험 안의 목록으로 둔다. 목록의 코드가 규칙을 통과하면 시험이 실패한다. 뒤의 변경이 자기 코드를 목록에서 빼고, 마지막 변경 `canon-0-36-codes-table`이 목록이 비었음을 확인한다.
- 옛 이름 모음 `OLD_CODES`에 이 변경이 없앤 이름을 더한다. 안내, 웹사이트, 대시보드 소스, 평가 스크립트, 설계 문서에서 옛 이름을 찾는 시험이 이 이름들을 본다.

### F3 옛 이름의 조회

- 옛 이름은 `canon schema --error <코드> --json`에서 `USAGE`, `field` `--error`로 거절한다. 다른 모르는 코드와 같은 답이다. 별칭과 "이름이 바뀌었다"는 안내 코드는 없다.

### F4 소비자

- 코드를 읽는 모든 곳이 새 이름으로 간다. 대상은 스킬 안내와 참조 문서, 웹사이트의 영어와 한국어 쪽, 대시보드의 소스와 시험, 평가 스크립트와 시험 자료이다. conformance 케이스, 시나리오의 문장, 소스의 힌트 문장도 포함한다.
- 시나리오 문장이 바뀌면 그 시나리오의 요약 해시가 바뀐다. 이 변경이 바꾸는 시나리오는 58개이다. 각 시나리오의 리뷰는 stale이 되고, 다른 행위자가 다시 리뷰한다.
- conformance 기대 출력은 `just conformance`로 다시 만들고 차이를 눈으로 확인한 뒤 커밋한다.
- 대시보드 빌드 산출물은 추적되지 않으므로 소스만 고치고 `just dashboard`가 통과함을 확인한다.

### F5 변경 기록

- CHANGELOG의 이번 릴리스 항목에 이 변경의 대체표 행을 싣는다. 행은 옛 이름, 새 이름, 이유를 갖는다. 마지막 변경이 전체 표와 업그레이드 노트로 묶는다.

## Acceptance

- F1:
  - 표의 코드마다 그 코드를 내던 호출이 새 코드로 답한다. 종료 값과 `retryable`과 `field`가 이전과 같다. 시험은 코드마다 실제 호출로 증명한다. 레지스트리 행만 읽는 시험은 증명이 아니다.
  - ==**표의 옛 이름은 `canon schema --errors --json`에도, 어떤 명령의 `errors`에도, 어떤 실패 봉투에도 없다.**==
  - 실패 경로: 호출 지점이 옛 이름을 문자열로 만들어 내면 레지스트리 일치 시험이 실패한다. 한 코드의 뜻이나 종료 값이 바뀌어 있으면 그 시험이 실패한다.
- F2:
  - 이 변경의 50개와 유지된 48개가 모두 규칙을 통과한다. core 코드 11개의 이름은 그대로이다.
  - 실패 경로: 옛 이름 목록의 코드가 규칙을 통과하면 시험이 실패한다. 규칙에 어긋난 새 이름(조건으로 끝나지 않는 이름)을 레지스트리에 넣으면 시험이 실패한다.
- F3:
  - 옛 이름마다 `schema --error`가 `USAGE`와 `field` `--error`로 거절한다.
- F4:
  - 옛 이름 시험이 모든 소비자에서 통과한다. 옛 이름 하나를 안내에 되살리면 실패한다.
  - 변경된 시나리오 58개의 다시 한 리뷰가 gate에서 stale이 아니다. `just conformance`의 차이가 이름 바꾸기뿐이다.
  - 실패 경로: 소비자가 존재하지 않는 코드를 말하면(오타 포함) 시험이 실패한다.
- F5:
  - CHANGELOG 항목이 표의 모든 행을 가진다. 표에 없는 바뀐 코드가 있으면 시험이 실패한다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: 표는 issue #135의 매핑과 계약 저자의 1.3.0 최종 규칙에서 나왔다. 호출 지점을 읽은 구현자가 한 코드에 상황이 둘 이상임을 확인하면 그 코드를 다른 변경(`codes-refusals`, `codes-stored`)으로 옮기고 사람에게 알린다.
- 가정: `CONTEXT_UNSAFE_PATH`는 호출자가 값을 고칠 수 있으므로 `CONTEXT_PATH_INVALID`로 둔다. 정책이 막는 경로라면 `DENIED`가 맞다. 사람이 다르게 보면 알려 달라.
- 가정: `BRIEF_HAS_BLOCKERS`, `FORMAT_VIOLATIONS`, `GATE_VIOLATIONS`, `SKILLS_DRIFT`, `REPORT_STALE`처럼 검사의 부정적 답은 종료 값 1의 `FAILED` 조건으로 둔다. 검사 자신의 결과이기 때문이다(계약 7.2.2).
- 정책 질문: 시나리오 문장 58개를 고치면 그 리뷰가 stale이 된다. 이름만 바뀐 시나리오에 대해 리뷰어가 차이표를 보고 다시 리뷰하는 방식을 가정한다. 이 가정이 맞지 않으면 사람이 정책을 정해야 한다.
- 이 변경이 쓰는 이름은 모두 계약 저자의 규칙으로 정해졌다. 저자에게 물은 질문은 이 변경에 없다.

## Out of scope

- 종료 값이나 재시도 값이 바뀌는 코드(`canon-0-36-codes-exit`).
- 뜻이 둘 이상이라 가르는 코드와 읽지 못한 파일(`canon-0-36-codes-refusals`, `canon-0-36-codes-stored`).
- 중간에 멈추는 코드와 `STOPPED_MIDWAY`(`canon-0-36-codes-incomplete`).
- 이유 이름의 정리(`canon-0-36-code-reasons`)와 전체 대체표(`canon-0-36-codes-table`).
- 경고 코드의 이름 바꾸기.

Approved: Journey Hwang 2026-11-03T22:21:29Z — "휴가 중 승인 위임(2026-11-01 대화); 코드 개명은 1.3 컨벤션 나오면 한 번에(2026-11-03 사용자 결정), 체커는 atip 레포로(2026-11-03 사용자 결정)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:6db223ece968cd12fd7426b223b6d10b6fc126312df2bd59ac0ffae2e4437e71)
