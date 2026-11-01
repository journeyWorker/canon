---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "1.1에서 새로 생기는 멤버, 코드, 레지스트리, 소비자 이동을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여섯 단계가 각자 멤버 이름과 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "계약 저자에게 확인할 가정이 서로 다른 결정이라 함께 적어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "meta 키와 코드 이름은 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "meta 키와 코드 이름은 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "meta 키와 코드 이름은 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "meta 키, 코드 이름, reason 이름은 호출자가 읽는 철자 자체이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "meta 키, 코드 이름, reason 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "코드 이름과 계약 번호는 결정 대상의 철자 자체다"
---

# canon이 atip/1 1.1을 채택한다: contractVersion, reason, 경고 레지스트리

Source: https://github.com/journeyWorker/atip tag v1.1.0, spec 6.6, 7.1-7.6, 8.3.1, 9.13, 10.1, 15.6, 15.7, CHANGELOG 1.1.0

## Intent

canon 0.33은 atip/1 1.0의 L0를 따르고, 출력에 1.1의 멤버가 없다. 봉투에 `contractVersion`이 없고, `--input` 형식 오류를 가르는 값이 계약 밖 확장(`x-canon.reason`)에 있다. `schema`에는 경고 레지스트리가 없어 경고 코드가 오류 레지스트리에 섞인다. 0.33은 빈 `received`를 생략하고, 그룹 뒤의 `--repo`를 `USAGE`로 거절하고, `canon`으로 시작하지만 명령 한 줄이 아닌 힌트를 낸다. 그래서 `atip check`의 1.1 검사를 canon이 통과하지 못한다.

## Desired outcome

- 모든 봉투가 `meta.contractVersion: "1.1"`을 가진다. `schema`의 레지스트리 답은 `data.kind`와 `meta.schemaVersion: "1.1"`을 가진다.
- `--input` 형식 오류는 계약의 예약 reason 이름으로 가려지고, `x-canon.reason`은 없다. 레지스트리가 reason을 싣는다.
- 경고 코드는 경고 레지스트리에 있고, `canon schema --warnings`와 `--warning`이 낸다.
- 힌트, `received`, 그룹 뒤의 옵션이 1.1 규칙을 따른다.
- ==**canon JSON을 읽는 모든 소비자가 모르는 멤버와 모르는 열거 값을 무시한다.**==

## User flow

1. F1 호출자는 canon의 아무 봉투나 읽는다 → meta에서 `contract`와 `contractVersion`을 본다 → 어느 계약 판을 따르는지 안다.
2. F2 호출자는 `--input` 오류를 읽는다 → `INPUT_INVALID`나 `INPUT_TOO_LARGE`에 reason이 붙는다 → 코드와 reason으로 다음 행동을 고른다.
3. F3 호출자는 경고와 레지스트리를 읽는다 → 경고 코드를 경고 레지스트리에서 찾는다 → 오류 코드와 경고 코드를 따로 조회한다.
4. F4 호출자는 `USAGE`와 힌트를 읽는다 → 인수를 못 가리킬 때 힌트가 `canon schema <명령> --json`이고, 빈 값은 `received: ""`로 온다 → 힌트를 명령 한 줄로 믿고 실행한다.
5. F5 호출자는 그룹 뒤에 도구 공통 옵션을 붙인다 → `COMMAND_REQUIRED`로 답한다 → 옵션 때문에 `USAGE`를 받지 않는다.
6. F6 에이전트와 사람은 canon의 소비자와 문서를 본다 → 새 이름이고 모르는 멤버를 무시한다 → ==**옛 이름이 남은 곳이 없다.**==

## Step detail

### F1 contractVersion

- 모든 봉투의 meta에 `contract: "atip/1"`과 `contractVersion: "1.1"`이 함께 있다. ==**같은 빌드가 내는 모든 JSON 문서가 같은 값을 가진다.**== 성공과 실패, `schema` 답이 모두 해당한다. `"1.0"`을 적지 않는다.
- `schema --errors`, `--error`, `--warnings`, `--warning`의 답은 `data.kind`(`errors`, `error`, `warnings`, `warning`)를 가진다. 0.33은 `data.kind`가 없다. 이 답들의 `meta.schemaVersion`은 `"1.1"`이다. 다른 출력의 `schemaVersion`은 그 출력의 판이고 지금 그대로다.
- 트리, 그룹, 명령 문서의 모양은 `canon-0-34-atip-schema-doc`가 바꾼다. 그 답의 `meta.schemaVersion`도 `"1.1"`이어야 하므로 두 변경은 같은 릴리스에 싣는다. 이 변경만 먼저 병합하면 1.0 모양의 트리가 1.1 표시를 갖게 된다.
- stream 시작 줄은 canon에 stream이 생기는 0.35 일이다.

### F2 reason

- 예약 reason 이름만 쓴다. `INPUT_INVALID`는 `UNREADABLE`(읽을 수 없는 파일), `STDIN_TTY`, `EMPTY`, `NOT_JSON`, `NOT_OBJECT`, `BATCH_UNSUPPORTED`, `UNKNOWN_KEY`, `FLAG_ONLY_KEY`, `INVALID_VALUE`다. `INPUT_TOO_LARGE`는 `TOO_MANY_BYTES`, `TOO_MANY_ITEMS`다.
- 0.33의 `x-canon.reason`은 이렇게 옮긴다.
  - `unreadable`은 `UNREADABLE`, `tty`는 `STDIN_TTY`, `empty`는 `EMPTY`, `syntax`는 `NOT_JSON`, `batch-unsupported`는 `BATCH_UNSUPPORTED`다.
  - 조각 스키마 위반은 `UNKNOWN_KEY`, `FLAG_ONLY_KEY`, `INVALID_VALUE`로 가린다. 문서나 배치 항목이 객체가 아니면 `NOT_OBJECT`다.
- `x-canon.reason`은 없어지고 별칭이 없다. `details`의 항목도 자기 reason을 가질 수 있다.
- 레지스트리 항목은 `reasons: [{reason, meaning, fix}]`를 가진다. reason은 코드 안에서 유일하고 종료 값과 retryable은 코드의 것이다. 등록되지 않은 reason은 내지 않는다.
- 오류는 reason 없이도 완전하다. reason이 없는 `INPUT_INVALID`가 남지 않게, 이 코드를 내는 모든 곳이 위 아홉 중 하나를 고른다.
- `CURSOR_INVALID`의 reason은 `canon-0-34-atip-paging-fields`가 정한다.

### F3 경고 레지스트리

- 경고 레지스트리 항목은 `{code, meaning, fix}`이고 종료 값과 retryable이 없다. 경고로만 나오는 코드는 오류 레지스트리에서 경고 레지스트리로 옮긴다. 경고와 오류로 모두 나오는 상황은 두 곳에 같은 코드와 같은 `meaning`으로 있다.
- `canon schema --warnings --json`은 `data.kind: "warnings"`로 전부를, `--warning <CODE>`는 `data.kind: "warning"`으로 하나를 낸다. 모르는 코드는 exit 2, `USAGE`, `field`가 `--warning`이다.
- `--error <경고 전용 코드>`는 exit 2, `USAGE`, `field`가 `--error`이고 힌트가 `canon schema --warning <코드> --json`이다. 모르는 `--error` 코드도 `USAGE`, `field`가 `--error`다. 지금의 `UNKNOWN_ERROR_CODE`는 없어진다.
- `schema`의 경로, `--errors`, `--error`, `--warnings`, `--warning`은 서로 배타이고 어기면 `USAGE`다.
- 오류 레지스트리 밖에서 경고 코드를 찾던 소비자는 경고 레지스트리를 먼저 읽는다.

### F4 USAGE, 힌트, received

- 호출자가 보낸 빈 값은 `received: ""`로 보고한다. 값이 없는 인수에는 `received`가 없다. 비밀일 수 있는 값은 이전처럼 가린다. 0.33이 빈 `received`를 뺐다.
- `USAGE`는 `field`를 가진다. 어떤 인수 때문인지 못 가리면 `field`가 없고 힌트는 그 명령의 `canon schema <명령> --json`이다. 명령이 안 풀렸으면 `canon schema --json`이다.
- 힌트는 세 형태 중 하나다. 실행할 호출 한 줄, `<UPPER_CASE>` 자리표시자가 든 호출 틀, 산문 한 문장이다. `canon `과 공백으로 시작하는 힌트는 그 한 줄만 담는다. 산문 힌트는 `canon `으로 시작하지 않는다. `canon retrieve`의 `NoScope` 힌트가 이 규칙을 어긴다.
- 위 규칙을 어기는 힌트가 없음을 소스 전체의 힌트에 대한 테스트가 확인한다.

### F5 그룹 뒤의 도구 공통 옵션

- 모든 명령이 같은 뜻으로 받는 옵션(`--json`, `--repo`, `--as`)은 그룹 뒤에도 받는다. `canon finding --repo . --json`은 `COMMAND_REQUIRED`로 답하고 `expected`가 그 그룹의 명령이다. 그룹에서 이 옵션은 아무 일도 하지 않고 오류도 아니다.
- 그룹이 모르는 옵션은 지금처럼 `USAGE`다.

### F6 소비자와 문서

- 소비자는 모르는 멤버를 무시하고, 모르는 열거 문자열 때문에 문서 전체를 거절하지 않는다(계약 15.6). 대상은 대시보드, npm 런처, dogfood 평가 스크립트, 스킬이 가르치는 읽기 방법이다.
- 스킬, 웹사이트 영어와 한국어, 명령 명세 파일, conformance 케이스, 시나리오 문장에서 옛 이름이 없어진다. 옛 이름은 `x-canon.reason`, `UNKNOWN_ERROR_CODE`, 0.33의 소문자 형식 오류 값이다.
- CHANGELOG의 이 버전 항목은 사라진 이름과 새 이름의 대체표를 싣는다. 경고 전용 코드가 `schema --errors`와 `--error`에서 빠지고 `--warnings`로 옮겨 갔다는 것을 릴리스 노트에 적는다. 1.0 호출자는 그 코드를 `--error`에서 더는 찾지 못한다.

## Acceptance

- F1:
  - `canon status --json`과 `canon nosuch --json`의 meta에 `contract: "atip/1"`과 `contractVersion: "1.1"`이 있다. `canon schema --errors --json`은 `data.kind: "errors"`이고 `meta.schemaVersion`이 `"1.1"`이다.
  - ==**모든 봉투가 계약 봉투 스키마를 통과한다.**== `contractVersion: "1.0"`이거나 `contract` 없이 `contractVersion`만 있는 봉투는 검증에서 거부된다.
- F2:
  - `--input` 형식 오류 다섯 가지는 `UNREADABLE`, `STDIN_TTY`, `EMPTY`, `NOT_JSON`, `BATCH_UNSUPPORTED`를 낸다. 조각 스키마 위반 세 가지는 `UNKNOWN_KEY`, `FLAG_ONLY_KEY`, `INVALID_VALUE`를 내고, 객체가 아닌 문서는 `NOT_OBJECT`다. 크기 초과는 `TOO_MANY_BYTES`와 `TOO_MANY_ITEMS`다.
  - 오류와 `details` 항목에 `x-canon`의 reason이 없다. 내보내는 모든 reason이 레지스트리의 그 코드 아래에 있음을 테스트가 확인한다.
  - 실패 경로: 레지스트리에 없는 reason을 내려는 시험은 테스트에서 실패한다. reason 없이 `INPUT_INVALID`를 내는 곳이 있으면 테스트가 실패한다.
- F3:
  - `canon schema --warnings --json`과 `--warning <CODE>`가 위 모양으로 답하고, 두 답의 항목이 같다. 경고로만 나오는 코드는 `--errors`에 없고 `--warnings`에 있다.
  - `--warning NOSUCH`는 exit 2, `USAGE`, `field`가 `--warning`이다. 경고 전용 코드로 `--error`를 부르면 `USAGE`이고 힌트가 `--warning`을 가리킨다. 모르는 `--error` 코드도 `USAGE`다.
  - `meta.warnings`의 모든 코드가 두 레지스트리 중 하나에 있음을 테스트가 확인한다.
  - 경고와 오류로 모두 나오는 코드는 두 레지스트리에 같은 코드와 같은 `meaning`으로 있다. 다르면 테스트가 실패한다.
- F4:
  - `canon finding add --change "" ...`처럼 빈 값을 보내면 `USAGE`의 `received`가 `""`이다. 값 없이 부르면 `received`가 없다.
  - `field`를 못 가리는 `USAGE`의 힌트는 `canon schema <명령> --json`이다. `canon `으로 시작하는 힌트는 모두 명령 한 줄이고 셸처럼 쪼개진다.
  - 실패 경로: `canon retrieve`에 범위 인수가 없을 때의 힌트가 명령 한 줄이거나 `canon `으로 시작하지 않는 산문이다.
- F5:
  - `canon finding --repo . --json`, `canon finding --as planner --json`이 exit 2, `COMMAND_REQUIRED`다. `canon finding --bogus --json`은 `USAGE`다.
- F6:
  - 옛 이름 검사가 안내, 웹사이트, 평가 스크립트, 명세 파일에서 통과하고 옛 이름 하나를 되살리면 실패한다.
  - 실패 경로: 소비자에게 모르는 멤버가 든 봉투를 주면 읽기를 계속하고, 모르는 열거 문자열은 그 항목만 건너뛴다. 대시보드와 런처 테스트가 해당한다.
  - `cargo test --workspace`, conformance, 생성 문서 drift 테스트가 통과한다.

## Questions and assumptions

- 계약 저자가 정했다. 이 변경은 1.1 채택이다. 변경 목록, 예약 reason 이름, 경고 레지스트리, `received: ""` 복원, 그룹 뒤 옵션이 그 범위다.
- 가정: 8.3.1(작업 전 실패를 확신할 수 없을 때 exit 1로 보고해도 되는 MAY)은 쓰지 않는다. canon은 작업 전에 안 것만 exit 2로 낸다. 0.33과 같다.
- 계약 저자가 정했다. 경고 전용 코드를 `schema --errors`에서 경고 레지스트리로 옮기는 것이 맞다(7.5.1-7.5.3). 조건이 둘이다. 어떤 호출에서는 경고이고 다른 호출에서는 오류인 코드는 두 레지스트리에 같은 코드와 같은 뜻으로 남는다. 경고 전용 코드를 `--error`에 주면 `USAGE`이고 힌트가 `canon schema --warning <코드> --json`이다.
- 계약 저자가 정했다. 모르는 `--error` 코드는 `USAGE`, `field`가 `--error`다. 0.33의 `UNKNOWN_ERROR_CODE`는 `USAGE`로 합친다(계약 7.3.4).
- 가정: `data.kind`가 0.33의 레지스트리 답에 없었던 것은 0.33의 누락이다. 계약 7.3.4는 `data.kind`를 정한다.
- 가정: 이 변경과 `canon-0-34-atip-schema-doc`가 같은 릴리스에 실리므로 `schema` 답의 `meta.schemaVersion`이 "1.1"이어야 하는 규칙은 릴리스 시점에 지켜진다. 이 변경만 먼저 병합하면 트리 답이 1.0 모양에 1.1 표시를 갖는다.
- 가정: 순서는 이 변경, `canon-0-34-atip-check-1-1`, 나머지다. 이 변경은 `atip-contract`와 시나리오 문장에 바뀐 이름이 나오는 출시된 subject에 함께 속한다.
- 정책 질문은 없다. 출력 이름이 바뀌어도 gate가 요구하는 것과 승인자는 바뀌지 않는다.

## Out of scope

- 1.1이 더하는 것 중 canon에 해당하지 않는 것: stream 시작 줄의 `contractVersion`(0.35), 경로 문법의 대괄호 키와 map 키(`canon-0-34-atip-paging-fields`).
- `CURSOR_INVALID`의 reason(`canon-0-34-atip-paging-fields`).
- 트리, 그룹, 명령 문서, 값 타입, 제약, 도메인(`canon-0-34-atip-schema-doc`, `canon-0-34-atip-constraints`).
- `atip check`의 1.1 검사(`canon-0-34-atip-check-1-1`).
- 8.3.1의 보수적 판정을 쓰는 일.
- 0.35의 `--input` 처리 순서, 배치 보고서, 계획 digest, `skills show`, stream.

Approved: Journey Hwang 2026-11-01T06:36:14Z — "승인 (6개 모두 0.34, 깨는 변경 괜찮아)" (relayed by orchestrator orchestrator-main via ask) (brief sha256:67d60ae716ddcb168d2beb3d84ac3e644a17cdbceebdee607dc413081d48c192)
