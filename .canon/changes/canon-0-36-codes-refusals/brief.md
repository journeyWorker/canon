---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "코드 이름은 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "코드 이름은 결정 대상의 철자 자체다"
  - rule: "prose.length"
    section: "detail"
    reason: "코드 표가 결정의 본체라 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "코드마다 확인할 사실이 많아 줄이면 확인 항목이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "코드 표가 결정의 본체라 줄이면 결정이 빠진다"
---

# 여러 거절을 가리던 *_REFUSED와 *_INVALID_ARGUMENT를 조건별 코드로 가른다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 7.2.2, 7.6, 계약 저자의 1.3.0 최종 규칙 댓글, canon issue #135의 (b) 항목과 3절

## Intent

`EVIDENCE_REFUSED`, `MIGRATE_REFUSED`, `SUBJECT_ADOPT_REFUSED` 같은 `*_REFUSED` 코드와 `EVIDENCE_INVALID_ARGUMENT`는 호출자가 해야 할 일이 다른 거절 여러 개를 한 이름으로 낸다. `MIGRATE_REFUSED` 하나가 설정이 틀린 경우, 작업 트리에 커밋 안 된 변경이 있는 경우, 임시 디렉터리를 쓸 수 없는 경우를 모두 말한다. 호출자는 메시지를 읽고 가를 수밖에 없다. 계약 1.3은 조건이 다르면 코드를 달리하고, 같은 조건 안에서 다음 행동만 다르면 이유로 가르게 한다. 이 변경은 이 계열의 28개 코드를 호출 지점마다 조건으로 가른다.

## Desired outcome

- 아래 표의 28개 코드는 새 코드로만 나온다. 한 호출 지점은 거절의 원인에 맞는 조건 하나의 코드를 낸다.
- ==**거절의 코드는 호출자가 해야 할 일을 정한다.**== 값을 고치면 `INVALID`, 채우면 `REQUIRED`, 상태를 읽으면 `CONFLICT`, 사람에게 넘기면 `DENIED`, 이름을 확인하면 `NOT_FOUND`, 나중에 다시 하면 `UNAVAILABLE`이다.
- 같은 조건 안에서 다음 행동이 다른 상황은 이유로 가르고, 이유는 레지스트리에 등록한다.
- 거절 원인이 여럿인 호출은 첫 원인의 코드와 `details`로 답한다. 거절은 `data`를 싣지 않는다.
- 이 변경의 코드는 모두 종료 값 2이고 아무것도 쓰지 않는다. 이전과 달리 종료 값이 바뀌는 것은 `LEARN_ROLLBACK_FAILED`의 서명 거절뿐이다.

## User flow

1. F1 에이전트는 증거나 승인의 인자를 빠뜨리거나 어긋나게 준다 → 인자 조건에 맞는 `INVALID`, `REQUIRED`, `CONFLICT` 코드가 온다 → ==**코드의 조건대로 인자를 고치거나 채우거나 하나를 뺀다.**==
2. F2 에이전트는 대상의 상태가 막는 명령을 한다 → `CONFLICT`, `DENIED`, `NOT_FOUND` 코드와 이유가 온다 → 상태를 읽고 요청을 바꾸거나 사람에게 넘긴다.
3. F3 에이전트는 설정이나 환경이 막는 명령을 시작한다 → `CONFIG_INVALID`와 이유, `*_REQUIRED`, `*_DENIED`, `*_UNAVAILABLE`이 온다 → 설정을 고치거나 만들거나 나중에 다시 한다.
4. F4 에이전트는 이름이나 색인이 없는 명령을 한다 → `BRIEF_REQUIRED`, `SCENARIO_INDEX_REQUIRED`, `*_MIGRATION_REQUIRED`가 온다 → 안내된 `canon migrate`나 `canon inventory sync`를 한다.
5. F5 에이전트는 여러 id를 한 번에 승인하려 한다 → 첫 거절 id의 코드와 `details`가 온다 → 거절된 id를 하나씩 고친다.
6. F6 사람은 CHANGELOG의 행과 안내를 읽는다 → 옛 `*_REFUSED`가 어느 코드로 갈라졌는지 보인다 → 스크립트를 고친다.

## Step detail

### F1 인자와 증거의 규칙

- ==**인자 제약이 만드는 거절은 제약의 규칙을 따른다.**== `requires`와 `atLeastOne`은 `REQUIRED`, `forbids`와 `atMostOne`은 `CONFLICT`, 값이 틀리거나 비면 `INVALID`이다. `EVIDENCE_INVALID_ARGUMENT`가 이렇게 셋으로 갈라진다.
- `EVIDENCE_REFUSED`는 `task`나 `scenario`가 하나도 없으면 `EVIDENCE_REQUIRED`이다. 증거 규칙이 기록을 막으면 `EVIDENCE_INVALID`나 `EVIDENCE_CONFLICT`이다.
- 증거의 사람 승인이 없으면 `EVIDENCE_APPROVAL_REQUIRED`이다. 있는데 틀렸거나 서명이 맞지 않으면 `EVIDENCE_APPROVAL_INVALID`이고 이유 `MALFORMED`, `SIGNATURE_MISMATCH`는 그대로이다.
- 묶으려는 파일이 앞서 본 뒤에 바뀌었으면 `*_BINDING_STALE`이다. 저장소 밖이면 `*_DENIED`, 없으면 `*_NOT_FOUND`, 형식이 틀리면 `*_INVALID`이다. 리뷰의 산출물 묶음(`REVIEW_ARTIFACT_*`)도 같다.
- `policy approve`는 정책 파일이 없으면 `POLICY_REQUIRED`, 읽을 수 없거나 틀리면 `POLICY_INVALID`, 승인자가 비면 `APPROVAL_INVALID`이다.

### F2 대상의 상태

- 발산 승인은 승인자가 리뷰어가 아니거나 스스로 증명한 증거이면 `DIVERGENCE_DENIED`(이유 `NOT_REVIEWER`, `SELF_ATTESTED`), 증거가 발산이 아니면 `DIVERGENCE_CONFLICT`(이유 `NOT_DIVERGENT`)이다. 해결할 사람이 다르다.
- 이미 끝난 dispatch run과 진행 중이 아닌 run은 `DISPATCH_CONFLICT`의 이유 `ALREADY_ENDED`와 `NOT_RUNNING`이다. 끝난 run을 다시 끝내는 호출은 지금처럼 거절한다.
- 채택과 채택 취소의 거절은 둘 다 `SUBJECT_CONFLICT`이고 원인마다 이유가 있다. 채택 취소가 승인된 브리프나 `@flow` 태그 시나리오 때문에 거절되면 이유로 구별한다. 지금 `subject unadopt`가 `SUBJECT_ADOPT_REFUSED`를 내는 곳이 있으면 고친다.
- `trace`가 앞자리 sha로 커밋 여럿을 만나면 `TRACE_CONFLICT`이고 `expected`에 후보를 싣는다. 아무 기록도 없으면 `TRACE_NOT_FOUND`이다.
- 전략 롤백은 서명이 맞지 않으면 종료 값 2의 `STRATEGY_SIGNATURE_INVALID`이고 아무것도 기록하지 않는다. 롤백을 기록하지 못하면 종료 값 1의 `STRATEGY_ROLLBACK_FAILED`이다.

### F3 설정과 환경

- 설정 파일의 한 절이 틀린 경우는 모두 `CONFIG_INVALID`이고 이유가 파일이나 절을 가리킨다: `POLICY`, `SPECS`, `SKILLS`. 어댑터 정책, `specs:` 절, `skills:` 절이 해당한다. 이유 목록의 완성은 `canon-0-36-code-reasons`가 한다.
- `inventory sync`가 색인 잠금을 다른 프로세스가 쥐어 시작하지 못하면 `INVENTORY_INDEX_UNAVAILABLE`이고 다시 할 수 있다. `plugin sync`는 플러그인이 설치되지 않았거나 오버레이 원본이 없으면 `PLUGIN_REQUIRED`이다.
- `migrate`는 옮길 수 없는 설정이면 `CONFIG_INVALID`, 지원하지 않는 배치이면 `MIGRATION_INVALID`, 임시 디렉터리를 쓸 수 없으면 `MIGRATION_DENIED`이다. 커밋 안 된 변경은 `MIGRATION_CONFLICT`이다(`canon-0-36-codes-rename`).
- `adapter authorize`는 응답 파일이 없으면 `ADAPTER_FILE_NOT_FOUND`, 읽을 권한이 없으면 `ADAPTER_FILE_DENIED`, 규칙을 어기거나 해석할 수 없으면 `ADAPTER_FILE_INVALID`이다.
- `artifacts move`는 대상이 s3 단계가 아니면 `ARTIFACTS_TIER_INVALID`이고, 로컬 블롭이 이름과 다르거나 객체 저장소의 사본이 나쁘면 `ARTIFACTS_BLOB_CORRUPT`이다. 도구가 쓴 상태가 깨진 경우이다.
- `gate install-hooks`는 `--repo`가 디렉터리가 아니면 `HOOKS_REPO_INVALID`, 훅 파일이 충돌하면 `HOOKS_FILE_CONFLICT`, 읽을 수 없으면 `HOOKS_FILE_DENIED`이다.
- 보존 내보내기는 민감 기록 포함이 허용되지 않으면 `RETENTION_EXPORT_DENIED`이다. 삭제와 내보내기가 단계를 열거나 읽지 못하면 `RETENTION_TIERS_UNAVAILABLE`(다시 할 수 있음)이나 `RETENTION_TIERS_INVALID`이다. 미리보기만 한 호출은 종료 값 0이므로 `RETENTION_PURGE_REFUSED`의 뜻에서 그 문구를 없앤다.
- `skills`는 제공자를 모르면 `SKILLS_ARGUMENT_INVALID`, 지정한 원본 디렉터리가 없으면 `SKILLS_SOURCE_NOT_FOUND`이다.

### F4 이름과 색인

- 살아 있는 변경 중 브리프를 가진 것이 없으면 `BRIEF_REQUIRED`이다. 호출자가 이름을 준 변경에 브리프가 없으면 `BRIEF_NOT_FOUND`이다.
- 옛 형식의 브리프는 `BRIEF_MIGRATION_REQUIRED`, 0.29의 장부 배치는 `LAYOUT_MIGRATION_REQUIRED`이다. 둘 다 `canon migrate`로 풀린다. `OLD_LAYOUT`은 이미 있는 `LAYOUT_MIGRATION_REQUIRED`와 합친다.
- 증거나 리뷰가 시나리오 색인에 없는 시나리오를 가리키면 `SCENARIO_INDEX_REQUIRED`이다. 이름이 맞고 색인이 비어 있는 경우이다.

### F5 여러 원인

- `brief approve`가 여러 id를 거절하면 첫 거절 id 자신의 코드(`BRIEF_NOT_FOUND`, 이유가 `BLOCKERS`나 `UNAPPROVABLE`인 `BRIEF_INVALID`)로 답하고, 거절된 id마다 `details` 항목이 있다. `data.refused`는 없앤다. 아무것도 쓰지 않는다.

### F6 표와 소비자

| 지금 | 지금 exit | 새 코드 | 이유 | 새 exit | 재시도 |
|---|---|---|---|---|---|
| `ADAPTER_REFUSED` | 2 | `ADAPTER_FILE_NOT_FOUND` · `ADAPTER_FILE_DENIED` · `ADAPTER_FILE_INVALID` · `CONFIG_INVALID` | POLICY | 2 | 아니오 |
| `ARTIFACTS_MOVE_REFUSED` | 2 | `ARTIFACTS_TIER_INVALID` · `ARTIFACTS_BLOB_CORRUPT` |  | 2 | 아니오 |
| `BRIEF_APPROVE_REFUSED` | 2 | 거절된 첫 id 자신의 코드와 `details` |  | 2 | 아니오 |
| `DISPATCH_ALREADY_ENDED` | 2 | `DISPATCH_CONFLICT` | ALREADY_ENDED | 2 | 아니오 |
| `DISPATCH_NOT_RUNNING` | 2 | `DISPATCH_CONFLICT` | NOT_RUNNING | 2 | 아니오 |
| `DIVERGENCE_ACCEPT_REFUSED` | 2 | `DIVERGENCE_DENIED` · `DIVERGENCE_CONFLICT` |  | 2 | 아니오 |
| `EVIDENCE_APPROVAL_INVALID` | 2 | `EVIDENCE_APPROVAL_INVALID` · `EVIDENCE_APPROVAL_REQUIRED` |  | 2 | 아니오 |
| `EVIDENCE_BIND_REFUSED` | 2 | `EVIDENCE_BINDING_STALE` · `EVIDENCE_BINDING_INVALID` · `EVIDENCE_BINDING_NOT_FOUND` · `EVIDENCE_BINDING_DENIED` |  | 2 | 아니오 |
| `EVIDENCE_INVALID_ARGUMENT` | 2 | `EVIDENCE_ARGUMENT_INVALID` · `EVIDENCE_ARGUMENT_REQUIRED` · `EVIDENCE_ARGUMENT_CONFLICT` |  | 2 | 아니오 |
| `EVIDENCE_REFUSED` | 2 | `EVIDENCE_REQUIRED` · `EVIDENCE_INVALID` · `EVIDENCE_CONFLICT` |  | 2 | 아니오 |
| `EVIDENCE_SCENARIO_NOT_INDEXED` | 2 | `SCENARIO_INDEX_REQUIRED` |  | 2 | 아니오 |
| `HOOKS_INSTALL_REFUSED` | 2 | `HOOKS_REPO_INVALID` · `HOOKS_FILE_CONFLICT` · `HOOKS_FILE_DENIED` |  | 2 | 아니오 |
| `INVENTORY_REFUSED` | 2 | `CONFIG_INVALID` · `INVENTORY_INDEX_UNAVAILABLE` | SPECS | 2 | 아니오/예 |
| `LEARN_ROLLBACK_FAILED` | 1 | `STRATEGY_SIGNATURE_INVALID` · `STRATEGY_ROLLBACK_FAILED` |  | 2/1 | 아니오 |
| `MIGRATE_REFUSED` | 2 | `MIGRATION_INVALID` · `MIGRATION_CONFLICT` · `MIGRATION_DENIED` · `CONFIG_INVALID` |  | 2 | 아니오 |
| `NO_BRIEF` | 2 | `BRIEF_REQUIRED` |  | 2 | 아니오 |
| `OLD_BRIEF_FORMAT` | 2 | `BRIEF_MIGRATION_REQUIRED` |  | 2 | 아니오 |
| `OLD_LAYOUT` | 2 | `LAYOUT_MIGRATION_REQUIRED` |  | 2 | 아니오 |
| `PLUGIN_SYNC_FAILED` | 2 | `CONFIG_INVALID` · `PLUGIN_REQUIRED` | SPECS | 2 | 아니오 |
| `POLICY_APPROVAL_REFUSED` | 2 | `POLICY_REQUIRED` · `POLICY_INVALID` · `APPROVAL_INVALID` |  | 2 | 아니오 |
| `RETENTION_EXPORT_REFUSED` | 2 | `RETENTION_EXPORT_DENIED` · `RETENTION_TIERS_UNAVAILABLE` |  | 2 | 아니오/예 |
| `RETENTION_PURGE_REFUSED` | 2 | `RETENTION_TIERS_UNAVAILABLE` · `RETENTION_TIERS_INVALID` |  | 2 | 아니오/예 |
| `REVIEW_ARTIFACT_REFUSED` | 2 | `REVIEW_ARTIFACT_STALE` · `REVIEW_ARTIFACT_INVALID` · `REVIEW_ARTIFACT_NOT_FOUND` · `REVIEW_ARTIFACT_DENIED` |  | 2 | 아니오 |
| `REVIEW_SCENARIO_NOT_INDEXED` | 2 | `SCENARIO_INDEX_REQUIRED` |  | 2 | 아니오 |
| `SKILLS_INVALID_ARGUMENT` | 2 | `SKILLS_ARGUMENT_INVALID` · `SKILLS_SOURCE_NOT_FOUND` · `CONFIG_INVALID` | SKILLS | 2 | 아니오 |
| `SUBJECT_ADOPT_REFUSED` | 2 | `SUBJECT_CONFLICT` | 원인마다 | 2 | 아니오 |
| `SUBJECT_UNADOPT_REFUSED` | 2 | `SUBJECT_CONFLICT` | 원인마다 | 2 | 아니오 |
| `TRACE_NOT_FOUND` | 2 | `TRACE_NOT_FOUND` · `TRACE_CONFLICT` |  | 2 | 아니오 |

- 소비자 이동과 시나리오 문장, conformance, CHANGELOG 행은 `canon-0-36-codes-rename`과 같은 규칙으로 한다. 이 변경이 바꾸는 시나리오는 37개이다.
- 옛 이름 모음에 이 변경이 없앤 이름을 더하고, 아직 옛 이름인 코드의 목록에서 이 변경의 코드를 뺀다.

## Acceptance

- F1:
  - 증거 호출에서 `--task`와 `--scenario`를 모두 빠뜨리면 종료 값 2의 `EVIDENCE_REQUIRED`이다. 서로 어긋나는 두 인자는 `EVIDENCE_ARGUMENT_CONFLICT`, 빈 값은 `EVIDENCE_ARGUMENT_INVALID`이다.
  - 실패 경로: 묶은 파일을 바꾼 뒤 다시 묶으면 `EVIDENCE_BINDING_STALE`이다. 저장소 밖 파일은 `EVIDENCE_BINDING_DENIED`이다. 두 경우 모두 아무것도 쓰지 않는다.
- F2:
  - ==**끝난 run을 다시 끝내면 종료 값 2의 `DISPATCH_CONFLICT`와 이유 `ALREADY_ENDED`이고 매니페스트가 그대로이다.**== 시작한 적 없는 run은 이유 `NOT_RUNNING`이다.
  - 실패 경로: 앞자리 sha가 커밋 둘에 맞으면 `TRACE_CONFLICT`이고 `expected`가 둘을 싣는다. 서명이 틀린 롤백은 `STRATEGY_SIGNATURE_INVALID`이고 아무것도 기록하지 않는다.
- F3:
  - `specs:` 절이 틀린 저장소에서 `inventory sync`가 `CONFIG_INVALID`와 이유 `SPECS`로 거절한다. 색인 잠금을 쥔 채 호출하면 `INVENTORY_INDEX_UNAVAILABLE`이다.
  - 실패 경로: 읽기 권한이 없는 응답 파일은 `ADAPTER_FILE_DENIED`이고 없는 파일은 `ADAPTER_FILE_NOT_FOUND`이다. 권한 시험은 비루트로 돌고 루트면 건너뛰며 그 사실을 말한다.
- F4:
  - 브리프 없는 저장소의 `brief show`는 `BRIEF_REQUIRED`이고 이름을 준 변경은 `BRIEF_NOT_FOUND`이다. 색인이 비었을 때 시나리오를 가리키는 증거는 `SCENARIO_INDEX_REQUIRED`이다.
  - 실패 경로: `canon migrate`와 `canon inventory sync`를 한 뒤 같은 호출이 거절하지 않는다.
- F5:
  - 브리프 세 개 중 둘이 거절되는 승인이 첫 거절 id의 코드, 종료 값 2, `error.details[]` 항목 2개, `data` 없음으로 답한다. 아무것도 쓰이지 않는다.
- F6:
  - 표의 옛 이름은 레지스트리와 모든 `errors`와 모든 봉투에 없고 `schema --error`가 거절한다. 표의 새 코드는 레지스트리의 행과 종료 값, 재시도 값이 같다.
  - 실패 경로: 옛 이름을 안내에 되살리면 옛 이름 시험이 실패한다. 이유가 레지스트리에 없는 방출은 실패한다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: 끝난 dispatch run을 다시 끝내는 호출은 지금처럼 거절하고 변경 없음의 성공으로 바꾸지 않는다. 저자에게 묻지 않았다. 사람이 성공으로 보면 이 변경이 바꾼다.
- 계약 저자의 답: 운영체제 오류의 조건은 `canon-0-36-codes-stored`가 정한다. 이 변경의 코드는 모두 종료 값 2이다. 호출자가 준 경로가 디렉터리 자리에 파일이거나 그 반대이면 `INVALID`이다.
- 가정: 표의 새 이름은 issue #135의 제안과 최종 규칙에서 나왔다. 호출 지점을 읽어 원인이 표에 없는 경우 구현자가 코드를 더하고 감사표에 적는다. 같은 조건 안의 상황은 이유로 더하고 코드를 더하지 않는다.
- 가정: 아직 쓰지 않는 원인(예를 들어 `SUBJECT_CONFLICT`의 채택 쪽 이유)은 호출 지점이 실제로 내는 것만 등록한다.
- 가정: 시나리오 문장 37개를 고치면 그 리뷰가 stale이 된다. 리뷰어가 차이표를 보고 다시 리뷰한다. 이 가정이 맞지 않으면 사람이 정책을 정해야 한다.
- 정책 질문은 이 변경에 새로 없다.

## Out of scope

- 이름만 바뀌는 코드(`canon-0-36-codes-rename`)와 게이트 거절의 종료 값(`canon-0-36-codes-exit`).
- 읽지 못한 파일과 깨진 저장 상태, 잠금과 저장소 접근(`canon-0-36-codes-stored`).
- 중간에 멈추는 명령(`canon-0-36-codes-incomplete`)과 이유 이름 전체의 정리(`canon-0-36-code-reasons`).
- 새 거절 규칙을 더하는 일.
