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
    section: "brief"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
---

# canon이 STOPPED_MIDWAY를 등록하고 단계 중단을 그 코드로 낸다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 7.2.1, 7.2.3, 8.2, 8.3.2, 10.13 규칙 14, approaches/migration.md §7, 계약 저자의 답(2026-11-02)

## Intent

여러 단계로 일하는 명령은 앞 단계가 효과를 낸 뒤에 뒤 단계가 작업 전이라면 exit 2로 거절했을 상황을 만날 수 있다. 이 시점에는 exit 2로 끝낼 수 없다(spec 8.2). canon은 이런 경우마다 exit 1짜리 쌍둥이 코드를 따로 갖는다. `MIGRATE_STOPPED`, `CHANGE_ADOPTION_INCOMPLETE`가 그렇다. 계약 1.2는 이 상황 하나를 예약 코드 `STOPPED_MIDWAY`(종료 값 1, 재시도 불가)로 정하고 `reason`에 그 거절의 코드를 싣게 한다. 호출자는 코드마다 쌍둥이를 따로 배우지 않고, `meta.next`가 한 일을 읽도록 이끈다.

## Desired outcome

- canon이 `STOPPED_MIDWAY`를 레지스트리에 등록한다. 종료 값 1, 재시도 불가, 이유는 단계가 만난 거절의 코드이다.
- ==**앞 단계가 효과를 낸 뒤 뒤 단계가 작업 전이라면 거절했을 상황을 만난 명령만 이 코드를 쓴다.**==
- 실제 입출력 실패 범주(`WRITE_FAILED`와 `*_FAILED`)는 자기 코드를 유지한다. 이름만 바꾸지 않는다.
- 이동한 쌍둥이 코드는 레지스트리와 소스에서 없어지고 CHANGELOG에 대체표가 있다.
- 이 릴리스에서 이동은 부분 집합이다. 나머지는 canon이 나중에 옮긴다(계약 9.10).

## User flow

1. F1 에이전트는 여러 단계 명령이 중간에 멈춘 응답을 읽는다 → 코드가 `STOPPED_MIDWAY`이고 `reason`이 거절의 코드이다 → ==**반복하기 전에 한 일을 읽는다.**==
2. F2 에이전트는 `meta.next`의 첫 항목을 실행한다 → 한 일을 읽는 명령이 온다 → 무엇이 끝났는지 안다.
3. F3 에이전트는 레지스트리를 읽는다 → `STOPPED_MIDWAY`의 이유가 단계가 만난 거절의 코드로 나열되어 있다 → 이유별 해결을 읽는다.
4. F4 에이전트가 입출력 실패를 만난다 → 지금과 같이 `WRITE_FAILED`나 `*_FAILED`이다 → 재시도 여부를 레지스트리로 안다.
5. F5 사람은 CHANGELOG를 읽는다 → 옮겨진 코드와 남은 코드가 표로 있다 → 자기 스크립트를 고친다.

## Step detail

### F1 코드와 이유

- ==**`STOPPED_MIDWAY`는 종료 값 1이고 재시도 불가이다.**== 호출자는 같은 호출을 그대로 반복하지 않는다. 앞 단계가 이미 일을 했기 때문이다.
- `reason`은 그 단계가 작업 전에 만났다면 낼 exit 2 코드의 이름 하나이다. 그 거절의 `reason`은 싣지 않고 복합 이름도 쓰지 않는다. 메시지가 더 말할 수 있다.
- 각 이유는 레지스트리의 `STOPPED_MIDWAY` 아래에 `{reason, meaning, fix}`로 등록된다. `fix`는 호출자가 한 일을 읽은 뒤 할 일을 말한다.
- 배치 항목이 이렇게 멈추면 그 항목은 `failed`이고 오류가 `STOPPED_MIDWAY`이다. 배치는 `BATCH_INCOMPLETE`이다.

### F2 meta.next

- `meta.next`의 첫 항목은 한 일을 읽는 명령이다. 읽기 명령이다. 예를 들어 상태나 변경 목록을 보여 주는 `canon status`나 해당 객체의 `show`이다. 반복 명령은 첫 항목이 아니다.

### F3 이동할 코드의 감사

- 구현자가 레지스트리의 exit 1 코드를 모두 훑어 둘로 가른다. 하나는 실제 입출력 실패 범주이고 다른 하나는 앞 단계가 효과를 낸 뒤 작업 전이라면 exit 2일 거절을 만난 경우이다. 결과를 표로 변경 기록과 CHANGELOG에 싣는다.
- 알려진 후보는 다음과 같다. 구현자가 각각을 판정한다.
  - `MIGRATE_STOPPED`: `migrate`가 이름 바꾸기, 브리프 다시 쓰기, 설정 쓰기 중 어느 단계에서 거절을 만나 멈춘다.
  - `CHANGE_ADOPTION_INCOMPLETE`: `change new`가 변경 디렉터리를 쓴 뒤 채택이 거절된다. 입출력 실패면 `WRITE_FAILED`가 맞다.
  - `EVIDENCE_BATCH_REFUSED`, `DISPATCH_END_IN_PROGRESS`, `PROMOTE_FAILED`도 같은 방식으로 판정한다.
- 모든 이름 바꾸기는 호출자 표면의 변경이다. 이 릴리스에서 이동하는 것은 판정에서 앞 단계의 효과가 남는 것으로 확인된 코드뿐이다.
- 아무 후보도 이동 대상이 아니면 `STOPPED_MIDWAY`는 등록만 하고 쓰지 않는다. 그 경우는 이 변경을 닫을 때 그 사실을 감사 표에 적는다.

### F4 입출력 실패

- `WRITE_FAILED`, `RETENTION_PURGE_FAILED`, `ARTIFACTS_STORE_FAILED`, `LEDGER_COMPACT_FAILED`, `TIER_FAILED` 같은 입출력 실패는 이름을 유지한다. 이들은 작업 중 실패 범주이고 작업 전 거절의 쌍둥이가 아니다(spec 8.3).

### F5 표와 소비자

- CHANGELOG의 이 버전 항목에 옮겨진 코드(옛 이름과 새 이유)와 유지한 코드를 싣는다.
- 옛 코드의 이름을 읽던 소비자(스킬, 웹사이트, 평가 스크립트, conformance 케이스, 시나리오 문장)가 새 이름과 이유로 옮겨 간다.

## Acceptance

- F1:
  - 레지스트리에 `STOPPED_MIDWAY`가 종료 값 1, 재시도 불가로 있다. 이유가 등록된 exit 2 코드의 이름이다. `canon schema --errors --json`과 `--error STOPPED_MIDWAY --json`이 같은 이유 목록을 낸다.
  - 이동한 명령마다 앞 단계가 쓴 뒤 뒤 단계가 거절하도록 만든 시험이 exit 1, `STOPPED_MIDWAY`, 해당 `reason`, 한 일을 읽는 `next`를 확인한다.
  - 실패 경로: 효과가 없는 상태에서 거절하는 경우는 exit 2와 거절의 코드이고 `STOPPED_MIDWAY`가 아니다. 등록되지 않은 이유를 내는 시험은 실패한다.
- F2:
  - 이동한 명령의 오류 봉투에서 `meta.next[0]`이 읽기 명령이고 그것을 실행하면 한 일을 보여 준다. 반복 명령이 첫 항목이 아니다.
  - 실패 경로: `meta.next`가 비어 있으면 안 된다. 첫 항목이 같은 쓰기 명령이면 시험이 실패한다.
- F3:
  - 감사 표가 exit 1 코드 전부를 두 부류로 싣는다. 표의 모든 코드가 레지스트리와 소스에서 존재 확인된다.
  - 실패 경로: 표에 없는 exit 1 코드가 있으면 시험이 실패한다.
- F4:
  - 입출력 실패 코드는 이름과 종료 값이 그대로이다. 이 코드를 낸 쓰기 명령의 시험이 0.34와 같다.
- F5:
  - 옛 코드 이름을 검사하는 시험이 안내, 웹사이트, 평가 스크립트에서 통과하고 하나를 되살리면 실패한다. CHANGELOG에 대체표가 있다.
- ==**소스에서 종료 값을 덮어쓰는 호출이 없다는 시험이 계속 통과한다.**== `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 결정(계약 저자 답): `STOPPED_MIDWAY`는 "뒤 단계가 작업 전이라면 거절했을 것을 만난" 경우에만 쓴다. 실제 입출력 실패 범주는 자기 `*_FAILED` 코드를 유지한다. 이 릴리스에서 부분 집합만 옮기고 나머지는 둔다. 각 이름 바꾸기는 canon의 표면 변경이다(9.10). 검사기는 레지스트리 행과 `L1.registry`만 본다.
- 가정: 이동 후보의 판정은 구현자의 감사에 맡긴다. 계획자가 코드의 의미 문장만으로 판정할 수 없는 코드가 있다. 이 변경이 후보를 하나도 옮기지 않아도 등록만으로 `L1.registry`는 통과한다.
- 가정: `PLAN_STALE`을 확정 중 효과를 낸 뒤 감지하는 경로는 canon에 없다. 모든 확정이 첫 효과 전에 한 번 관문을 지난다. 그래서 `PLAN_STALE` 이유는 지금 쓰이지 않는다. `canon-0-35-plan-jcs`가 같은 사실을 확인한다.
- 정책 질문은 없다.

## Out of scope

- 입출력 실패 코드의 이름 바꾸기와 쌍둥이를 모두 옮기는 일.
- 확정 중 효과마다 계획을 확인하는 일(`canon-0-35-plan-jcs`).
- 읽기 명령이 거절 전에 쓰는 문제(`canon-0-35-read-writes`).
- 1.2 선언과 L2(`canon-0-35-claim-l2`).

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:916d9ebb766766d4e40db12ca0184e7425bcbf3b979058aebdaef8f1401e3e4b)
