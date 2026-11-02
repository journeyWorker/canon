---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "이름과 철자는 사용자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "이름과 철자는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "이름과 철자는 결정 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
---

# canon의 배치가 /<i> 포인터와 BATCH_INCOMPLETE의 규칙을 따른다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 11.8, 11.11, approaches/migration.md §3.1 4번, approaches/checker.md의 L2.batch-*

## Intent

배치는 두 단계로 돈다. 사전 검사는 모든 항목의 상태 없는 검사(spec 11.7의 1-6단계)를 실행 전에 하고, 실행은 항목을 순서대로 하며 처음 실패한 항목에서 멈춘다. 계약 11.11의 표는 세 경우를 정한다. 사전 검사 실패는 exit 2와 그 항목의 코드, `field`는 `/<index>/…`이고 `data`가 없다. 모두 성공하면 exit 0이다. 실행 중 실패는 exit 1, `BATCH_INCOMPLETE`, `field`는 `/<index>`이다. canon은 네 곳이 다르다. 포인터에 `/items/`가 붙고, `BATCH_INCOMPLETE`의 `field`가 항목 안쪽 포인터이고, 실패한 항목이 `data`를 가질 수 있고, `--dry-run`을 가진 14개 명령은 상태를 읽는 사전 검사를 한다.

## Desired outcome

- 배치의 `field`가 `/<index>/…` 형식이다. `/items/`가 없다.
- `BATCH_INCOMPLETE`의 `field`가 정확히 `/<index>`이다. 항목 안쪽 포인터는 그 항목의 `error`에만 있다.
- ==**사전 검사는 상태를 읽지 않는다.**== 항목이 실행 시점의 상태에서 선행 조건에 실패하면 `BATCH_INCOMPLETE`이다.
- 실패한 항목은 `error`만 가지고 `data`가 없다. 건너뛴 항목은 둘 다 없다.

## User flow

1. F1 에이전트는 배치의 마지막 항목이 틀린 타입을 가진 채 보낸다 → 사전 검사가 exit 2와 `/<index>/…` 포인터로 답한다 → 앞 항목은 실행되지 않았음을 안다.
2. F2 에이전트는 모든 항목이 맞는 배치를 보낸다 → exit 0과 항목마다 `written`이나 `unchanged`가 온다 → 결과를 읽는다.
3. F3 에이전트는 둘째 항목이 존재하지 않는 객체를 이름으로 가리킨 배치를 보낸다 → `BATCH_INCOMPLETE`와 `/1`이 온다 → 첫 항목은 일을 마쳤고 둘째가 실패했고 나머지는 건너뛰었음을 안다.
4. F4 에이전트는 앞 항목이 만든 객체를 뒤 항목이 가리키는 배치를 보낸다 → 사전 검사가 거절하지 않는다 → 순서대로 실행된다.
5. F5 에이전트는 실패한 항목을 읽는다 → 자기 `error`만 있다 → ==**`error`의 `field`로 고친다.**==

## Step detail

### F1 포인터

- ==**배치 안의 문서 값에서 온 오류의 `field`는 항목 번호를 앞에 붙인 JSON 포인터이다.**== `/1/round`, `/0/operands/0`이다. 지금은 `/items/1/round`이다.
- 사전 검사의 거절은 그 항목의 코드와 종료 값(2)이고 `field`가 `/<index>/…`이다. `data`가 없다. 어떤 항목도 실행되지 않는다.
- 배치가 아닌 `--input`의 포인터(`/round`)는 바뀌지 않는다.

### F2 성공

- 모든 항목이 실행되면 exit 0이고 배치 보고서 `{items:[…]}`가 항목마다 `{index, status, data?}`를 하나씩 순서대로 싣는다. 상태는 `written`이나 `unchanged`이고 `data`가 있다.
- 항목이 변경 없음을 보고하는 방식은 지금과 같다. 이 변경은 상태 이름을 바꾸지 않는다.

### F3 실행 중 실패

- 항목이 실행 중 실패하면(상태에 따른 선행 조건 포함) 배치가 멈춘다. exit 1, `BATCH_INCOMPLETE`, `field`가 `/<index>`이다. 항목 번호가 0이어도 같다. 첫 항목이 실패해도 exit 2가 아니다.
- 앞 항목은 `written`이나 `unchanged`, 실패한 항목은 `failed`와 자기 `error`, 뒤 항목은 `skipped`이다.
- `failed`는 되돌렸다는 뜻이 아니다. 그 항목의 `error` 코드의 종료 값이 1이면 효과가 있었을 수 있고 2이면 실행을 거절한 것이다. 배치는 트랜잭션이 아니다.
- `data`는 `BATCH_INCOMPLETE` 오류와 함께 배치 보고서로 온다.

### F4 사전 검사가 읽지 않는 상태

- 사전 검사는 상태를 읽지 않는다. 지금 `--dry-run`을 가진 14개 명령은 사전 검사에서 각 항목의 `--dry-run`을 돌려 상태를 읽고, 없는 객체를 가리키면 exit 2와 그 항목의 코드를 낸다. 이 사전 `--dry-run`은 없앤다.
- 한 항목이 앞 항목이 만드는 객체를 이름으로 가리켜도 사전 검사가 거절하지 않는다. 실행 시점에 앞 항목이 만든 뒤에 확인한다.
- 개별 `--dry-run`은 지금처럼 각 항목의 옵션으로 남는다. 배치 전체의 사전 `--dry-run` 호출은 이 변경이 아니다.
- 이는 동작 변경이다. 사전 검사가 거절하던 배치가 이제 앞의 항목들을 실행한 뒤 `BATCH_INCOMPLETE`로 멈춘다. 업그레이드 노트에 싣는다.

### F5 실패한 항목

- `failed` 항목은 `error`를 가지고 `data`가 없다. `error`는 그 코드의 레지스트리 `retryable`을 가진다. 지금은 자식 봉투에 `data`가 있으면 실패한 항목이 그것을 싣는다. 이 `data`는 없앤다.
- `skipped` 항목은 `error`도 `data`도 없다.

## Acceptance

- F1:
  - 배치 `finding close`의 둘째 항목에서 타입이 틀린 `round`는 exit 2, `field` `/1/round`이다. `data`가 없고 첫 항목은 실행되지 않았다.
  - 실패 경로: `/items/`로 시작하는 `field`가 어떤 거절에도 없다. 단일 `--input`의 포인터는 `/round`이다.
- F2:
  - 모든 항목이 맞는 배치가 exit 0이고 `data.items`가 항목마다 한 개씩 순서대로 있고 상태가 `written`이나 `unchanged`이다.
- F3:
  - 존재하지 않는 `finding`을 가리킨 둘째 항목이 있는 배치가 exit 1, `BATCH_INCOMPLETE`, `field` `/1`이다. `data.items`가 `[written, failed, skipped…]`이고 실패한 항목이 `FINDING_NOT_FOUND` 오류를 가진다.
  - ==**첫 항목이 실패하는 배치도 exit 1, `BATCH_INCOMPLETE`, `field` `/0`이다.**==
  - 실패 경로: `BATCH_INCOMPLETE`의 `field`가 `/<index>`이고 그보다 깊지 않다. 항목 안쪽 포인터는 항목의 `error.field`에만 있다.
- F4:
  - `--dry-run`을 가진 명령(`subject adopt`)의 배치에서 존재하지 않는 변경을 가리킨 항목이 exit 1, `BATCH_INCOMPLETE`로 답한다. 지금은 exit 2이다. 앞 항목은 실행되었다.
  - 앞 항목이 만드는 객체를 가리키는 뒤 항목이 사전 검사를 통과하고 순서대로 실행된다.
  - 실패 경로: 상태 없는 위반(타입, 제약)은 여전히 사전 검사에서 exit 2이다. 어떤 항목도 실행되지 않는다.
- F5:
  - 실패한 항목에 `error`만 있고 `data`가 없다. 건너뛴 항목에 `error`도 `data`도 없다.
- `atip check`의 `L2.batch-preflight`, `L2.batch-incomplete`, `L2.batch-ok`가 canon에서 통과한다. 그 외에 `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 결정(계약 저자 답): 11.11의 표대로 실행 중 상태 실패는 항목 번호가 0이어도 exit 1, `BATCH_INCOMPLETE`, `field` `/0`이다. 그 항목은 자기 `error`로 `failed`이고 뒤 항목은 `skipped`이다. exit 2와 `data` 없음은 상태를 읽지 않는 1-6단계의 사전 검사 실패에만 쓴다. 이에 따라 canon의 상태를 읽는 사전 `--dry-run`을 없앤다.
- 가정: 사전 `--dry-run` 없이 실행되면 앞 항목들이 일을 한 뒤 중간에 멈출 수 있다. 계약은 이것을 정상으로 본다(배치는 트랜잭션이 아니다). 호출자는 `data.items`로 어디까지 갔는지 읽는다. 업그레이드 노트에 싣는다.
- 가정: 이 변경은 항목의 `error` 모양과 `status` 이름을 바꾸지 않는다. 포인터와 `BATCH_INCOMPLETE`의 `field`와 `data`의 유무만 바꾼다.
- 정책 질문은 없다.

## Out of scope

- 입력 문서의 처리 순서와 NDJSON 규칙(`canon-0-35-input-order`).
- 배치를 지원하지 않는 명령(파괴적 명령)의 배치 거절 규칙.
- 항목의 `status` 이름과 `schemaVersion` 올리기. 배치 보고서의 모양이 바뀌는 만큼 minor를 올릴지는 구현자가 기록한다.
- 새 배치 명령을 더하는 일.

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:212d6e46df563112db9e240b78e5341e38e1e0e9db7c2e4640536474f91d6593)
