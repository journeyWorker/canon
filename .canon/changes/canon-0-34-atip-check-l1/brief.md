---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "L1 검사 30개의 규칙, 생성하는 호출, 두 정규식 엔진 비교, fixture 키, 시험 방식을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여섯 단계가 각자 검사 목록과 판정 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "계약 저자의 결정과 가정이 서로 다른 결정이라 함께 적어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "검사 id와 코드 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "검사 id와 옵션 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, 옵션, fixture 키, 코드 이름은 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, 옵션, 코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "검사 id와 fixture 키는 결정 대상의 철자 자체다"
---

# atip check가 L1 검사를 돌리고 canon이 L1을 통과한다

Source: https://github.com/journeyWorker/atip tag v1.1.1, approaches/checker.md §2-§7, spec 10.13, 17

## Intent

`atip check`는 L0만 돌리고 `--level L1`을 `LEVEL_UNSUPPORTED`로 거절한다. `schema`의 트리, 그룹과 명령 문서, 레지스트리, 제약, 도메인, 검증자, 정수 범위, 쪽 나누기, `--fields` 투영은 사람이 읽을 뿐 바이너리를 밖에서 대어 확인하는 도구가 없다. 계약 문서의 `scripts/check.ts`는 계약 저장소의 예시 문서를 검증할 뿐 빌드된 바이너리를 돌리지 않는다. 정규식 패턴은 ECMAScript와 Rust가 같은 값을 받아들이는지 확인할 수단이 없다. 그래서 canon이 L1을 주장해도 증명할 방법이 없다.

## Desired outcome

- `atip check --level L1`이 L0 검사와 L1 검사 31개를 돌린다. 1.1 주장의 도구에는 MUST 25개와 SHOULD 6개가 적용된다.
- 검사기가 `schema` 답을 바이너리마다 읽어 명령 문서의 규칙을 검증하고, 선언한 제약, 도메인, 검증자, 숫자 범위를 실제 호출로 시험한다.
- ==**정규식 패턴이 ECMAScript(`u`)와 Rust에서 같은 값을 받아들이는지 검사기가 비교한다.**==
- canon의 CI가 canon에 L1을 돌리고 통과하며, canon이 트리의 `level`을 `L1`로 주장한다.

## User flow

1. F1 도구 작성자는 `--level L1`을 준다 → 검사기가 `schema --json`의 `data.level`과 주장을 비교하고 L0와 L1 검사를 돌린다 → 둘이 다르면 실패한다.
2. F2 도구 작성자는 자기서술 검사 결과를 읽는다 → 트리, 그룹, 명령 문서, 레지스트리, 문법, 예시가 검증된다 → 어느 문서의 어느 규칙이 깨졌는지 안다.
3. F3 도구 작성자는 인수 검사 결과를 읽는다 → 제약과 도메인과 검증자와 숫자 범위가 실제 호출로 시험된다 → 선언한 것을 파서가 거절하지 않으면 실패로 안다.
4. F4 도구 작성자는 데이터 검사 결과를 읽는다 → 출력 스키마, 다음 단계, 쪽 나누기, `--fields`가 시험된다 → 보고서가 선언대로 나오는지 안다.
5. F5 도구 작성자는 fixture에 L1 항목을 쓴다 → 검사가 필요한 호출이 있고 없음을 구분한다 → fixture를 빼서 통과할 수 없다.
6. F6 검사기 개발자는 시험을 돌린다 → L1 검사마다 위반을 심은 가짜 도구가 그 검사만 실패시킨다 → 검사기를 믿을 수 있다.
7. F7 canon 개발자는 CI에서 canon에 L1을 돌린다 → canon이 L1 검사를 모두 통과한다 → ==**0.34가 L1을 주장할 수 있다.**==

## Step detail

### F1 L1 열기

- ==**`--level L1`은 L0 검사 전부와 L1 검사를 돌린다.**== `L2`는 `LEVEL_UNSUPPORTED`로 거절하고 hint가 0.35를 말한다. 레벨 값은 `L0`, `L1`, `L2`뿐이다.
- 검사기는 `schema --json`의 `data.level`을 읽는다. 주장과 다르면 `L1.tree`가 실패한다. L0 주장에는 `schema`를 읽지 않는다.
- `schema`를 제공하지 않는 도구가 L1을 주장하면 `L1.tree`가 실패한다.

### F2 자기서술 검사

- 모든 문서는 계약의 `schema-doc` 스키마를 통과하고 계약 10.13의 규칙 1부터 16이 적용된다. 규칙 12(예시)는 `L1.examples`가 소유한다.
- 이 단계의 검사는 다음과 같다. 접두사 `L1.`은 생략한다.

| id | 강도 | 하는 일 |
|---|---|---|
| `tree` | M | `schema --json`을 검증한다. 그룹이 자기 명령 앞에 오고 id가 유일하다. |
| `no-state` | M | 빈 디렉터리와 최소 환경에서 `schema`가 답하고 아무것도 쓰지 않는다. |
| `walk` | M | 트리의 모든 항목에 `schema <경로>`를 부르고 문서를 규칙 1-11, 13으로 검증한다. 그룹의 자식이 트리와 같다. |
| `quoted-path` | M | 두 낱말 명령을 한 문자열로 줘도 낱말로 줄 때와 같은 답이다. |
| `schema-errors` | M | 모르는 경로는 `UNKNOWN_COMMAND`이고 `expected`는 유효한 낱말이 있을 때만 있고 `field`가 있으면 `command`다. 겹친 옵션은 `USAGE`이고 모르는 `--error`는 `USAGE`이며 `field`가 `--error`다. 표의 `USAGE`는 등록된 더 세분된 코드도 받는다. |
| `registry` | M | 코드가 유일하고 예약 코드의 종료 값이 계약 값이며 명령의 `errors`가 레지스트리에 있다. |
| `reasons` | M | reason이 UPPER_SNAKE이고 코드 안에서 유일하며 `--error`와 `--errors`가 같은 reason을 싣는다. reason 항목에는 종료 값이 없다. |
| `reasons-reserved` | S | 예약 situation의 reason이 예약 이름을 쓴다. |
| `warnings` | M | 도구가 `schema --warnings`를 제공하면 `--warnings`와 `--warning`이 같고 코드가 유일하고 모르는 코드는 `USAGE`이며 양쪽 레지스트리의 `meaning`이 같다. |
| `reserved-applicable` | M | 명령이 제공하는 기능의 예약 코드를 `errors`에 가진다. |
| `codes-known` | M | 어떤 검사에서 관찰한 모든 실패 코드가 그 명령의 `errors`에 있다. |
| `groups-not-runnable` | M | 모든 그룹이 `COMMAND_REQUIRED`와 자기 명령 목록을 `expected`로 낸다. |
| `grammar` | M | 낱말이 kebab-case이고 `-id`로 끝나는 옵션이 없고 예약 옵션이 예약 모양이다. |
| `depth` | S | 경로가 두 낱말을 넘지 않는다. |
| `one-meaning` | M | 같은 옵션 이름이 모든 명령에서 같은 `valueName`과 `valueType`을 가진다. |
| `one-meaning-values` | S | 같은 열거 옵션 이름의 값 집합이 서로소이면 알린다. |
| `help-line` | M | `--help`의 첫 줄이 `schema`를 가리킨다. |
| `defaults` | S | `--help`에 `[default: X]`가 있으면 명령 문서의 기본값과 같다. |
| `deterministic` | S | 트리와 모든 명령 문서가 두 번 불러도 바이트가 같다. |
| `examples` | M | 모든 `examples`가 도구 이름으로 시작하고 계약 9.12 문법으로 파싱되고 도메인, 필수, 제약을 만족하며 자리표시자가 없다. 안내 문서의 명령 줄은 `L2.guide-commands`가 같은 방식으로 파싱한다. |
| `example-runs` | M | `read` 명령의 모든 예시를 fixture 상태에서 돌려 종료 값 0이나 등록된 코드가 나오고 `USAGE`는 없다. |
| `hint-command` | S | 도구 이름과 공백으로 시작하는 힌트가 `schema` 문서의 실제 명령 호출로 파싱된다. 명령 낱말, 옵션 이름, 9.12 문법을 보고, `<UPPER_CASE>` 자리표시자의 값은 도메인과 제약으로 검사하지 않는다. 쪼개기는 `L0.hint-form`이 소유한다. 1.1 문서를 가진 도구에만 돌고 1.0 도구에는 돌지 않는다. |

- 검사 id마다 소유하는 판정은 하나다. 검사 하나가 다른 검사의 규칙을 다시 단언하지 않는다.
- 통과하지 못한 문서와 위반 규칙, 경로(명령 id)를 보고에 싣는다.

### F3 인수 검사

- `constraints`: `read` 명령과 fixture가 올린 쓰기 명령마다 규칙 하나만 어기는 호출을 만든다. 필수와 다른 제약은 만족한다. fixture의 통과 호출에서 가장 적은 인수를 바꾼다.
  - 그 호출은 선언한 `code`(없으면 `USAGE`)로 exit 2 거절되어야 한다.
  - 위반하면 다른 선언된 규칙도 깨지는 규칙은 `entailed`이고 깨지는 규칙을 이름으로 보고한다.
  - 위반 호출이 같은 집합인 동등한 규칙들은 한 시험 의무로 묶고, 코드가 다르면 실패한다.
- `domains`: 도메인이 있는 인수마다 통과 호출에서 그 하나만 바꿔 경계 바로 안과 밖의 값을 낸다.
  - 경계는 `minimum - 1`, `maximum + 1`, 배타 경계, `multipleOf`의 비배수, 길이 한계, 타입이 다른 JSON 값이다.
  - 안은 받고 밖은 exit 2와 `USAGE`나 그 인수의 코드, `field`로 거절되어야 한다.
  - `pattern`과 구조 키워드는 fixture의 `domainRejects` 값을 쓴다.
- `patterns`: 모든 `pattern`이 제한된 I-Regexp 형식이고 ECMAScript `u`와 Rust `regex`의 기본 한도에서 모두 컴파일되어야 한다.
  - `domainRejects`와 fixture가 준 수용 값에 두 엔진이 같은 답을 내야 한다.
  - 다른 답은 유니코드 판 차이일 때만 통과한다. 한 코드 포인트이고 패턴이 카테고리를 쓰며 두 엔진이 그 카테고리를 다르게 알 때다. 보고가 두 판과 코드 포인트를 적는다.
  - 그 밖의 불일치는 실패다.
- `numbers`: `integer` 인수에 `9007199254740992`와 `-9007199254740992`를 주면 exit 2로 거절되고 반올림된 값으로 받아들여지지 않는다.
- `validators`: 모든 검증자의 `rejects` 호출을 `setup` 없이 만든 fixture 환경의 사본에서 돌린다. exit 2와 선언한 `code`가 나오고 `field`가 검증자의 `arguments` 중 하나이고 아무것도 쓰이지 않는다.
  - 검증자는 상태를 읽지 않고 모든 선행 조건보다 먼저 돈다(계약 10.8.3).
  - 저장소가 초기화되지 않았다는 코드처럼 선행 조건 코드가 검증자의 코드 대신 나오면 실패다.
- 위 검사는 `read` 명령과 fixture의 `commands`에 올라 있는 쓰기 명령에만 호출을 만든다. 올라 있지 않은 쓰기 명령은 이 검사들이 제공되지 않음이다.

### F4 데이터 검사

- `output-data`: fixture의 `ok` 호출과 모든 `read` 명령 예시의 `data`가 `meta.output`이 가리키는 출력(하나뿐이면 그것)의 스키마를 통과하고 `meta.schemaVersion`이 그 출력의 판과 같다.
- `next`: 관찰한 모든 `meta.next` 항목의 `argv` 첫 낱말이 실제 명령이다. `read` 명령이면 같은 시퀀스에서 실행해 `USAGE`가 나오지 않는다. 자리표시자, `--json`, 쪼개기는 `L0.next`가 소유한다.
- `paging`: 쪽 나누는 명령을 `--limit 1`로 부른다. 어느 명령이 쪽을 나누는지는 명령 문서가 `--cursor`를 선언하는지로 정한다. fixture의 `paged` 호출이 먼저 쓰이고, 없으면 관찰한 `nextCursor`를 낸 호출을 쓴다.
  - `meta.page.returned`가 1 이하이고 `L0.page`가 찾은 다음 쪽 항목을 실행하면 다음 항목이 온다.
  - 같은 cursor에 필터 인수를 바꿔 주면 exit 2, `CURSOR_INVALID`, `field`가 `--cursor`다.
  - 쪽 나누는 명령이 있는데 `paged` 호출도 관찰한 `nextCursor`도 없으면 제공되지 않음이다. 어떤 명령 문서도 `--cursor`를 선언하지 않으면 해당 없음이다.
- `fields`: fixture가 준 선택(중첩 경로와 `[]` 경로 포함)마다 다음을 확인한다.
  - `meta.fields`가 선택과 `keep`의 합이다.
  - 고르지 않은 멤버가 어떤 깊이에도 없고, 선택한 멤버는 투영하지 않은 같은 호출에 있을 때마다 있다.
  - 투영한 `data`가 `required`를 줄인 스키마를 통과한다.
  - 없는 경로는 exit 2, `USAGE`, `field`가 `--fields`다.
- `paging`과 `fields`는 도구가 그 옵션을 제공할 때만 돈다. 제공하지 않으면 해당 없음이다.

### F5 fixture

- L1에서 읽는 키가 늘어난다. `commands`의 항목은 명령 id를 키로 하고 `ok`(통과 호출 목록), `missingValue`, `paged`(`nextCursor`를 돌려주는 호출 목록), `fields`(선택 목록), `domainRejects`(인수별 값 목록)를 가진다. `search`는 L2 키라 무시한다.
- `prepare`는 L0의 키와 같다. 셸 없는 argv 배열 목록이고 `setup` 전에 템플릿에서 한 번 돈다. 부작용 기준선은 `prepare`와 `setup` 뒤에 잡고, `prepare`는 환경만 만든다. 0이 아닌 종료는 검사기 오류다. L1은 `prepare`가 만든 환경에서 `validators`가 `setup` 없이 돌 때도 같은 환경을 쓴다.
- `registry`에는 `"from-schema"`를 쓸 수 있다. 그러면 오류와 경고 레지스트리를 도구의 `schema --errors --json`과 `--warnings --json`에서 가져오고 fixture와 비교하지 않는다. canon의 fixture는 이 값을 쓸 수 있다.
- 필요한 항목이 없고 관찰한 위반도 없으면 해당 검사는 제공되지 않음이고, MUST이면 레벨이 통과하지 못한다. 항목이 없어도 이미 관찰한 위반은 실패다.
- 검사기는 fixture에 올라 있지 않은 `write`나 `destructive` 명령을 돌리지 않는다.

### F6 시험

- 가짜 도구의 위반 설정이 L1 검사 id 31개마다 하나 이상 있다. 각각 소유 검사 하나만 실패하고 나머지는 통과하거나 해당 없음이다. 예는 다음과 같다.
  - 같은 `atMostOne`을 두 번 적으면 `walk`, 선언한 제약을 강제하지 않으면 `constraints`.
  - `--limit 0`을 받으면 `domains`, `9007199254740993`을 받으면 `numbers`, `^[a&&b]+$`를 선언하면 `patterns`.
  - 예시가 제약을 어기면 `examples`, 보고서에 필수 필드가 빠지면 `output-data`, `--fields`가 고르지 않은 멤버를 남기면 `fields`.
  - 힌트가 없는 명령을 가리키면 `hint-command`(권고만), 선행 조건 코드를 검증자보다 먼저 내면 `validators`.
- 정규식 시험은 계약 저장소의 `scripts/regex-cases.json`을 쓴다. 두 엔진의 답이 그 기록과 같고, 기록된 유니코드 판 차이만 통과한다. 계약 저장소의 예시 `schema-*`는 `L1.tree`와 `L1.walk`가 통과시키고 `invalid/`의 L1 위반 예시는 거부한다. 사본은 태그 `v1.1.1`에 고정한다.
- 정상 가짜 도구는 L1의 적용되는 모든 검사를 통과한다. 검사기를 자기 자신에게 돌려도 통과한다. 검사기는 L0 도구이므로 `--level L0`이다.

### F7 canon에 대한 실행

- canon fixture가 L1 항목을 가진다. `commands`에 모든 `read` 명령의 호출과 일부 쓰기 명령, `fields` 선택, 시각과 id 인수의 `domainRejects`가 있다. fixture 준비가 검사가 읽는 기록을 만든다.
- CI 잡과 `just atip`이 `--level L1`을 돌린다. canon이 적용되는 모든 L1 검사를 통과해야 한다. 이 단계는 `canon-0-34-atip-1-1`, `-schema-doc`, `-constraints`, `-paging-fields`가 병합된 뒤에만 초록이 된다.
- 트리의 `level`은 이 잡이 통과한 뒤에 `L1`이 된다.

## Acceptance

- F1:
  - 정상 가짜 도구에 `--level L1 --contract 1.1`은 종료 코드 0이고 `data.checks`에 L0와 L1 검사가 있다. `--level L2`는 exit 2, `LEVEL_UNSUPPORTED`다.
  - 트리의 `level`이 주장과 다르면 `L1.tree`가 실패한다. `schema`가 없는 도구의 L1 주장도 `L1.tree` 실패다.
- F2:
  - 위반 문서마다 소유 검사 하나만 실패하고 보고가 명령 id와 규칙을 적는다. 예는 그룹이 명령 뒤에 오는 트리, `-id`로 끝나는 옵션, 중복 reason, 풀리지 않는 `schemaRef`다.
  - `schema`가 저장소 없이 답하지 않으면 `no-state`만 실패한다. 두 번의 답이 다르면 `deterministic`만 권고로 실패한다.
  - 실패 경로: 경고 레지스트리를 제공하지 않는 도구에서 `warnings`가 해당 없음이고, 제공하는데 양쪽의 `meaning`이 다르면 실패다.
- F3:
  - 선언했지만 강제하지 않는 제약, 경계를 받아들이는 도메인, 거절하지 않는 `rejects`, 반올림해 받는 정수는 각 소유 검사만 실패한다. 다른 규칙도 같이 깨지는 제약은 `entailed`이고 레벨이 통과한다.
  - `^[a&&b]+$`와 `^\p{L}{1,500}$` 패턴은 `patterns`가 실패한다. 기록된 유니코드 판 차이는 통과하고 보고가 두 판을 적는다. 그 밖의 불일치는 실패다.
  - 실패 경로: fixture에 올리지 않은 쓰기 명령은 돌리지 않고 해당 검사가 제공되지 않음이다.
  - 검증자의 `rejects`를 초기화하지 않은 환경에서 돌렸을 때 선행 조건 코드를 내는 가짜 도구는 `validators`만 실패한다.
- F4:
  - 필수 필드가 빠진 보고서는 `output-data`, 투영이 고르지 않은 멤버를 남기면 `fields`, 필터를 바꿔 쓴 cursor를 받아들이면 `paging`이 실패한다.
  - 실패 경로: `--fields`나 쪽 나누기를 제공하지 않는 도구에서 두 검사가 해당 없음이다. 쪽을 나누는 명령이 있는데 `paged` 호출도 관찰한 `nextCursor`도 없으면 `paging`은 제공되지 않음이다.
- F5:
  - fixture에서 필요한 `commands` 항목을 빼면 해당 MUST 검사가 제공되지 않음이고 종료 코드는 1이다. 관찰한 위반은 실패다.
  - 모양이 틀린 fixture는 exit 2, `FIXTURE_INVALID`다.
  - 실패 경로: 0이 아닌 종료 값으로 끝나는 `prepare`는 exit 2, `PREPARE_FAILED`이고 어떤 검사 결과도 만들지 않는다. `prepare`가 만든 파일은 효과 판정(`L1.validators`의 아무것도 쓰이지 않는다는 조건 포함)에서 변화로 세지 않는다.
- F6:
  - ==**L1 검사 id 31개 모두에 대해 위반 설정 시험이 있다.**== 그 검사 하나만 실패한다.
  - 정규식 기록의 모든 사례에서 두 엔진의 답이 기록과 같다. 정상 가짜 도구와 검사기 자신이 통과한다.
  - 실패 경로: 검사 하나를 항상 통과하도록 바꾸면 그 검사의 위반 시험이 실패한다.
- F7:
  - canon fixture가 L1 항목을 가지고 CI 잡이 L1을 돌려 통과한다. 트리의 `level`이 `L1`이다.
  - 실패 경로: canon 명령 하나의 `examples`를 파싱되지 않게 바꾸면 CI 잡이 `examples` 실패로 빨개진다.

## Questions and assumptions

- 계약 저자가 정했다. `atip check`는 L1을 연다. `L1.patterns`의 ECMAScript 엔진은 `regress` 크레이트 같은 것을 이 변경에서 검토한다.
- 가정: 1.1 주장의 L1에서 적용되는 L1 검사는 31개이고 MUST 25개, SHOULD 6개다. 검사기 문서 v1.1.1의 L1 표를 센 것이다. canon은 경고 레지스트리, 쪽 나누기, `--fields`를 모두 제공하므로 조건부 검사 셋이 모두 적용된다.
- 계약 저자가 정했다. 계약 1.1.1은 패치이고 `meta.contractVersion`은 `"1.1"`로 남는다. 1.1.0 검사기를 통과한 도구도 1.1.1 규칙을 이미 지켜야 했다. 단 `L1.validators`는 `setup` 없이 돌고, `L0.page`와 `L1.paging`은 `paged` 호출이 없으면 제공되지 않음이다.
- 가정: 정규식 엔진은 `regress`를 먼저 평가한다. ECMAScript `u`와 맞는지는 계약 저장소의 기록(`regex-cases.json`)으로 증명한다. 안 맞으면 다른 엔진으로 바꾸고 이유를 보고한다. 계약 저자의 답이 정한 범위다.
- 가정: 검사기는 canon의 쓰기 명령 일부만 fixture에 올린다. 올리지 않은 쓰기 명령의 제약, 도메인, 검증자는 canon 자신의 테스트(`constraints_hold`)가 증명한다. 계약의 소유 분담과 같다.
- 가정: 검사 id마다 소유하는 판정이 하나이므로 L1에서도 한 위반은 한 검사만 실패시킨다.
- 가정: L2 검사는 0.35에 온다. `L2.input-usage`도 그때다.
- 정책 질문은 없다. CI 잡의 레벨 주장은 gate가 요구하는 것을 바꾸지 않는다.

## Out of scope

- L2 검사(0.35)와 L2 fixture 키.
- 플랫폼별 바이너리 배포(npm), crates.io 게시, atip 레포로의 이동.
- 보고만 하고 실패하지 않는 모드, lute 같은 다른 도구에 대한 실행.
- 계약 문서, 스키마, 예시의 수정. 사본을 읽기만 한다.
- canon이 L1을 만족하게 하는 일(`canon-0-34-atip-1-1`, `-schema-doc`, `-constraints`, `-paging-fields`).

Approved: Journey Hwang 2026-11-01T11:53:57Z — "승인" (relayed by orchestrator orchestrator-main via ask) (brief sha256:1579985b4e7c3cc5283dc39e88fd35abcb24d50edd8ec47a7de5c102974d7643)
