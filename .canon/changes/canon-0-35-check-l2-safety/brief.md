---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "L2 안전 검사 10개와 비동기 시험 방식을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여섯 단계가 각자 검사 규칙과 fixture 사용을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "검사 id와 오류 코드는 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, fixture 키, 오류 코드는 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, fixture 키, 오류 코드는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "검사 id와 fixture 키는 결정 대상의 철자 자체다"
---

# atip check L2: 멱등성, 파괴적 계획, 비동기, 효과 검사

Source: https://github.com/journeyWorker/atip tag v1.2.1, approaches/checker.md §3, §4, §5와 §6의 L2 표, spec 12와 13.2-13.4, issue 134

## Intent

파괴적 명령의 안전 규칙(미리보기가 먼저이고, digest가 상태에 묶이고, 확정은 짝이 있어야 한다)은 도구마다 자기 테스트로 확인한다. 같은 규칙을 검사기가 바이너리 밖에서 확인하지 않으면 한 도구가 미리보기에서 파일을 쓰거나 다른 digest를 받아들여도 증명되지 않는다. 쓰기 명령의 반복 실행(멱등성)과 읽기 명령의 부작용, 비동기 명령의 접수와 대기도 마찬가지이다. 이 변경이 L2 행 10개를 더한다.

## Desired outcome

- `--level L2`에서 L2 행 `idempotency`, `destructive-safety`, `destructive-preview`, `destructive-preview-stdin`, `destructive-stale`, `destructive-half`, `destructive-commit`, `async`, `operation-key`, `effects`가 돈다.
- 파괴적 명령이 없는 도구는 파괴 행이 `unsupported`이다. 비동기나 `--operation`이 없는 도구는 그 행이 `unsupported`이다. 어느 것도 fixture만으로 `unsupported`가 되지 않는다.
- ==**미리보기, 거절, `--dry-run`, `--help`, `--version`이 아무것도 바꾸지 않는다는 것을 스냅샷으로 증명한다.**==
- `L2.effects`는 L0와 L1에서도 권고로 돈다. 레벨을 떨어뜨리지 않는다.

## User flow

1. F1 도구 작성자는 파괴적 명령의 미리보기를 시험한다 → 계획 digest와 효과와 확정 호출을 가리키는 첫 `next`가 있고 아무것도 쓰이지 않는다 → 미리보기 보고서를 고친다.
2. F2 도구 작성자는 확정을 시험한다 → 한 글자 다른 digest는 `PLAN_STALE`이고 한쪽만 준 확정은 거절되고 맞는 확정은 효과를 낸다 → 확정 관문을 고친다.
3. F3 도구 작성자는 멱등성을 시험한다 → 같은 쓰기를 두 번 돌린 결과가 선언한 멱등성과 같다 → 선언이나 동작을 고친다.
4. F4 도구 작성자는 효과 검사를 읽는다 → 거절, 미리보기, `--dry-run`, 도움말, 버전, 성공한 읽기가 스냅샷을 바꾸지 않는다 → 숨은 쓰기를 없앤다.
5. F5 도구 작성자는 비동기 명령을 시험한다 → 접수와 대기가 fixture의 통제 가능한 작업자로 세 결과를 낸다 → 접수와 완료를 구분한다.
6. F6 검사기 개발자는 시험을 돌린다 → 행마다 위반을 심은 가짜 도구가 그 행 하나만 실패시킨다 → ==**검사기를 믿을 수 있다.**==

## Step detail

### F1 안전 규칙과 미리보기

- ==**`L2.destructive-safety`: 모든 `destructive` 명령이 `commit`과 `plan-digest` 옵션을 가지고 둘 다 `flagOnly`이고 배치(`input.batch`)가 없다.**== fixture를 읽지 않는다.
- `L2.destructive-preview`: 파괴적 명령마다 fixture의 `preview` 호출을 돌린다. exit 0이고 `data`가 `planDigest`, `effects`, `committed: false`를 가진다. `meta.next[0].argv`가 `--commit`, `--plan-digest <그 digest>`, `--json`을 포함한다. 스냅샷이 바뀌지 않는다.
- `L2.destructive-preview-stdin`: 같은 미리보기를 인수를 `--input -`로 주고 돌린다. `meta.next[0].argv`에 `--input -`가 없고, 그 `next`를 돌리면 같은 계획으로 재생된다.
- 비어 있지 않은 `effectNote`가 있는 명령도 미리보기와 거절은 같은 규칙으로 시험한다. 노트가 쓰기를 숨길 수 없다.

### F2 확정

- `L2.destructive-stale`: 미리보기의 확정 argv에서 digest의 한 글자를 바꿔 돌린다. exit 2, `PLAN_STALE`이고 `meta.next[0]`이 미리보기이다. 아무것도 바뀌지 않는다.
- `L2.destructive-half`: `--commit`만, `--plan-digest x`만 준다. exit 2이고 코드는 `USAGE`, 도구가 등록한 더 세분된 코드, 또는 둘의 관계를 말하는 `requires` 규칙의 `code`이다(spec 10.8.1).
- `L2.destructive-commit`: 미리보기의 `next[0]`를 같은 순서 안에서 돌린다. exit 0이고 `committed: true`이며 효과가 실제로 일어난다. 스냅샷에서 변화가 보인다.
- 미리보기와 그 확정은 한 순서(같은 상태 사본)에서 돈다. 다른 시험은 사본이 따로이다.

### F3 멱등성

- `L2.idempotency`: fixture의 `ok` 쓰기 호출마다 같은 순서 안에서 두 번 돌린다. 명령 문서의 `idempotency` 값에 따라 본다.
  - `replay-safe`: 두 번째가 `unchanged`나 같은 결과이고 새 효과가 없다.
  - `once`: 두 번째가 exit 2이고 아무것도 바뀌지 않는다.
  - `content-keyed`: 같은 내용이 두 번이면 효과가 한 번이다.
  - `appends`는 시험하지 않는다. 기록이 하나 더 늘어나는 것이 맞다.
- fixture에 올리지 않은 쓰기 명령은 돌리지 않는다. 이름을 보고에 싣는다.

### F4 효과

- `L2.effects`는 모든 `--dry-run`, `--help`, `--version` 호출과 fixture의 성공한 `read` 명령을 돌려 스냅샷이 바뀌지 않는지 본다. `read` 명령 중 `effectNote`가 있는 것은 fixture의 `allowedEffects` 경로 패턴(글롭)에 맞는 변화만 허용한다.
- L2에서는 MUST이다. L0와 L1에서는 SHOULD로 권고만 하고 레벨 판정을 바꾸지 않는다. `L0.refusal-effects`와 `L1.no-state`의 판정은 그대로이다.
- 보고는 바뀐 경로를 싣는다. `effectNote`가 있다고 거절과 미리보기의 변화가 허용되지 않는다.

### F5 비동기와 작업 키

- 비동기 명령은 명령 문서가 `async: true`로 선언한 것이다. fixture의 `async`는 `start` 호출, 통제 가능한 `worker`(도구의 시험 빌드가 받는 환경 변수), `outcomes`(`done`, `failed`, `timeout`)를 가진다.
- `L2.async`: 시작이 exit 0이고 `async: true`와 식별자가 있다. `next[0]`이 기다린다. 작업자를 `done`으로 두면 대기가 0, `failed`는 1과 등록된 코드, `timeout`은 1과 재시도 가능, `next[0]`이 같은 대기이다.
- `L2.operation-key`: `--operation`을 가진 비동기 시작을 같은 키로 두 번, 새 키로 동시에 두 번, 같은 키에 다른 내용으로 한 번 돌린다. 반복은 같은 식별자를 돌려주고 새 작업을 시작하지 않는다. 작업자는 키마다 시작이 한 번이다. 다른 내용은 `OPERATION_REUSED`이다.
- 비동기나 `--operation`이 없는 도구는 두 행이 `unsupported`이다. 도구의 문서에서 정한다.

### F6 시험

- 가짜 도구의 위반 설정이 행 10개마다 있다. 예는 다음과 같다.
  - 미리보기가 파일을 쓰면 `L2.destructive-preview`.
  - 미리보기 `next`가 `--input -`를 싣으면 `L2.destructive-preview-stdin`.
  - 한 글자 다른 digest를 받아들이면 `L2.destructive-stale`.
  - `--commit`만으로 확정되면 `L2.destructive-half`.
  - `--dry-run`이 파일을 쓰면 `L2.effects`.
  - 같은 `--operation` 두 번째가 작업자를 또 시작하면 `L2.operation-key`.
- 계약 저장소의 예시(`acme deploy start`, `purge` 미리보기)를 시험 입력으로 쓴다. 사본은 `v1.2.1`에 고정한다.

## Acceptance

- F1:
  - 정상 가짜 도구의 파괴적 명령 미리보기가 `planDigest`, `effects`, `committed: false`와 확정 `next`를 내고 스냅샷이 같다. `--input -`로 준 미리보기도 같은 계획이다.
  - 실패 경로: 미리보기가 파일을 쓰는 도구는 `L2.destructive-preview`만 실패하고 그 변화는 `L0.refusal-effects`를 건드리지 않는다. `commit`이 flag-only가 아닌 도구는 `L2.destructive-safety`가 실패한다.
- F2:
  - 한 글자 다른 digest가 `PLAN_STALE`, exit 2, `next[0]`이 미리보기이다. 맞는 확정이 `committed: true`이고 스냅샷이 바뀐다.
  - 실패 경로: 다른 digest를 받아들이는 도구는 `L2.destructive-stale`이 실패한다. `--commit`만으로 확정되는 도구는 `L2.destructive-half`가 실패한다. 짝의 규칙 코드로 거절하는 도구는 통과한다.
- F3:
  - 세 멱등성 값마다 정상 도구가 통과한다. 두 번째 실행이 새 효과를 내는 `replay-safe` 도구는 `L2.idempotency`가 실패한다. `once`가 두 번째에 exit 0이면 실패한다.
  - 실패 경로: `appends`는 시험하지 않고 fixture에 없는 쓰기는 돌리지 않는다.
- F4:
  - `--dry-run`이나 `--help`가 파일을 쓰면 `L2.effects`가 L2에서 MUST로 실패한다. `allowedEffects`에 맞는 읽기의 변화는 통과한다.
  - L0와 L1 실행에서 `L2.effects`가 SHOULD이고 실패해도 레벨이 통과한다. `effectNote`가 있는 명령의 거절 변화는 `L0.refusal-effects`가 실패한다.
  - 실패 경로: `allowedEffects`에 맞지 않는 읽기의 변화는 실패이고 보고가 경로를 싣는다.
- F5:
  - 통제 가능한 작업자의 세 결과가 시험되고 위 표대로 종료 값이 나온다. `OPERATION_REUSED`가 다른 내용에서 나온다.
  - 실패 경로: `async: true`를 쓰지 않는 도구와 `--operation`이 없는 도구는 해당 행이 `unsupported`이다. 접수만 하고 완료를 주장하는 도구는 `L2.async`가 실패한다.
- F6:
  - ==**L2 안전 행 10개 모두에 위반 설정 시험이 있고 소유 행 하나만 실패한다.**==
  - 고정한 예시 사본의 digest가 `v1.2.1` 값과 같다. 정상 가짜 도구와 검사기 자신(L0)이 통과한다.
  - 실패 경로: 행 하나를 항상 통과하도록 바꾸면 그 행의 위반 시험이 실패한다.

## Questions and assumptions

- 가정: `L2.effects`는 L0와 L1에서도 id가 보고에 나오고 SHOULD이다. 검사기 문서의 "M (S at L0 and L1)"을 그렇게 읽었다. 그래서 L0 검사 수 28과 L1 검사 수가 하나씩 늘고 시험의 기대 수가 바뀐다.
- 가정: 효과 허용 경로는 글롭이고 구현자가 어느 글롭 문법을 쓸지 정해 기록한다. checker.md가 문법을 정하지 않는다.
- 가정: 비동기와 작업 키는 canon이 제공하지 않아 canon 쪽에서는 `unsupported`이다. 가짜 도구로만 시험한다.
- 정책 질문은 없다.

## Out of scope

- 입력 문서와 배치(`canon-0-35-check-l2-input`), 스트림과 안내와 검색(`canon-0-35-check-l2-guides`).
- `notRun`을 쓰는 서버 명령의 효과 시험. `canon-0-35-check-fixture`가 정한 대로 뺀다.
- canon의 미리보기와 확정을 계약에 맞추는 일(`canon-0-35-plan-jcs`)과 읽기 명령의 숨은 쓰기를 없애는 일(`canon-0-35-read-writes`).
- 계획 digest 형식 검증. 검사기는 digest를 불투명하게 다룬다.

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:9c71a037b45b09cf020c3591fe4db9aa29dd22ea3f3dadfb11bb13b8c950dd4d)
