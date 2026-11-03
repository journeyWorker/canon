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

# 중간에 멈춘 명령은 <SUBJECT>_INCOMPLETE로 내고 STOPPED_MIDWAY를 없앤다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 7.2.2, 7.2.3, 8.3.2, approaches/migration.md §7과 §8, 계약 저자의 1.3.0 최종 규칙 댓글, canon issue #135의 (b)와 3절 6항

## Intent

여러 단계로 일하는 명령은 앞 단계가 효과를 낸 뒤 뒤 단계에서 멈출 수 있다. canon 0.35는 이 경우를 예약 코드 `STOPPED_MIDWAY`로 내고, 이유로 거절의 코드를 싣는다. 계약 1.3은 이 예약 코드를 없애고 대상마다 하나의 `<SUBJECT>_INCOMPLETE` 코드를 쓰게 한다. 한편 canon에는 중간에 멈추는데 이름이 `*_FAILED`인 코드가 있다. `LEDGER_COMPACT_FAILED`는 "옮긴 것은 옮긴 채로 남는다"고 레지스트리가 말한다. 이름과 종료 값이 같아 호출자는 실패한 명령을 반복해도 되는지 알 수 없다.

## Desired outcome

- 앞 단계가 효과를 낸 뒤 멈춘 명령은 `<SUBJECT>_INCOMPLETE`로 답한다. 종료 값 1이고 `meta.next`의 첫 항목이 한 일을 읽는 명령이다.
- ==**하나가 통째로 실패하거나 모두 되돌려진 명령만 `*_FAILED`로 낸다.**==
- `STOPPED_MIDWAY`는 레지스트리와 소스에서 없어진다. `canon init`의 중단은 `INIT_INCOMPLETE`이고 이유가 그 거절의 코드이다.
- 재시도 값은 레지스트리가 정한다. 같은 호출을 반복해도 안전하면 예이고, 반복하면 `PLAN_STALE`을 만나는 확정 명령은 아니오이다.
- 대상마다 `_INCOMPLETE`는 하나이다. 도구 전체의 중단은 명령 이름을 쓴다.

## User flow

1. F1 에이전트는 `canon init`이 설정을 쓴 뒤 스킬 설치에서 멈춘 응답을 읽는다 → `INIT_INCOMPLETE`와 이유(거절의 코드)가 온다 → ==**반복하기 전에 한 일을 읽는다.**==
2. F2 에이전트는 확정 명령이 일부를 한 뒤 멈춘 응답을 읽는다 → `MIGRATION_INCOMPLETE`, `LEDGER_COMPACT_INCOMPLETE`, `RETENTION_PURGE_INCOMPLETE` 같은 코드가 재시도 값 아니오로 온다 → 남은 것을 새 미리보기로 다시 계획한다.
3. F3 에이전트는 이어서 하면 되는 저장 명령이 멈춘 응답을 읽는다 → `ARTIFACTS_STORE_INCOMPLETE`가 재시도 값 예로 온다 → 같은 호출을 반복해 이어 간다.
4. F4 에이전트는 쓰기 하나가 통째로 실패한 응답을 읽는다 → `WRITE_FAILED`나 `*_FAILED`가 온다 → 재시도 값대로 다시 한다.
5. F5 에이전트는 레지스트리를 읽는다 → `STOPPED_MIDWAY`가 없고 `*_INCOMPLETE` 코드마다 이유가 있다 → 이유별 해결을 읽는다.
6. F6 사람은 CHANGELOG를 읽는다 → 옛 코드와 새 코드, 재시도 값이 표로 있다 → 스크립트를 고친다.

## Step detail

### F1 INCOMPLETE와 FAILED

- ==**`INCOMPLETE`는 명령이 실행되어 일을 남긴 채 멈췄고 한 일은 그대로 남는 경우이다.**== `FAILED`는 실행되어 결과가 부정적이거나 한 작업이 통째로 실패한 경우이다. 구현자가 exit 1 코드마다 호출 지점을 읽고 부분 효과가 남는지로 가른다. 결과를 감사표에 적는다(0.35의 감사표를 갱신한다).
- 대상마다 `_INCOMPLETE`는 하나이다. 같은 대상의 여러 중단은 이유로 가른다. 이미 있는 `CHANGE_ADOPTION_INCOMPLETE`와 core 코드 `BATCH_INCOMPLETE`는 그대로이다.
- `WRITE_FAILED`처럼 한 범주가 쓰는 일하는 중의 실패 코드는 이름이 그대로이다. 쓰기 하나의 실패는 `FAILED`이다.

### F2 STOPPED_MIDWAY의 퇴장

- `STOPPED_MIDWAY`는 등록에서 빠지고 소스의 모든 방출 지점이 새 코드를 쓴다. 지금 방출하는 곳은 `canon init`의 스킬 설치 단계이다. 이 단계가 쓴 뒤 거절을 만나면 `INIT_INCOMPLETE`이고 이유가 그 거절의 코드이다.
- `INIT_INCOMPLETE`의 이유는 스킬 단계가 만날 수 있는 종료 값 2의 코드이다: `SKILLS_ARGUMENT_INVALID`, `SKILLS_SOURCE_NOT_FOUND`, `CONFIG_INVALID`. 이유는 거절의 코드 하나이고 그 거절의 이유는 싣지 않는다. 각 이유는 레지스트리에 등록한다.
- 배치 항목이 이렇게 멈추면 그 항목은 `failed`이고 오류가 `INIT_INCOMPLETE`이다. 배치는 `BATCH_INCOMPLETE`이다.
- `meta.next`의 첫 항목은 한 일을 읽는 명령이다. 반복 명령이 아니다.

### F3 확정 명령의 중단

- 이동, 삭제, 압축처럼 미리보기 digest로 확정하는 명령이 일부를 한 뒤 멈추면 `<SUBJECT>_INCOMPLETE`이고 재시도 값이 아니오이다. 같은 `--commit --plan-digest`를 반복하면 `PLAN_STALE`을 만나기 때문이다. `meta.next`는 현재 상태를 읽는 명령이고, 남은 일은 새 미리보기로 시작한다.
- `migrate`의 세 중단(`MIGRATE_STOPPED`, `MIGRATE_INDEX_FAILED`, `MIGRATE_PINS_FAILED`)은 `MIGRATION_INCOMPLETE`의 이유 `STOPPED`, `INDEX`, `PINS`이다. 이유마다 해결이 다르다.
- `inventory sync`가 어느 스펙 루트를 위반이나 중복 시나리오 때문에 중단했고 앞의 루트는 색인했으면 `INVENTORY_INCOMPLETE`이다. 이유는 `VIOLATIONS`, `DUPLICATE_SCENARIOS`이다. `data.roots`가 루트마다 결과를 말한다.
- `plugin sync`가 일부 후보를 쓰지 못했으면 `PLUGIN_SYNC_INCOMPLETE`이고 `data.roots`가 오류를 말한다.

### F4 이어 가는 저장

- 인공물 저장소에 쓰다 멈추고 저장된 블롭은 남으며 같은 호출이 이어 가면 `ARTIFACTS_STORE_INCOMPLETE`이고 재시도 값 예이다. 쓰다 실패했지만 기록과 블롭을 모두 되돌렸으면 `ARTIFACTS_STORE_FAILED`이고 재시도 값 예이다. `evidence add`가 쓰던 `EVIDENCE_STORE_FAILED`는 이 코드로 합쳐진다.
- `artifacts move --commit`의 중단은 `ARTIFACTS_MOVE_INCOMPLETE`이고 재시도 값 아니오이다. 확정이 digest로 막히기 때문이다. 같은 이름이 두 명령에 쓰이던 것을 가른 것이다.

### F5 그 밖의 중단 코드

- 훅 설치가 일부 파일을 쓴 뒤 멈추면 `HOOKS_INSTALL_INCOMPLETE`, 보존 삭제가 지우기 시작한 뒤 멈추면 `RETENTION_PURGE_INCOMPLETE`, 장부 압축이 일부를 옮긴 뒤 멈추면 `LEDGER_COMPACT_INCOMPLETE`이다. 계층 이동이 일부를 옮긴 뒤 멈추면 `TIER_INCOMPLETE`이고, 하나도 옮기지 못하고 실패하면 `TIER_FAILED`이다.
- 승격은 스테이징한 기록이 남고 승격이 실패했으면 `PROMOTION_FAILED`이다. 전략 승격 실패(`STRATEGY_PROMOTION_FAILED`)는 호출 지점이 이름 붙이는 원인(메시지가 말하는 것)마다 부분 효과를 확인해 `FAILED`와 `INCOMPLETE`로 가른다. 구현자가 감사표에 적는다.

### F6 표와 소비자

| 지금 | 지금 exit | 새 코드 | 이유 | 새 exit | 재시도 |
|---|---|---|---|---|---|
| `ARTIFACTS_STORE_FAILED` | 1 | `ARTIFACTS_STORE_INCOMPLETE` · `ARTIFACTS_MOVE_INCOMPLETE` |  | 1 | 예/아니오 |
| `EVIDENCE_STORE_FAILED` | 1 | `ARTIFACTS_STORE_FAILED` |  | 1 | 예 |
| `HOOKS_INSTALL_FAILED` | 1 | `HOOKS_INSTALL_INCOMPLETE` |  | 1 | 아니오 |
| `INVENTORY_ABORTED` | 1 | `INVENTORY_INCOMPLETE` |  | 1 | 아니오 |
| `LEARN_PROMOTION_FAILED` | 1 | `STRATEGY_PROMOTION_FAILED` |  | 1 | 아니오 |
| `LEDGER_COMPACT_FAILED` | 1 | `LEDGER_COMPACT_INCOMPLETE` |  | 1 | 아니오 |
| `MIGRATE_INDEX_FAILED` | 1 | `MIGRATION_INCOMPLETE` | INDEX | 1 | 아니오 |
| `MIGRATE_PINS_FAILED` | 1 | `MIGRATION_INCOMPLETE` | PINS | 1 | 아니오 |
| `MIGRATE_STOPPED` | 1 | `MIGRATION_INCOMPLETE` | STOPPED | 1 | 아니오 |
| `PLUGIN_SYNC_ERRORS` | 1 | `PLUGIN_SYNC_INCOMPLETE` |  | 1 | 아니오 |
| `PROMOTE_FAILED` | 1 | `PROMOTION_FAILED` |  | 1 | 아니오 |
| `RETENTION_PURGE_FAILED` | 1 | `RETENTION_PURGE_INCOMPLETE` |  | 1 | 아니오 |
| `STOPPED_MIDWAY` | 1 | `INIT_INCOMPLETE` | 거절의 코드 | 1 | 아니오 |
| `TIER_FAILED` | 1 | `TIER_INCOMPLETE` · `TIER_FAILED` |  | 1 | 아니오 |

- 소비자 이동, 시나리오 문장(20개와 `atip.midway.*` 14개의 개정), conformance, CHANGELOG 행은 `canon-0-36-codes-rename`과 같은 규칙으로 한다. 시험 `stopped_midway`는 새 코드로 다시 쓴다. 옛 이름 모음에 `STOPPED_MIDWAY`와 이 변경이 없앤 이름을 더한다.

## Acceptance

- F1:
  - 감사표가 레지스트리의 모든 종료 값 1 코드를 `FAILED`, `INCOMPLETE`, 검사 결과, core로 가른다. 표의 모든 코드가 레지스트리와 소스에 있는지 확인한다.
  - 실패 경로: 표에 없는 종료 값 1 코드가 있으면 시험이 실패한다. 이름이 `_INCOMPLETE`로 끝나는데 부분 효과가 남지 않는 호출은 실패한다.
- F2:
  - `canon init`이 스킬 설치에서 거절을 만나게 하면 종료 값 1, `INIT_INCOMPLETE`, 이유(거절의 코드), 한 일을 읽는 `meta.next[0]`이 온다. 설정 파일과 그 옆의 파일은 쓰여 있다.
  - 실패 경로: 효과가 없는 상태의 같은 거절은 종료 값 2와 거절의 코드이다. 등록되지 않은 이유를 내는 시험은 실패한다.
- F3:
  - ==**확정이 일부를 한 뒤 멈추면 종료 값 1, `<SUBJECT>_INCOMPLETE`, 재시도 값 아니오이고, 같은 호출을 반복하면 `PLAN_STALE`이다.**==
  - 실패 경로: 아무것도 하기 전의 거절은 종료 값 2이다. 위반으로 중단한 `inventory sync`는 앞 루트의 색인이 남고 `data.roots`가 그렇게 말한다.
- F4:
  - 이어 가는 저장이 재시도 값 예로 멈추고, 같은 호출이 남은 블롭을 올려 성공한다. 되돌린 저장은 `ARTIFACTS_STORE_FAILED`이고 아무것도 남지 않는다.
  - 실패 경로: `artifacts move --commit`이 멈추면 재시도 값이 아니오이고 같은 호출은 `PLAN_STALE`이다.
- F5:
  - 훅, 보존 삭제, 장부 압축, 계층 이동이 일부를 한 뒤 멈추는 시험이 각자의 `*_INCOMPLETE`와 남은 효과를 확인한다.
  - 실패 경로: 쓰기 하나가 실패하면 `WRITE_FAILED`이고 `INCOMPLETE`가 아니다.
- F6:
  - `canon schema --errors --json`에 `STOPPED_MIDWAY`가 없고 `*_INCOMPLETE` 코드마다 이유가 있다. 표의 옛 이름은 모든 곳에 없다.
  - 실패 경로: 옛 이름을 되살리면 옛 이름 시험이 실패한다. 종료 값을 덮어쓰는 호출이 있으면 종료 값 시험이 실패한다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: 표는 issue #135와 계약 저자의 규칙에서 나왔다. 호출 지점에서 부분 효과가 없음이 확인되면 `FAILED`로 두고 감사표에 적는다. 부분 효과가 확인되는 `FAILED`는 `INCOMPLETE`로 옮긴다.
- 가정: `PROMOTE_FAILED`는 스테이징된 기록이 일반 흐름이 받아들이는 상태로 남으므로 `FAILED`로 둔다. 스테이징을 효과가 남은 중단으로 보면 `PROMOTION_INCOMPLETE`가 맞다. 구현자가 호출 지점을 읽고 정한다.
- 가정: `MIGRATION_INCOMPLETE`의 재시도 값은 아니오이다. 같은 `--commit --plan-digest`를 반복하면 `PLAN_STALE`을 만나기 때문이다. 계약 문서는 "재시도 가능일 수 있다"고 쓰지만 7.1.1의 정의가 이 값을 정한다.
- 가정: 시나리오 문장 20개를 고치면 그 리뷰가 stale이 된다. 리뷰어가 차이표를 보고 다시 리뷰한다. 이 가정이 맞지 않으면 사람이 정책을 정해야 한다.
- 정책 질문은 이 변경에 새로 없다.

## Out of scope

- 이름만 바뀌는 코드(`canon-0-36-codes-rename`), 게이트 거절(`canon-0-36-codes-exit`), 거절의 조건 구분(`canon-0-36-codes-refusals`), 읽기 실패(`canon-0-36-codes-stored`).
- 이유 이름 전체의 정리(`canon-0-36-code-reasons`).
- 중단된 명령을 자동으로 이어 가는 일.

Approved: Journey Hwang 2026-11-03T22:21:29Z — "휴가 중 승인 위임(2026-11-01 대화); 코드 개명은 1.3 컨벤션 나오면 한 번에(2026-11-03 사용자 결정), 체커는 atip 레포로(2026-11-03 사용자 결정)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:82b5137d9db214af043746429afac90a1aabf2a00871cc7b8a5832f3b36d8a56)
