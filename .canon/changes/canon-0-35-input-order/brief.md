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

# canon의 --input 문서가 spec 11.7의 순서로 처리된다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 11.2-11.9, approaches/rust.md §6, approaches/migration.md §3.1 4번, issue 134

## Intent

`--input`으로 준 문서는 계약 11.7이 정한 순서(읽기와 틀 잡기, 조각 검증, 충돌, 합치기, 환경과 기본값, 호출 검증)로 처리되어야 두 도구가 한 입력에 같은 코드와 `field`를 낸다. canon은 이미 argv를 lexer로 읽고 clap에 `--name=value`로 넘기는 방식이라 rust.md §6이 경계하는 "argv만 먼저 clap으로 파싱"은 하지 않는다. 그러나 조각 검증이 첫 문제만 보고하고, 열거형 값 위반을 `USAGE`로 내고, 충돌 검사가 argv의 파싱 실패보다 앞서고, 정수 표기를 글자로 비교하고, NDJSON의 `#` 줄을 건너뛴다. `L2.input-refusals`, `L2.input-normalise`, `L2.input-conflict`가 canon에서 실패한다.

## Desired outcome

- 한 문서의 모든 조각 검증 문제가 문서의 키 순서로 보고된다. 첫 문제가 오류의 `field`이고 문제가 여럿이면 `details`가 있다.
- 열거형 값 위반이 `INPUT_INVALID`와 그 키의 포인터이다.
- ==**argv의 파싱 실패가 충돌 검사보다 먼저 답한다.**==
- 정수 표기는 정규화한 값으로 비교한다. 문서의 정수 문자열은 계약의 정수 문법을 따르고, argv의 정수 표기는 지금 그대로 둔다.
- NDJSON의 비어 있지 않은 모든 줄이 문서이고 `#` 줄을 건너뛰지 않는다.

## User flow

1. F1 에이전트는 NDJSON 배치를 준다 → 비어 있지 않은 줄이 모두 문서이다 → 주석 줄을 쓰지 않고 JSON만 쓴다.
2. F2 에이전트는 틀린 키가 여럿인 문서를 준다 → 모든 문제가 키 순서로 `details`에 온다 → ==**한 번에 고친다.**==
3. F3 에이전트는 열거형에 없는 값을 준다 → `INPUT_INVALID`와 그 키의 포인터가 온다 → 값을 고른다.
4. F4 에이전트는 argv의 값이 틀린 채 문서도 준다 → 먼저 argv의 파싱 실패가 답한다 → argv를 고친다.
5. F5 에이전트는 같은 정수를 `1`과 `"1"`과 `-0`과 `0`으로 쓴다 → 같은 값으로 비교되어 충돌이 없다 → 표기를 신경 쓰지 않는다.
6. F6 에이전트는 환경 변수가 거절할 값이어도 문서가 그 키를 준다 → 환경 변수는 무시된다 → 합친 인수에서 빠진 것만 환경에서 채워진다.

## Step detail

### F1 틀 잡기

- 소스는 먼저 JSON 값 하나로 파싱한다. 객체는 한 호출이다. 배열은 항목 하나여도 배치이다. 파싱이 실패하고 소스가 파일이나 stdin이면 비어 있지 않은 줄을 각각 NDJSON 문서로 파싱한다. 그것이 배치이다.
- ==**`#`로 시작하는 줄은 건너뛰지 않고 문서로 파싱한다.**== 파싱 실패면 `INPUT_INVALID`, 이유 `NOT_JSON`이다. 이유의 이름과 `field` `--input`은 지금과 같다.
- 객체나 배열이 아닌 JSON 값, 비어 있는 소스, 읽을 수 없는 파일, 크기 초과, 터미널 stdin이 모두 지금처럼 거절된다. 이 변경은 그 목록을 바꾸지 않는다.

### F2 조각 검증

- 각 문서를 조각 스키마로 검증한다. 모르는 키, flag-only 키, 타입, 열거형 값, `null`이 대상이다.
- 문서의 모든 문제를 보고한다. 순서는 문서에 쓰인 키 순서이다. 지금은 키가 알파벳 순으로 처리되어 문서의 순서와 다르다.
- 오류는 `INPUT_INVALID`, `field`가 첫 문제 키의 포인터(`/reasn`)이다. 문제가 둘 이상이면 `details`에 문제마다 `{field, message, reason?}`가 있다. 문제 하나뿐이면 `details`가 없다.
- 지금은 첫 문제만 보고하고 `details`가 없다. 조각 스키마는 `required`를 갖지 않는다. 0.34가 이미 맞다. 이 변경은 그 스키마를 실제 검증에 쓴다.

### F3 열거형

- 열거형에 없는 값은 조각 검증에서 `INPUT_INVALID`, 이유 `INVALID_VALUE`, `field`가 그 키의 포인터(`/disposition`)이다. 지금은 argv 파싱 단계의 `USAGE`로 나온다.
- argv로 준 열거형 값의 거절은 지금처럼 `USAGE`이다. 코드는 입력의 출처로 갈린다.

### F4 충돌 검사

- 3단계는 먼저 argv를 추출한다. 요구사항, 제약, 환경, 기본값은 강제하지 않는다.
- 파싱 실패(모르는 옵션, 값 타입이 틀린 값)는 이때 그 argv 코드로 거절한다. `--round abc --input '{"round":1}'`은 `USAGE`이고 `field`가 `--round`이다. 지금은 `INPUT_CONFLICT`가 나온다.
- 파싱 실패가 없으면 argv와 문서가 모두 준 인수를 정규화한 값으로 구조적으로 비교한다. 스칼라는 한 값으로, 반복 옵션과 피연산자는 순서 있는 배열로 비교한다. 다르면 `INPUT_CONFLICT`, `field`가 문서 포인터이다.
- 이 단계가 clap으로 argv만 완전히 파싱하는 것은 아니다. 지금의 lexer 방식을 유지한다.

### F5 정수 정규화

- 정수 인수의 문서 값은 JSON 정수이거나 `-?(0|[1-9][0-9]*)` 문자열이다. 둘 다 ±(2^53-1) 안이고 반올림하지 않는다. `-0`은 `0`이고 숫자 문자열 `"-0"`도 `0`이다. 비교는 정규화한 값으로 한다.
- 문서의 그 밖의 표기(`+1`, 16진수, `1e3`, 공백)는 `INPUT_INVALID`, 이유 `INVALID_VALUE`이다. 이 문법은 문서의 숫자 문자열에만 적용된다(spec 11.5).
- argv의 정수 표기는 계약이 정하지 않으므로 바꾸지 않는다. 지금 받아들이는 `+1`을 계속 받아들인다. 충돌 비교는 정규화한 값으로 하므로 argv `+1`과 문서 `1`은 충돌하지 않는다.
- 문서의 `1.0`처럼 정수가 아닌 JSON 수는 `INPUT_INVALID`이다. 지금은 `USAGE`가 나온다. canon에는 `number`와 `json` 값 타입의 인수가 없으므로 멤버 순서와 `1.0`과 `1` 같은 표기 변형은 정수에서만 시험된다.

### F6 환경과 기본값

- 5단계의 환경 변수와 기본값은 합친 인수에서 빠진 옵션에만 적용된다. 문서가 준 키는 환경 변수와 충돌하지도 환경 변수로 검증되지도 않는다.
- 환경 변수와 기본값은 명시한 것이 아니므로 충돌이나 제약의 "준" 인수에 들지 않는다(spec 11.7 5단계).
- 환경 변수로 채운 값의 검증 순서는 `canon-0-35-canon-check-order`가 정한다.

## Acceptance

- F1:
  - NDJSON 파일의 `# 주석` 줄은 `INPUT_INVALID`, 이유 `NOT_JSON`이다. 빈 줄은 건너뛴다. 비어 있지 않은 줄이 모두 문서로 처리되어 배치가 된다.
  - 실패 경로: 객체도 배열도 아닌 값, 읽을 수 없는 파일, 크기 초과가 지금처럼 거절된다.
- F2:
  - `{"roundd":1,"seq":"x"}`처럼 문제가 둘인 문서가 `details`에 두 항목이 문서 순서대로 있고 오류의 `field`가 첫 문제의 포인터이다.
  - 실패 경로: 문제가 하나뿐인 문서는 `details`가 없다. 모든 키가 맞는 문서는 거절되지 않는다.
- F3:
  - ==**`{"disposition":"nope"}`가 `INPUT_INVALID`, 이유 `INVALID_VALUE`, `field` `/disposition`이다.**== argv의 `--disposition nope`는 지금처럼 `USAGE`이다.
- F4:
  - `--round abc --input '{"round":1}'`이 `USAGE`, `field` `--round`이다. 값이 틀린 `--disposition bogus`와 문서도 같다.
  - 같은 인수를 argv와 문서가 다른 값으로 주면 `INPUT_CONFLICT`와 문서 포인터이다.
  - 실패 경로: argv만 있는 호출과 문서만 있는 호출은 충돌 검사의 영향을 받지 않는다.
- F5:
  - argv의 `--round 1`과 문서 `{"round":"1"}`, 문서 `{"round":1}`이 충돌하지 않는다. 문서 `-0`과 argv `0`도 충돌하지 않는다. 숫자 문자열 `"-0"`도 같다.
  - 실패 경로: 문서의 `"+1"`, `"0x1"`, `"1e3"`, `" 1"`이 `INPUT_INVALID`, `INVALID_VALUE`로 거절된다. `9007199254740992`는 문서와 argv 모두에서 거절된다.
  - argv의 `--round +1`은 지금처럼 받아들여지고 문서 `1`과 충돌하지 않는다.
- F6:
  - 환경 변수가 정수 옵션이 거절할 값으로 설정되어도 문서가 그 키를 주면 결과가 환경 변수가 없을 때와 같다. 문서가 주지 않은 옵션은 환경에서 채워진다.
- `atip check`의 `L2.input-refusals`, `L2.input-normalise`, `L2.input-conflict`, `L2.input-env`, `L2.input-fragment`, `L2.input-pointer`, `L2.input-dash-values`가 canon에서 통과한다. 그 외에 `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 결정(계약 저자 답): 현재 계약은 argv의 정수 표기를 정하지 않는다. 11.5의 `-?(0|[1-9][0-9]*)`는 `--input`의 숫자 문자열에만 적용된다. 0.35는 argv의 `+1`을 계속 받아들인다. 거절은 계약 1.3 후보이다.
- 가정: 문서의 열거형 위반이 `USAGE`에서 `INPUT_INVALID`로 바뀌는 것은 spec 11.7 2단계("types and enum membership")에서 직접 나온다. 0.34의 `USAGE`는 clap으로 넘어간 뒤에 나오던 부수 효과였다.
- 가정: 조각 검증의 문제 순서는 문서에 쓰인 키 순서이다. 계약이 "all problems"만 말하고 순서를 정하지 않는다. 키 순서는 구현이 입력 순서를 보존할 때 가장 자연스럽다.
- 가정: 이 변경은 argv의 전체 clap 파싱을 하지 않는 현재 방식(lexer)을 유지한다. rust.md §6이 권하는 대로다.
- 정책 질문은 없다.

## Out of scope

- 배치의 포인터와 `BATCH_INCOMPLETE` 규칙(`canon-0-35-batch-pointers`).
- 6단계의 순서(`canon-0-35-canon-check-order`)와 `limit`, `cursor`, `all`의 flag-only(`canon-0-35-canon-cursor-1-2`).
- `number`와 `json` 값 타입 인수를 더하는 일.
- 입력 문서의 새 키와 조각 스키마의 모양 변경.

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:e40e0408aa14f6f99ca2d5c21aca112072157163d85b3740329db23c36c23f70)
