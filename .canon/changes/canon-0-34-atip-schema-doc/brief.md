---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "트리, 그룹, 명령 문서의 모든 멤버와 생성 방식, 소비자 이동을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "일곱 단계가 각자 멤버 이름과 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "일곱 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "계약 저자에게 확인할 가정이 서로 다른 결정이라 함께 적어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "멤버 이름과 코드 이름은 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "멤버 이름은 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "멤버 이름, 옵션 이름, 코드 이름은 호출자가 읽는 철자 자체이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "멤버 이름, 옵션 이름, 코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "멤버 이름과 계약 번호는 결정 대상의 철자 자체다"
---

# schema 자기 서술을 atip/1 L1 문서로 낸다

Source: https://github.com/journeyWorker/atip tag v1.1.0, spec 4.7, 9, 10.1-10.7, 10.9-10.10, 10.12-10.13, approaches/rust.md §3-§5, approaches/migration.md §1 L1 표, §3.1 3번

## Intent

`canon schema`의 답은 계약의 L1 문서가 아니다. 목록은 명령 78개의 평평한 배열이고 그룹 27개가 없다. `level`이 없고, 그룹을 부르면 명령 문서 대신 접두사로 거른 목록이 온다. 명령 문서는 canon 전용 이름(`nodeKind`, `about`, `errorCodes`, `inputs`, `exits`)을 쓴다. 옵션은 값 종류가 `flag`와 `value` 둘뿐이고, 기본값은 문자열이거나 `[]`다. 보고서 스키마는 저장소의 명세 파일에만 있고 실행 중의 답에는 없다. 루트 `--help`의 첫 줄은 `schema`를 가리키지 않는다. 그래서 에이전트는 canon의 옵션 타입과 보고서 모양을 `schema`만으로 알 수 없다.

## Desired outcome

- `canon schema --json`은 그룹과 명령의 트리와 `level`을 낸다. 그룹 문서와 명령 문서가 계약의 이름과 모양을 따른다.
- 명령 문서가 옵션과 인수의 값 타입, 타입 있는 기본값, 플래그 전용 표시, 오류 코드, 출력 스키마, 입력 정보, 예시를 싣는다. 보고서 스키마가 실행 중의 답에 들어 있다.
- ==**`schema`의 답은 그 명령이 실제로 하는 일과 같은 원천에서 만들어지고, 어긋나면 테스트가 실패한다.**==
- 루트 `--help`의 첫 줄이 `schema`를 가리킨다.

## User flow

1. F1 에이전트는 `canon schema --json`을 부른다 → 트리가 `data.kind: "tree"`, `level`, 그룹과 명령의 항목으로 온다 → 어떤 명령이 있는지 안다.
2. F2 에이전트는 `canon schema <그룹> --json`이나 `canon schema "finding close" --json`을 부른다 → 그룹 문서나 명령 문서가 온다 → 모르는 경로와 겹친 옵션은 `expected`와 함께 거절된다.
3. F3 에이전트는 명령 문서의 옵션과 인수를 읽는다 → 값 타입, 필수, 반복, 타입 있는 기본값, 열거 값, `env`, `flagOnly`가 있다 → 호출을 직접 구성한다.
4. F4 에이전트는 명령 문서의 `errors`, `outputs`, `schemas`를 읽는다 → 예약 코드 포함 오류 목록과 출력마다 보고서 스키마가 있다 → 결과를 해석한다.
5. F5 에이전트는 명령 문서의 `input`과 `examples`를 읽는다 → `--input`의 조각 스키마와 완전한 예시가 있다 → 입력 문서를 쓰고 예시를 그대로 실행한다.
6. F6 에이전트는 `canon --help`를 읽는다 → 첫 줄이 `schema`를 가리킨다 → 자기서술이 있음을 안다.
7. F7 에이전트와 사람은 canon의 명세 파일, 스킬, 문서를 본다 → 새 이름과 모양이다 → ==**옛 이름이 남은 곳이 없다.**==

## Step detail

### F1 트리

- `data`는 `kind: "tree"`, `level`, `commands`다. `level`은 canon이 주장하는 레벨이고, 0.34에서는 `L1`이다.
- `commands`는 그룹 27개와 명령 78개를 한 번씩 나열한다. 그룹이 자기 명령 앞에 오고, 순서는 실행마다 같다. 항목은 `command`, `path`, `kind`(`group`이나 `command`), `summary`이고 명령에는 `effect`가 있다. `schema`도 다른 명령처럼 나열된다.
- `--as <ROLE>`은 명시적 필터로 남는다. 그 역할의 명령과 그 명령을 가진 그룹만 남는다. 환경 변수는 답을 거르지 않는다. 역할 목록은 `x-canon.roles`에 있고, 옛 최상위 `roles`는 없다.
- 옛 평평한 목록의 `about`은 `summary`가 되고, 옛 `role` 멤버는 없다.
- `schema`는 저장소, 설정, 네트워크, 환경 변수 없이 답하고 아무것도 쓰지 않는다. 같은 빌드에서 답이 바이트 단위로 같다.
- `meta.schemaVersion`은 `"1.1"`이다.

### F2 그룹, 경로, 거절

- `canon schema <그룹>`은 `kind: "group"`, `command`, `path`, `summary`, `commands`(직접 자식 항목)다. 명령 경로는 `kind: "command"`인 명령 문서다.
- 경로는 낱말 여러 개나 따옴표로 묶은 한 문자열 모두 받고 둘의 답이 같다.
- 모르는 경로는 exit 2, `UNKNOWN_COMMAND`, `expected`가 처음 모르는 자리의 유효한 낱말이다. 옛 답에는 `expected`와 `suggestions`가 없었다.
- 경로, `--errors`, `--error`, `--warnings`, `--warning`을 겹쳐 주면 exit 2, `USAGE`다.
- 그룹을 `canon <그룹> --json`으로 실행하면 `COMMAND_REQUIRED`이고 `expected`가 그 그룹의 명령이다. 지금과 같다.

### F3 옵션과 인수

- 옵션 멤버는 `name`(`--` 없이), `valueType`, `required`, `repeatable`, `default`, `help`다. 값을 받으면 `valueName`, 열거형이면 `possibleValues`, 선택적으로 `defaultNote`, `env`, `flagOnly`, `short`가 있다. 인수 멤버는 `name`, `valueName`, `valueType`, `required`, `variadic`, `default`, `help`다. 옵션 이름이 `-id`로 끝나지 않는다.
- `valueType`은 `switch`, `count`, `string`, `integer`, `number`, `path`, `enum`, `json` 중 하나다. clap의 인수 동작과 값 파서 타입에서 읽는다.
  - `bool`은 `switch`, 정수형은 `integer`, `PathBuf`는 `path`, 닫힌 값 집합은 `enum`이다.
  - 사용자 정의 파서 26개(id 종류, 시각, `NAME@VERSION`, `FORMAT:PATH` 등)는 표에 값 타입을 적는다. 표가 없는 사용자 정의 파서가 있으면 생성이 실패한다. 추측으로 `string`을 쓰지 않는다.
- 기본값은 옵션의 타입으로 파싱한 값이다. `"100"`은 `100`이다. `--help`가 약속한 기본값은 모두 실린다. 지금 `--limit`의 도움말은 "default 100"인데 기본값은 `[]`다.
- ==**실행 시 계산되는 기본값은 `null`과 `defaultNote`다.**== 필수 옵션의 기본값은 `null`이고 스위치의 기본값은 `false`다.
- `flagOnly: true`는 `--input` 문서에 쓸 수 없는 옵션이다. `json`, `input`, `commit`, `plan-digest`는 항상 그렇다. canon이 이미 문서에서 거절하는 옵션 집합과 같다.
- `env`는 옵션이 읽는 환경 변수다. 지금은 `--actor`의 `CANON_ACTOR`다.
- 같은 이름의 옵션은 모든 명령에서 `valueName`과 `valueType`이 같다. `--limit`은 쪽 크기, `retrieve`, `search`에서 모두 정수 `N`이다.
- 옛 `kind`, `aliases`, `conflictsWith`, `requires`, `arity`는 없어진다. 관계는 `canon-0-34-atip-constraints`가 `constraints`로 싣는다.

### F4 오류, 출력, 스키마

- 명령 문서의 `errors`는 그 명령이 낼 수 있는 모든 코드다. 종료 값은 레지스트리에 있다. 명령이 제공하는 기능의 예약 코드가 들어 있다. `--input`이 있으면 `INPUT_INVALID`, `INPUT_CONFLICT`, `INPUT_TOO_LARGE`, 배치가 있으면 `BATCH_INCOMPLETE`, `--commit`이 있으면 `PLAN_STALE`, `--cursor`가 있으면 `CURSOR_INVALID`다. 옛 `exits`는 없다.
- `outputs`의 항목은 `kind`, `schemaRef`, `schemaVersion`이고 아티팩트는 `mediaType`, 배치 출력은 `batch: true`, 여러 출력은 `when`이나 `batch`로 서로 구분된다. 출력 둘이 같은 `schemaRef`를 쓰지 않는다. `brief approve`와 `init`이 이 경우다.
- `schemas`는 `schemaRef`들이 가리키는 모든 JSON Schema를 이름으로 담는다. 보고서 스키마의 `$defs`는 형제 이름으로 올리고 참조는 `#/schemas/<이름>`이며, 각 스키마의 자체 `$id`는 없다.
- 보고서와 아티팩트(`skills show`, `export`, `completion`, 승인 payload)가 각자 JSON 표현 타입과 스키마를 가진다. `dashboard`는 stream이 아직 아니므로 보고서 하나로 선언한다. stream 선언은 0.35 일이다.

### F5 입력과 예시

- `input`은 `--input`이 있는 명령 47개에 있다. 멤버는 `option`, `schemaRef`, `batch`, `maxBytes`, 배치면 `maxItems`, `example`이고 `argumentsRef`는 선택이다.
- 조각 스키마는 `--input` 문서 하나의 키와 인코딩된 값을 정한다. `additionalProperties: false`이고 루트에 `required`가 없고 플래그 전용 옵션이 없다. 정수는 정수나 숫자 문자열을 받는다. 지금 47개 중 30개가 `required`를 가진다. 그 스키마는 합친 뒤의 인수 객체를 설명하므로 `argumentsRef`가 되고, `required`가 없는 조각 스키마가 새로 생긴다.
- `example`은 완전하고 유효한 문서 하나다. 이 변경은 `--input`의 처리 순서를 바꾸지 않는다. 조각 문서만 받는 입력 경로는 0.35 일이다.
- `examples`는 명령마다 하나 이상이고, 도구 이름으로 시작하는 완전한 호출이다. `--json`이 경로 뒤에 있고, 자리표시자가 없고, 필수 인수와 제약을 만족한다. 파일이나 stdin을 읽는 `--input`은 쓰지 않는다. 모든 예시가 실제 파서로 파싱된다는 것을 테스트가 확인한다.

### F6 도움말 첫 줄

- 루트 `canon --help`의 첫 줄은 `Agents: "canon schema --json" describes every command as JSON.`이다. 그룹과 명령의 `--help`도 같은 줄로 시작한다. 지금의 첫 줄(소개 문장)은 그 줄 아래로 간다.
- `--help`가 보이는 `[default: X]`는 명령 문서의 기본값과 같다. 테스트가 모든 옵션에서 대조한다.

### F7 소비자와 생성물

- 명세 파일은 이 답들과 같은 생성기로 만들고, 명령마다 답과 바이트가 같다. 파일의 명령 목록 구조도 트리와 명령 문서로 바뀐다.
- 새 이름으로 옮기는 곳은 다음과 같다.
  - 명세 파일을 읽는 테스트와 생성 문서. 스킬, 웹사이트 영어와 한국어의 CLI 표 28개씩, CHANGELOG가 해당한다.
  - `canon context`, dogfood 평가 스크립트, 설계 문서, conformance 케이스.
  - 스킬의 `schema` 사용 설명.
- 옛 이름(`nodeKind`, `errorCodes`, `supportsJson`, `about`, 옛 `inputs`, 옛 `exits`, `arity`)은 안내와 문서에서 없어진다. CHANGELOG의 대체표는 예외다.
- clap 트리를 읽어 명세로 투영하는 공통 부분(값 타입, 기본값 파싱, 제약 추출, 트리)은 `crates/atip`에 둔다. 이 crate는 `canon-*`에 의존하지 않는다. canon 전용 표(`effect`, 멱등성, 오류 코드, 출력, 역할)는 `canon-cli`에 남는다.

## Acceptance

- F1:
  - ==**`canon schema --json`의 `data.kind`가 `tree`이고 `level`이 `L1`이다.**== 그룹 27개와 명령 78개가 한 번씩 있고 그룹이 자기 명령 앞에 온다. 두 번 부르면 바이트가 같다.
  - `canon schema --as reviewer --json`은 `reviewer`의 명령과 그 그룹만 낸다. `CANON_ROLE=reviewer`는 답을 바꾸지 않는다.
  - 실패 경로: 저장소 밖 빈 디렉터리에서도 `schema`가 답하고 아무것도 쓰지 않는다.
- F2:
  - `canon schema finding --json`은 `kind: "group"`이고 직접 자식 3개를 낸다. `schema "finding close"`와 `schema finding close`의 답이 같다.
  - `canon schema finding nosuch --json`은 exit 2, `UNKNOWN_COMMAND`이고 `expected`가 `finding`의 명령이다.
  - 실패 경로: `schema finding --errors`는 exit 2, `USAGE`다. `canon finding --json`은 `COMMAND_REQUIRED`다.
- F3:
  - `canon schema finding close --json`에서 `round`의 `valueType`이 `integer`이고, `evidence add`의 `--max-artifact-mib`의 `default`가 숫자 `25`다. `--limit`이 있는 쪽 나누기 명령의 기본값이 `100`이다. `--actor`에 `env`가 `CANON_ACTOR`다.
  - `json`과 `input`이 `flagOnly: true`이다. `-id`로 끝나는 옵션 이름이 없다.
  - 실패 경로: 표에 없는 사용자 정의 파서를 가진 인수를 추가하면 명세 생성 테스트가 실패한다.
- F4:
  - `--input`이 있는 모든 명령의 `errors`에 `INPUT_INVALID`, `INPUT_CONFLICT`, `INPUT_TOO_LARGE`가 있고 `--cursor`가 있는 명령에는 `CURSOR_INVALID`가 있다. 명령의 `errors`에 없는 코드를 낸 호출이 없음을 테스트가 확인한다.
  - `brief approve`의 출력이 서로 다른 `schemaRef` 둘과 배치 출력을 가지고, `schemas`가 그 이름들을 모두 담는다. 모든 `schemaRef`가 `schemas`에서 풀린다.
  - 실패 경로: 풀리지 않는 `schemaRef`나 `schemas`의 컴파일되지 않는 스키마가 있으면 테스트가 실패한다.
- F5:
  - 47개 `input`의 조각 스키마에 루트 `required`와 플래그 전용 속성이 없다. `example`이 조각 스키마를 통과한다.
  - 모든 `examples`가 실제 파서를 통과하고 자리표시자가 없다.
  - 실패 경로: 파싱되지 않는 예시를 넣으면 테스트가 실패한다.
- F6:
  - `canon --help`와 `canon finding --help`의 첫 줄에 `schema`가 있다. 모든 `[default: X]`가 명령 문서의 기본값과 같다.
- F7:
  - 명세 파일이 명령마다 `canon schema <명령> --json`의 `data`와 같다. 옛 이름 검사 테스트가 안내, 웹사이트, 평가 스크립트에서 통과하고 하나를 되살리면 실패한다.
  - `cargo test --workspace`, conformance, 생성 문서 drift 테스트가 통과한다. 실패 경로: 소비자 한 곳이 옛 이름을 읽으면 그 테스트가 실패한다.

## Questions and assumptions

- 계약 저자가 정했다. `schema --search`는 이 변경이 아니다. 명령이 약 50개를 넘는 도구에는 SHOULD이지만 검사의 MUST가 아니다. canon은 78개이므로 알려진 SHOULD 공백으로 backlog에 둔다.
- 가정: 이 변경의 `schema` 답에 `argumentsRef`를 싣는다. 지금 `required`를 가진 입력 스키마 30개를 그대로 옮기는 것이 가장 싸다. 계약상 `argumentsRef`는 선택이다.
- 가정: 같은 이름 `--limit`의 세 가지 쓰임(쪽 크기, `retrieve` 개수, `search` 개수)은 모두 "돌려줄 최대 개수"라 계약 9.6의 한 의미로 본다. 계약 저자가 다르게 보면 이름을 바꾸는 별도 변경이다.
- 가정: 새 이름을 정할 때 옛 이름과 별칭을 두지 않는다. 0.33과 같은 깨지는 릴리스 방식이다.
- 가정: 보고서 데이터(`data`)의 모양과 `schemaVersion`은 이 변경에서 바뀌지 않는다. 투영 규칙 때문에 바뀌는 보고서는 `canon-0-34-atip-paging-fields`가 다룬다.
- 계약 저자가 정했다. `dashboard`는 `--json`이 봉투 하나를 내므로 보고서 출력(10.9.1)이다. stream은 NDJSON 이벤트 줄에만 해당한다(13.5). 이 선언을 유지한다.
- 가정: canon은 이 릴리스의 모든 변경이 병합되고 `atip check --level L1`이 통과한 뒤에 트리의 `level`을 `L1`로 주장한다. 순서는 `canon-0-34-atip-1-1`, 이 변경, `canon-0-34-atip-constraints`, `canon-0-34-atip-paging-fields`, `canon-0-34-atip-check-l1`이다.
- 정책 질문은 없다. 명령 문서의 모양이 바뀌어도 gate가 요구하는 것과 승인자는 바뀌지 않는다.

## Out of scope

- 제약, 도메인, 검증자의 선언과 정수 범위(`canon-0-34-atip-constraints`).
- 쪽 나누기 cursor 묶기, `--fields` 투영과 `meta.fields`, 대괄호 경로(`canon-0-34-atip-paging-fields`).
- 1.1 채택의 `contractVersion`, reason, 경고 레지스트리(`canon-0-34-atip-1-1`).
- 검사기의 L1 검사(`canon-0-34-atip-check-l1`).
- `schema --search`(알려진 SHOULD 공백, backlog).
- L2(0.35): `--input` 처리 순서와 조각 병합, 배치 포인터, 계획 digest 형식, `--operation`, stream 선언(`dashboard`), `skills show`의 data 모양.

Approved: Journey Hwang 2026-11-01T06:36:14Z — "승인 (6개 모두 0.34, 깨는 변경 괜찮아)" (relayed by orchestrator orchestrator-main via ask) (brief sha256:cfa5f36c309494f7d5b055314cef3809eaf058f6b8f2b31ede1b4f2e30a029df)
