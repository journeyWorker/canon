---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "코드와 필드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "코드와 시험 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "코드 이름은 결정 대상의 철자 자체다"
  - rule: "prose.length"
    section: "detail"
    reason: "코드 표가 결정의 본체라 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "코드 표가 결정의 본체라 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "코드마다 확인할 사실이 많아 줄이면 확인 항목이 빠진다"
---

# 게이트·승인·검사의 거절을 결과와 가르고 거절은 exit 2로 낸다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 6.4, 7.2.2, 8.1, 8.3, 계약 저자의 1.3.0 최종 규칙 댓글, canon issue #135의 (a)와 (c) 항목

## Intent

canon은 게이트나 승인이나 검사가 명령을 막을 때 종료 값 1로 끝나는 코드를 많이 쓴다. 예를 들어 승인 없는 브리프에 증거를 붙이면 `BRIEF_UNAPPROVED`가 종료 값 1로 온다. 명령은 아무것도 쓰지 않았지만 호출자는 "실행했고 결과가 나쁘다"와 "거절했고 바뀐 것이 없다"를 종료 값으로 구별하지 못한다. 계약 1.3은 일하기 전에 통과해야 하는 게이트, 승인, 검사를 선행 조건으로 보고 종료 값 2와 거절의 조건으로 내게 한다. 검사의 부정적 답이 그 명령의 정상 결과이면 종료 값 1의 `FAILED`이다. 거절은 `data`를 싣지 않는다(spec 6.4).

## Desired outcome

- 아래 표의 코드는 새 이름으로 나오고, 일하기 전에 거절하고 아무것도 쓰지 않은 것은 종료 값 2이다. 종료 값이 1에서 2로 바뀌는 코드는 20개이다.
- ==**종료 값 1은 명령이 실행되어 부정적 결과를 낸 경우에만 쓴다.**==
- 거절 봉투에는 `data`가 없다. 계약 6.4는 모든 minor에서 규범이다. 거절 원인이 여럿이면 `error`가 첫 원인이고 `error.details[]`가 원인마다 자기 코드로 한 항목씩 싣는다.
- 승인과 게이트를 넘는 `--override-reason`이 있는 호출은 지금처럼 넘어가고, `data.overrides[].code`가 새 코드 이름을 말한다.
- 종료 값이 바뀌는 코드의 호출자(훅, 스크립트, 평가, 런처)가 이 변경으로 깨지지 않는다.

## User flow

1. F1 에이전트는 승인 없는 브리프의 시나리오에 증거를 붙인다 → 종료 값 2와 `BRIEF_APPROVAL_REQUIRED`가 오고 아무것도 쓰이지 않는다 → ==**사람에게 승인을 받는다.**==
2. F2 에이전트는 게이트가 막는 상태 전이나 작업 완료를 시도한다 → 종료 값 2와 거절의 코드가 오고 원인이 `details`에 있다 → 원인을 고친다.
3. F3 에이전트는 검사 명령의 결과를 읽는다 → 종료 값 1의 `*_FAILED`이고 `data`에 결과가 있다 → 결과를 읽고 고친다.
4. F4 에이전트는 시작하지 못한 명령을 만난다 → 종료 값 2와 `*_REQUIRED`, `*_UNAVAILABLE`, `*_UNSUPPORTED`가 온다 → 코드의 재시도 값대로 다시 하거나 전제를 만든다.
5. F5 스크립트 작성자는 CHANGELOG의 종료 값 표를 읽는다 → 1에서 2로 바뀐 코드 20개가 있다 → 자기 분기를 고친다.

## Step detail

### F1 선행 조건의 거절

- 표에서 새 종료 값이 2인 코드는 일을 시작하기 전의 거절이다. 구현자가 코드마다 호출 지점을 읽고 명령이 아무것도 쓰지 않았음을 확인한다. 시험은 거절 뒤 작업 트리와 `.canon/`이 그대로임을 확인한다.
- 거절 전에 무언가를 쓴 코드는 종료 값 2로 옮기지 않는다. 그 코드는 `canon-0-36-codes-incomplete`로 넘기고 사람에게 알린다.
- 승인 없음은 `BRIEF_APPROVAL_REQUIRED`, 승인 뒤 브리프가 바뀜은 `BRIEF_APPROVAL_STALE`, 정책이 승인 뒤 바뀜은 `POLICY_APPROVAL_STALE`이다. 사람의 행동은 승인과 재승인이다.
- 증거 기록의 거절은 셋으로 가른다. 메모가 틀리면 `EVIDENCE_NOTE_INVALID`, 작업의 증거 계약과 어긋나면 `EVIDENCE_CONTRACT_CONFLICT`, 보고서가 판정과 모순되면 `EVIDENCE_REPORT_CONFLICT`이다.

### F2 거절의 원인

- 거절 봉투에는 `data`가 없다. 지금 `data.violations`, `data.refused`가 싣던 목록은 `error.details[]`로 간다. `error`는 첫 원인이고, `details`는 원인이 둘 이상일 때 모든 원인을 싣는다.
- `details`의 항목은 오류 모양이다. 자기 `code`, 다음 행동이 다를 때만 `reason`, `message`, 해당하면 `field`를 갖는다. 원인이 하나이면 오류 자체가 그 원인이다.
- ==**게이트가 기록이나 전이를 거절하면 조건마다 다른 코드로 낸다.**== 승인이나 리뷰가 없으면 `<대상>_APPROVAL_REQUIRED`(`PROMOTION_`, `STRATEGY_`, `SUBJECT_`, `DIVERGENCE_`), 근거가 낡았으면 `EVIDENCE_STALE`, 그 밖의 규칙 위반은 `<대상>_INVALID`이다. 대상은 `PROMOTION`, `STRATEGY_PROMOTION`, `DIVERGENCE`, `SUBJECT`이고 구현자가 실제로 내는 것만 등록한다. 계약 저자의 규칙이다.
- 게이트의 11개 위반 종류는 이 조건에 구현자가 나눈다. 시작점은 이렇다. 승인이나 리뷰나 증거가 없으면 REQUIRED(`unreviewed-promotion`, `uncovered-cell`, `unevidenced-flip`), `stale-evidence`는 `EVIDENCE_STALE`, `old-layout`은 `LAYOUT_MIGRATION_REQUIRED`, 나머지는 INVALID이다. 같은 조건 안에서 다음 행동이 다를 때만 이유를 붙이고, 이유는 `canon-0-36-code-reasons`가 정한다.
- `subject move`의 허용되지 않는 전이는 `SUBJECT_CONFLICT`이다. `gate promote`가 거절한 기록은 스테이징에 남는다.
- 증거 배치는 쓰기 잠금을 쥔 채 모든 항목의 게이트를 첫 쓰기 전에 판정한다. 하나라도 거절되면 일이 시작되지 않았으므로 종료 값 2, 그 항목의 코드, `field` `/<index>`, `data` 없음이다. 쓰는 중 거절은 일어나면 안 되고, 일어나면 `BATCH_INCOMPLETE`, 종료 값 1이다. 기록을 쓴 뒤 지우는 지금의 방식은 없어진다.

### F3 결과인 FAILED

- 검사의 부정적 답이 그 명령의 정상 결과이면 종료 값 1이고 결과는 `data`에 있다. `adapter authorize`가 실행을 허가하지 않는 답은 `ADAPTER_AUTHORIZATION_FAILED`이다. `adapter validate`의 부정적 답은 `ADAPTER_VALIDATION_FAILED`이다. `report check`가 보고서를 없음이나 낡음으로 보는 답은 `REPORT_CHECK_FAILED`이고 이유는 `STALE`이다.
- `context-pack verify`의 부정적 답은 `CONTEXT_PACK_VERIFY_FAILED`이다. 깨진 팩을 읽는 다른 명령은 종료 값 2의 `CONTEXT_PACK_CORRUPT`로 거절한다. 이 코드는 `canon-0-36-codes-stored`가 정한다.

### F4 시작하지 못한 명령

- 대시보드 포트가 이미 쓰이면 `DASHBOARD_PORT_UNAVAILABLE`, 앱이 빌드되지 않았으면 `DASHBOARD_UNSUPPORTED`이다. 다른 프로세스가 run을 닫는 중이면 `DISPATCH_RUN_UNAVAILABLE`이다. 앞의 둘 중 포트와 run은 재시도할 수 있다.
- git 신원이 없으면 `CONTEXT_GIT_REQUIRED`, 불변 객체가 다른 바이트를 가지면 `CONTEXT_OBJECT_CONFLICT`, 수집 원본이 틀렸으면 `INGEST_SOURCE_INVALID`이다. 이 셋은 모두 아무것도 쓰기 전에 거절한다.
- 작업 완료(`task done`)가 증거 부족이면 `TASK_EVIDENCE_REQUIRED`, 증거나 타입 원자가 틀리면 `TASK_EVIDENCE_INVALID`이다. 전략 평가가 후보와 맞지 않으면 `STRATEGY_EVALUATION_INVALID`, 전략 승격 게이트가 막으면 `STRATEGY_PROMOTION_INVALID`, 발산 후보가 게이트에 막히면 `DIVERGENCE_INVALID`이다.

### F5 종료 값 표와 호출자

| 지금 | 지금 exit | 새 코드 | 이유 | 새 exit | 재시도 |
|---|---|---|---|---|---|
| `ADAPTER_EXECUTION_NOT_AUTHORIZED` | 1 | `ADAPTER_AUTHORIZATION_FAILED` |  | 1 | 아니오 |
| `ADAPTER_RESPONSE_INVALID` | 1 | `ADAPTER_VALIDATION_FAILED` |  | 1 | 아니오 |
| `BRIEF_CHANGED_SINCE_APPROVAL` | 1 | `BRIEF_APPROVAL_STALE` |  | 2 | 아니오 |
| `BRIEF_UNAPPROVED` | 1 | `BRIEF_APPROVAL_REQUIRED` |  | 2 | 아니오 |
| `CONTEXT_GIT_UNAVAILABLE` | 1 | `CONTEXT_GIT_REQUIRED` |  | 2 | 아니오 |
| `CONTEXT_OBJECT_CONFLICT` | 1 | `CONTEXT_OBJECT_CONFLICT` |  | 2 | 아니오 |
| `CONTEXT_PACK_TAMPERED` | 1 | `CONTEXT_PACK_VERIFY_FAILED` · `CONTEXT_PACK_CORRUPT` |  | 1/2 | 아니오 |
| `DASHBOARD_NOT_BUILT` | 1 | `DASHBOARD_UNSUPPORTED` |  | 2 | 아니오 |
| `DASHBOARD_PORT_IN_USE` | 1 | `DASHBOARD_PORT_UNAVAILABLE` |  | 2 | 예 |
| `DISPATCH_END_IN_PROGRESS` | 1 | `DISPATCH_RUN_UNAVAILABLE` |  | 2 | 예 |
| `DIVERGENCE_REFUSED` | 1 | `DIVERGENCE_APPROVAL_REQUIRED` · `EVIDENCE_STALE` · `DIVERGENCE_INVALID` | 다음 행동이 다른 규칙만 | 2 | 아니오 |
| `EVIDENCE_BATCH_REFUSED` | 1 | 거절한 항목 자신의 코드 |  | 2 | 아니오 |
| `EVIDENCE_CONTRACT_MISMATCH` | 1 | `EVIDENCE_CONTRACT_CONFLICT` |  | 2 | 아니오 |
| `EVIDENCE_FABRICATED` | 1 | `EVIDENCE_NOTE_INVALID` |  | 2 | 아니오 |
| `EVIDENCE_REPORT_CONTRADICTION` | 1 | `EVIDENCE_REPORT_CONFLICT` |  | 2 | 아니오 |
| `GATE_TASK_BLOCKED` | 1 | `TASK_EVIDENCE_REQUIRED` |  | 2 | 아니오 |
| `GATE_TASK_EVIDENCE_INVALID` | 1 | `TASK_EVIDENCE_INVALID` |  | 2 | 아니오 |
| `INGEST_SOURCE_UNUSABLE` | 1 | `INGEST_SOURCE_INVALID` |  | 2 | 아니오 |
| `LEARN_EVALUATION_REJECTED` | 1 | `STRATEGY_EVALUATION_INVALID` |  | 2 | 아니오 |
| `LEARN_PROMOTION_BLOCKED` | 1 | `STRATEGY_APPROVAL_REQUIRED` · `EVIDENCE_STALE` · `STRATEGY_PROMOTION_INVALID` | 다음 행동이 다른 규칙만 | 2 | 아니오 |
| `POLICY_CHANGED_SINCE_APPROVAL` | 1 | `POLICY_APPROVAL_STALE` |  | 2 | 아니오 |
| `PROMOTE_REFUSED` | 1 | `PROMOTION_APPROVAL_REQUIRED` · `EVIDENCE_STALE` · `PROMOTION_INVALID` | 다음 행동이 다른 규칙만 | 2 | 아니오 |
| `REPORT_STALE` | 1 | `REPORT_CHECK_FAILED` | STALE | 1 | 아니오 |
| `SUBJECT_GATE_VIOLATION` | 1 | `SUBJECT_APPROVAL_REQUIRED` · `EVIDENCE_STALE` · `SUBJECT_INVALID` | 다음 행동이 다른 규칙만 | 2 | 아니오 |

- CHANGELOG의 이번 릴리스 항목이 이 표의 20개를 종료 값 1에서 2로 바뀐 코드로 따로 모아 싣는다.
- canon 자신이 종료 값으로 분기하는 곳을 구현자가 모두 찾아 감사표에 적는다. 대상은 `gate install-hooks`가 쓰는 훅, npm 런처, 평가 스크립트, 스킬 안내의 예시이다. 분기가 바뀌어야 하면 고친다.

## Acceptance

- F1:
  - 표의 거절 코드마다 그 거절을 내는 실제 호출이 종료 값 2와 새 코드로 답하고 `data`가 없다. 호출 전후의 작업 트리와 `.canon/`이 같다.
  - 실패 경로: 승인된 브리프에는 같은 호출이 성공한다. 승인 뒤 브리프를 고치면 `BRIEF_APPROVAL_STALE`이다. 거절 전에 쓰는 코드가 있으면 시험이 실패한다.
- F2:
  - 승인 없는 기록과 틀린 기록을 함께 거절하는 `gate promote`가 종료 값 2이고 `error`가 첫 거절의 코드이며 `details` 항목이 2개이다. 각 항목이 자기 코드를 갖고 `data`가 없다. 낡은 근거는 `EVIDENCE_STALE`이다.
  - ==**어떤 종료 값 2 응답도 `data`를 싣지 않는다.**==
  - 증거 배치의 둘째 항목이 게이트에 거절되면 종료 값 2, 그 항목의 코드, `field` `/1`이고 아무것도 쓰이지 않는다.
  - 실패 경로: 게이트가 막은 `subject move`는 상태를 바꾸지 않고, 원인이 하나이면 `details` 없이 오류 자체가 원인이다. 쓰는 중 거절을 일으키는 시험은 `BATCH_INCOMPLETE`, 종료 값 1이다.
- F3:
  - `adapter authorize`, `adapter validate`, `report check`, `context-pack verify`의 부정적 답이 종료 값 1, 새 `*_FAILED` 코드, `data`의 결과이다.
  - 실패 경로: 같은 명령이 입력 파일 없음처럼 시작하지 못하면 종료 값 2이고 `FAILED`가 아니다.
- F4:
  - 시작하지 못하는 다섯 경우가 각각 종료 값 2와 새 코드로 답한다. 다섯은 포트가 쓰이는 대시보드, 빌드되지 않은 대시보드, 닫는 중인 run, git 없는 context-pack, 틀린 수집 원본이다. 재시도 값은 표와 같다.
  - 실패 경로: 닫는 중인 run은 기다린 뒤 다시 하면 성공한다.
- F5:
  - 종료 값 표의 모든 행이 레지스트리와 같다. 종료 값 레지스트리 시험이 새 표로 통과하고, 호출 지점이 종료 값을 덮어쓰면 실패한다.
  - 감사표의 모든 분기가 새 종료 값으로 동작한다. 사람의 훅과 런처 시험이 통과한다.
- 옛 이름 시험이 이 변경의 이름에서 통과하고 `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 계약 저자의 답: 배치의 게이트는 쓰기 잠금을 쥔 채 첫 쓰기 전에 모두 판정한다. 거절은 종료 값 2에 그 항목의 코드와 `field` `/<index>`이고 근거는 계약 8.3이다.
- 계약 저자의 답: 게이트 거절은 조건마다 다른 코드이다. 이유는 같은 조건 안에서 다음 행동이 다를 때만 쓴다. 이전의 코드 하나와 이유 안은 기각되었다.
- 계약 저자의 답: 6.4는 모든 minor에서 규범이다. 원인이 여럿이면 `error.details[]`에 둘 이상 항목을 싣는다.
- 가정: `details`는 모든 원인을 싣고 `error`는 첫 원인이다. 원인이 둘이면 `details`는 둘이다. 스키마가 둘 이상을 요구한다.
- 가정: `ADAPTER_EXECUTION_NOT_AUTHORIZED`는 `adapter authorize`의 정상 결과이므로 종료 값 1의 `ADAPTER_AUTHORIZATION_FAILED`이다. 아무것도 실행되기 전에 거절한다고 보면 종료 값 2의 `ADAPTER_EXECUTION_DENIED`가 맞다. 구현자가 호출 지점을 읽고 정한다.
- 가정: 시나리오 문장 17개를 고치면 그 리뷰가 stale이 된다. 리뷰어가 차이표를 보고 다시 리뷰한다. 이 가정이 맞지 않으면 사람이 정책을 정해야 한다.
- 정책 질문은 이 변경에 새로 없다. 종료 값 1을 기다리는 외부 스크립트가 깨질 수 있다는 점은 업그레이드 노트에 싣는다.

## Out of scope

- 이름만 바뀌는 코드(`canon-0-36-codes-rename`)와 하나의 코드가 가리던 여러 거절(`canon-0-36-codes-refusals`).
- 읽지 못한 파일과 깨진 저장 상태(`canon-0-36-codes-stored`).
- 중간에 멈추는 명령의 코드(`canon-0-36-codes-incomplete`).
- `gate check`, `format`, `brief check`의 종료 값 1 결과. 그 명령들의 정상 결과이다.

Approved: Journey Hwang 2026-11-03T22:21:29Z — "휴가 중 승인 위임(2026-11-01 대화); 코드 개명은 1.3 컨벤션 나오면 한 번에(2026-11-03 사용자 결정), 체커는 atip 레포로(2026-11-03 사용자 결정)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:92d836ee933b572ed76877f883a87769fa655924f3cfe8283dbdab57333d8d6f)
