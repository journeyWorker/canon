---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "L2 입력과 배치 검사 15개의 규칙과 fixture 사용을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여섯 단계가 각자 검사 규칙과 입력 문서 구성을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "검사 id와 오류 코드는 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, 입력 문서 키, 오류 코드는 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, 입력 문서 키, 오류 코드는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "검사 id와 fixture 키는 결정 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
---

# atip check L2: --input 문서와 배치 검사

Source: https://github.com/journeyWorker/atip tag v1.2.0, approaches/checker.md §3과 §5-§7, spec 11, CHANGELOG 1.2.0의 Checker 항목, issue 134

## Intent

`atip check`는 `--level L2`를 `LEVEL_UNSUPPORTED`로 거절한다. L2 행 30개 중 `--input`과 배치를 다루는 15개는 계약이 가장 자세히 정한 부분이다. 입력 문서의 처리 순서, 문서와 argv의 합치기, 충돌과 환경 변수 값, 배치의 사전 검사와 중단이 그렇다. 도구마다 이 순서를 따로 해석하고 있고, 같은 입력이 도구마다 다른 코드와 `field`로 거절된다. 이 변경이 그 15개를 바이너리 밖에서 증명한다.

## Desired outcome

- `--level L2`가 열린다. L0와 L1의 모든 검사를 돌리고 L2 행을 더한다. 레벨은 L2 MUST가 모두 통과해야 통과한다.
- 입력 문서와 배치의 L2 행 15개가 계약대로 돈다. 입력이 없는 도구는 해당 행이 `unsupported`이다.
- ==**문서와 argv를 합치는 순서를 위반하는 도구가 소유 행 하나만 실패한다.**==
- L1.numbers가 L2에서 문서의 JSON 수와 숫자 문자열도 시험한다.

## User flow

1. F1 도구 작성자는 `--level L2`를 준다 → 검사기가 L0, L1, L2 행을 돌린다 → L2 행의 결과를 읽는다.
2. F2 도구 작성자는 입력 문서의 거절 행을 읽는다 → 잘못된 문서마다 `INPUT_INVALID`와 `field` 포인터와 이유를 시험한다 → 문서의 프레이밍을 고친다.
3. F3 도구 작성자는 합치기 행을 읽는다 → 문서와 argv의 분할, 정규화, 충돌, 같은 값의 다른 표기, 환경 변수 값, 포인터가 시험된다 → 합치는 순서를 고친다.
4. F4 도구 작성자는 배치 행을 읽는다 → 사전 검사 실패와 실행 중 실패와 성공이 시험된다 → 배치 보고서를 고친다.
5. F5 도구 작성자는 fixture에 `batchFailsAt`과 `batchFailsFirst`를 쓴다 → 필요한 항목이 없으면 그 행이 `unprovided`이다 → fixture를 빼서 통과할 수 없다.
6. F6 검사기 개발자는 시험을 돌린다 → L2 입력 행 15개마다 위반을 심은 가짜 도구가 그 행 하나만 실패시킨다 → ==**검사기를 믿을 수 있다.**==

## Step detail

### F1 L2 열기

- ==**`--level L2`는 L0와 L1 검사 전부와 L2 행을 돈다.**== L2 행은 이 변경과 `canon-0-35-check-l2-safety`와 `canon-0-35-check-l2-guides`가 나눠 더한다.
- 이 변경만 병합된 상태에서는 L2의 다른 행이 아직 없다. 세 변경은 같은 릴리스에 싣는다.
- 검사기는 `schema --json`의 `data.level`을 읽고 `L2`와 다르면 `L1.tree`가 실패한다. L1 행은 `L2` 주장에서도 L1 때와 같다.
- `--level L3`은 `USAGE`이다. 검사기 자신은 L0 도구이므로 자기 검사는 `--level L0`이다.

### F2 문서의 프레이밍

- `L2.input-present`: 모든 `write`와 `destructive` 명령에 `input`이 있다. `input`이 없는 쓰기 명령이 있으면 실패이다. fixture를 읽지 않는다.
- `L2.input-example`: `input.example`이 조각 스키마와 `required`, 도메인, 제약을 통과한다. `argumentsRef`가 있으면 그 인수 객체(spec 11.7 6단계, 수는 수로, 기본값 적용)가 그 스키마를 통과한다.
- `L2.input-refusals`: `input`을 가진 모든 명령에 fixture 상태에서 문서를 준다. 모두 exit 2, `INPUT_INVALID`(크기만 `INPUT_TOO_LARGE`)이고 `field`가 JSON 포인터나 `--input`이다.
  - 모르는 키 `{"bogusKey":1}`, 닫힌 키 `{"json":true}`와 `{"commit":true}`.
  - 예시에서 키 하나를 `null`로 바꾼 것, `{"operands":[]}`, 배치가 아닌 명령의 `[{}]`.
  - 없는 파일 `./missing-xyz.json`, `maxBytes`보다 한 바이트 큰 값.
  - 모르는 키 둘 `{"bogusOne":1,"bogusTwo":2}`. 이 답은 `details`가 항목 둘이다.
- 1.2 주장이면 명령이 가진 `limit`, `cursor`, `all`, `fields`마다 그 이름을 문서 키로 준 문서도 같은 방식으로 거절되어야 한다.
- `L2.input-tty`: pty를 stdin으로 `--input -`를 주면 시간 제한 안에 exit 2, `INPUT_INVALID`, `field: "--input"`이다.
- `L2.input-usage`: 1.1 이상 주장에서 MUST이다. `ok` 호출에 `--input '{}'`를 두 번 주거나 `--input`을 마지막 낱말로 주면 `USAGE`나 등록된 더 세분된 코드이고 `field: "--input"`이다. `INPUT_INVALID`는 실패이다. 1.0 주장에서는 SHOULD이다.
- `L2.input-reasons`: 레지스트리가 `INPUT_INVALID`나 `INPUT_TOO_LARGE` 아래에 이유를 나열할 때 SHOULD로 돈다. 위 프레이밍 거절이 spec 7.2.3의 예약 이유(`NOT_JSON`, `NOT_OBJECT`, `UNKNOWN_KEY`, `FLAG_ONLY_KEY`, `INVALID_VALUE`, `UNREADABLE`, `STDIN_TTY`, `TOO_MANY_BYTES`, `BATCH_UNSUPPORTED`)를 가진다.

### F3 합치기

- `L2.input-fragment`: fixture의 `ok` 호출마다 argv 피연산자와 나머지를 문서로 나눠 주면 argv로 준 것과 같은 결과이다. 완전한 문서와 argv 인수 없이 줘도 필수 인수 부족으로 거절되지 않는다.
- `L2.input-normalise`: 정수 인수를 숫자 문자열로 문서에 주면 같은 결과이다.
- `L2.input-conflict`: `ok` 호출에 문서가 한 인수의 다른 값을 주면 `INPUT_CONFLICT`와 문서 포인터이다. 같은 값을 다르게 쓴 경우는 성공이어야 한다. 값은 정규화 뒤에 비교하고 글자로 비교하지 않는다. 같은 값의 다른 표기는 명령이 그런 인수를 가질 때만 시험한다.
  - `json` 객체의 멤버 순서가 다른 것.
  - `number`를 `1.0`으로 쓴 것을 `1`과.
  - 정수 `-0`을 `0`이나 숫자 문자열과.
- `L2.input-env`: `env`를 선언한 옵션이 있는 `input` 명령에서 그 옵션의 키를 유효한 값으로 문서에 준다. 그 옵션의 `env` 변수는 인수가 거절할 값(타입이 틀리거나 도메인 밖)으로 둔다.
  - 결과는 변수가 없을 때와 같아야 한다.
  - 환경 변수 값은 합친 뒤에도 빠진 인수에만 적용되므로(spec 11.7 5단계) 충돌도 검증도 일으키지 않는다.
- `L2.input-pointer`: 제약 규칙 하나를 어기는 문서, 또는 argv도 주지 않는 필수 키를 정말로 뺀 문서는 제약의 코드(없으면 `USAGE`)와 문서 포인터 `field`이다. 빠진 키는 argv에서도 빠져 있다.
- `L2.input-dash-values`: 문자열 값이 `-`, `{`, `[`로 시작하는 문서가 값으로 받아들여진다. 옵션이나 파일이나 인라인 JSON으로 읽지 않는다.
- `L1.numbers`는 L2에서 `integer` 인수에 `9007199254740992`와 `-9007199254740992`를 JSON 수와 숫자 문자열로 문서에 준다. `INPUT_INVALID`로 거절되어야 하고 반올림된 값으로 받아들이면 실패이다.

### F4 배치

- `L2.batch-preflight`: 마지막 항목이 상태 없는 검사(타입, 제약)를 어기는 배치를 준다. exit 2, 그 항목의 코드, `field`가 `/<index>/…`, `data` 없음이다. 앞 항목은 실행되지 않아야 하고 스냅샷이 바뀌면 `L0.refusal-effects`가 실패한다.
- `L2.batch-incomplete`: fixture의 `batchFailsAt`과 `batchFailsFirst`로 돈다. exit 1, `BATCH_INCOMPLETE`, `field`가 `/<index>`이다. `data.items`는 제출한 항목마다 하나이고 순서가 같고 상태는 spec 11.11의 것이다. 실패한 항목은 자기 `error`를 가지고 `meta.output`은 배치 출력이다.
- `L2.batch-ok`: 명령의 `ok` 호출들을 배치로 준다. exit 0, 배치 보고서, 모든 상태가 `written`이나 `unchanged`이다.
- 배치를 지원하지 않는 명령(`input.batch`가 거짓)은 세 행이 `unsupported`이다.

### F5 fixture

- 새 fixture 키는 `commands.<명령>.batchFailsAt`과 `batchFailsFirst`이다. 항목은 `{input: [문서 항목…], index}`이다. `batchFailsFirst`는 첫 항목이 실패하는 배치이다.
- 문서를 만들 때 flag-only 옵션(`json`, `repo`, `as`, 도구가 더한 것)은 argv에 남긴다. 나머지 인수는 camelCase 키와 정규화한 값으로 문서에 간다.
- fixture 항목이 없고 관찰한 위반도 없으면 그 행은 `unprovided`이고 MUST이므로 레벨이 통과하지 못한다. 항목이 없어도 이미 관찰한 위반은 `fail`이다.
- 모든 쓰기 호출은 fixture에 올린 명령에만 돈다. 올리지 않은 쓰기 명령은 이름을 보고에 싣고(`unlisted`) 돌리지 않는다.

### F6 시험

- 가짜 도구의 위반 설정이 L2 입력 행 15개마다 있다. 예는 다음과 같다.
  - `--input '{"commit":true}'`를 받아들이면 `L2.input-refusals`.
  - `{"round":"1"}`을 거절하면 `L2.input-normalise`.
  - 환경 변수를 합치기 전에 적용하면 `L2.input-env`.
  - 값 `-x`를 옵션으로 읽으면 `L2.input-dash-values`.
  - 첫 항목이 사전 검사에서 거절되면 `L2.batch-incomplete`.
- 1.1 이상에서 `--input`을 두 번 주고 `INPUT_INVALID`로 답하는 도구는 `L2.input-usage`가 실패한다. 1.0 주장에서는 SHOULD로 권고만 한다.
- 계약 저장소의 예시 중 입력과 배치의 것(`schema-command-acme-member-tag`, 배치 보고서)을 시험 입력으로 쓴다. 사본은 `v1.2.0`에 고정한다.

## Acceptance

- F1:
  - `--level L2`가 exit 0이거나 `LEVEL_NOT_MET`이다. `LEVEL_UNSUPPORTED`가 아니다. `--level L3`은 exit 2, `USAGE`이다.
  - 정상 가짜 도구가 L2의 적용되는 모든 행을 통과한다. 도구의 트리 `level`이 `L1`이면 `L1.tree`가 실패한다.
  - 실패 경로: `input`이 없는 도구는 입력 행이 `unsupported`이고 `L2.input-present`가 쓰기 명령 때문에 실패한다.
- F2:
  - 위 거절 문서마다 시험이 있고 각각 exit 2와 `field`, 이유를 확인한다. `input-tty`는 pty에서 돈다.
  - 실패 경로: `{"commit":true}`를 받아들이는 도구는 `L2.input-refusals`만 실패한다. 1.2 주장에서 `{"limit":1}`을 받아들이는 도구도 같다. 1.1 주장에서는 실패하지 않는다.
- F3:
  - 합치기 행마다 위반 설정이 있고 소유 행 하나만 실패한다. 같은 값의 다른 표기(멤버 순서, `1.0`, `-0`)를 `INPUT_CONFLICT`로 거절하는 도구는 `L2.input-conflict`가 실패한다.
  - 실패 경로: 환경 변수를 합치기 전에 적용해 유효하지 않은 변수가 문서를 거절하게 하는 도구는 `L2.input-env`만 실패한다.
  - 실패 경로: 빠진 필수 키를 argv로도 주지 않고 시험했는데 `INPUT_INVALID`로 답하면 `L2.input-pointer`가 실패한다.
- F4:
  - 사전 검사 실패, 중단, 성공 배치가 각각 시험되고 `data.items`의 개수, 순서, 상태를 확인한다.
  - 실패 경로: 첫 항목이 실패해 `BATCH_INCOMPLETE` 대신 exit 2를 내는 도구는 `L2.batch-incomplete`가 실패한다. 마지막 항목의 사전 검사 실패를 `BATCH_INCOMPLETE`로 내는 도구는 `L2.batch-preflight`가 실패한다.
- F5:
  - fixture에서 `batchFailsAt`이나 `batchFailsFirst`를 빼면 `L2.batch-incomplete`가 `unprovided`이고 종료 코드는 1이다. 이미 관찰한 위반이 있으면 `fail`이다.
  - 실패 경로: 모양이 틀린 fixture 항목은 exit 2, `FIXTURE_INVALID`이다.
- F6:
  - ==**L2 입력 행 15개 모두에 위반 설정 시험이 있고 소유 행 하나만 실패한다.**==
  - 고정한 예시 사본의 digest가 `v1.2.0` 값과 같다. 정상 가짜 도구와 검사기 자신(L0)이 통과한다.
  - 실패 경로: 행 하나를 항상 통과하도록 바꾸면 그 행의 위반 시험이 실패한다.

## Questions and assumptions

- 결정(계약 저자 답): L2 행과 fixture 키 `preview`, `batchFailsAt`, `batchFailsFirst`, `async`는 v1.2.0에서 안정이고 대기 중인 변경이 없다.
- 가정: `L2.input-example`과 `L2.input-present`는 fixture를 읽지 않으므로 `unprovided`가 될 수 없다.
- 가정: L2 행을 세 변경으로 나눠도 `--level L2`의 문을 여는 것은 이 변경이다. 세 변경을 같은 릴리스에 실으면 중간 상태가 나가지 않는다. 오케스트레이터가 순서를 정한다.
- 가정: 배치 문서 생성에서 항목이 하나뿐인 명령도 배치로 시험한다. 계약 11.11이 항목 하나의 배치를 허용한다.
- 정책 질문은 없다.

## Out of scope

- 위험한 계획과 확정, 비동기, 효과 검사(`canon-0-35-check-l2-safety`).
- 스트림, 안내, 검색, 도움말 첫 줄(`canon-0-35-check-l2-guides`).
- canon이 L2를 통과하게 만드는 일(`canon-0-35-input-order`, `canon-0-35-batch-pointers`, `canon-0-35-claim-l2`).
- 1.2 주장의 L1 행(`canon-0-35-check-1-2`)과 발견의 구조화, fixture 거절 처리.
