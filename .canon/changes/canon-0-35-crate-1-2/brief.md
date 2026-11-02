---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
---

# crates/atip: atip/1 1.2의 상수, STOPPED_MIDWAY 행, flag-only 집합, minor로 고르는 필드 규칙

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 7.2.1, 8.3.2, 10.13 규칙 8, 11.6, approaches/rust.md §3.1, issue 134 (첫 댓글 3과 4, 둘째 댓글 4)

## Intent

`atip` crate는 계약 1.1까지만 안다. 계약 판은 `CONTRACT_VERSION = "1.1"` 상수 하나이고, 예약 코드 표는 `atip-check`의 `documents.rs`에만 있다. flag-only 옵션 집합은 canon과 검사기가 각자 따로 쓰고 둘 다 `limit`, `cursor`, `all`이 빠져 있다. 필드 선언 규칙은 `Rules { v11: bool }`이라 1.2를 말할 수 없다. 코드 행 `CodeInfo`는 상수 표 안에서 다른 행을 가리킬 수 없어, 이유가 "등록된 모든 exit 2 코드"인 `STOPPED_MIDWAY` 행을 쓸 수 없다. 세 번째 구현은 이 crate로 1.2 L0과 L1을 목표로 하는데 이 모두를 직접 만들어야 한다.

## Desired outcome

- crate가 1.0, 1.1, 1.2를 아는 `Minor`를 갖고, 가장 새로운 판을 `CONTRACT_VERSION = "1.2"`로 노출한다.
- ==**canon과 `atip-check`가 선언하는 판은 각자의 상수이고 이 변경으로 바뀌지 않는다.**== 이 변경이 canon의 출력에 1.2를 선언하게 하지 않는다.
- 예약 코드 표(이름, 종료 값, retryable, 처음 생긴 판)가 crate에 있고 `STOPPED_MIDWAY` 행이 1.2 이후로 들어 있다.
- flag-only 옵션 집합과 필드 선언 규칙이 판으로 정해진다.
- 상수 표 안에서 다른 코드 행에서 이유를 만들 수 있다.

## User flow

1. F1 도구 작성자는 `atip`의 판 상수를 읽는다 → 1.0, 1.1, 1.2를 가린 `Minor`와 가장 새로운 판이 있다 → 자기 선언을 자기 코드에 둔다.
2. F2 도구 작성자는 예약 코드 표를 읽는다 → 13개 코드가 종료 값과 처음 생긴 판과 함께 있다 → 자기 레지스트리를 계약 값과 맞춘다.
3. F3 도구 작성자는 flag-only 집합을 읽는다 → 판에 따라 4개나 8개이다 → ==**자기 옵션 표시에 쓴다.**==
4. F4 도구 작성자는 `STOPPED_MIDWAY` 행을 상수 표에 쓴다 → 이유가 다른 코드 행에서 만들어진다 → 이름과 뜻과 해결을 두 번 쓰지 않는다.
5. F5 도구 작성자는 필드 선언을 판으로 검증한다 → 1.0은 대괄호 키를 거절하고 1.1과 1.2는 받는다 → 선언한 판에 맞는 규칙을 받는다.

## Step detail

### F1 판

- `Minor`는 `V1_0`, `V1_1`, `V1_2`와 `parse`, `as_str`, `at_least`를 가진다. 낯선 판은 `parse`가 거절한다.
- `schema::CONTRACT_VERSION`은 `"1.2"`이다. 이 상수는 crate가 구현하는 가장 새로운 판이다. 도구가 선언하는 판이 아니다.
- `canon-cli`는 자기 선언을 `DECLARED_MINOR = "1.1"`로 두고 지금 `CONTRACT_VERSION`을 쓰는 두 곳(`json_out.rs`와 `spec.rs`)이 그것을 읽는다. `atip-check`의 `output.rs`도 같다. 두 도구의 출력이 바뀌지 않는다.
- 묶어 둔 계약 스키마는 `canon-0-35-check-1-2`가 갱신한다. 이 변경은 스키마 사본을 건드리지 않는다.

### F2 예약 코드 표

- ==**crate의 표는 이름, 종료 값, retryable, 도입한 판이다.**== 지금 열두 개와 `STOPPED_MIDWAY`(종료 값 1, 재시도 불가, 1.2)가 있다.
- `atip-check`의 `RESERVED_CODES`가 이 표로 옮겨 가고 검사기가 crate의 표를 읽는다. 검사기에 같은 표의 복제가 없다.
- 도구가 `STOPPED_MIDWAY`를 등록하더라도 예약 뜻은 1.2 문서에서만 성립한다. 1.1 이하의 문서에서는 도구 자신의 코드이다(spec 8.3.2). 표의 "도입한 판" 멤버가 이 구분을 싣는다.
- 예약 코드 표는 "도구가 이 코드를 레지스트리에 등록하면 이 종료 값과 retryable이어야 한다"를 확인하는 함수를 함께 낸다. 같은 확인을 검사기와 도구의 시험이 쓴다.

### F3 flag-only 집합

- crate가 `flag_only_options(minor)`를 낸다. 1.0과 1.1은 `json`, `input`, `commit`, `plan-digest`이고 1.2는 `limit`, `cursor`, `all`, `fields`가 더해진다.
- `project::option_doc`의 `flag_only` 인수는 그대로 받고, 도구가 이 집합에 자기 옵션(canon의 `repo`와 `as`)을 더해 넘긴다. crate는 도구의 옵션 이름을 모른다.
- 지금 canon의 `FLAGS_ONLY`와 검사기의 `FLAG_ONLY`는 이 변경이 바꾸지 않는다. canon은 `canon-0-35-canon-cursor-1-2`에서, 검사기는 자기 상수 그대로 쓴다.

### F4 이유를 다른 행에서 만들기

- 상수나 정적 표 안에서 다른 `CodeInfo`를 가리켜 `ReasonInfo`를 만들 수 있어야 한다. 이름은 그 코드, 뜻과 해결은 그 코드 행의 것이다.
- `STOPPED_MIDWAY`의 이유는 "도구의 모든 exit 2 코드"처럼 표의 다른 행을 훑어서 만들어진다. 순환 참조가 없고 표 순서에 의존하지 않는다.
- 구현 방식(const 함수, 매크로, 지연 평가)은 구현자가 고른다. 조건은 canon의 코드 표(230행)가 이것으로 `STOPPED_MIDWAY` 행을 쓸 수 있다는 것이다.

### F5 Rules

- `fields::Rules`는 `for_minor("1.2")`나 `for_minor(Minor)`로 만든다. 1.1과 1.2는 같은 규칙(대괄호 키, 맵 키, 컷 노드)이고 1.0은 `v11` 규칙이 없다. 낯선 판은 거절한다.
- 필드 `v11`은 공개가 아니다. 호출 지점은 `canon-cli`의 시험 두 곳이다.

## Acceptance

- F1:
  - `Minor::parse("1.2")`는 `V1_2`이고 `"1.3"`은 거절된다. `V1_2.at_least(V1_1)`은 참이다.
  - `CONTRACT_VERSION`이 `"1.2"`이다. `canon status --json`의 `meta.contractVersion`과 `atip check`의 봉투는 이 변경 뒤에도 `"1.1"`이다.
  - 실패 경로: 선언 상수를 지우면 두 도구의 컴파일이 실패한다. 두 도구가 crate의 `CONTRACT_VERSION`을 읽는 곳이 없음을 시험이 확인한다.
- F2:
  - ==**표가 13개이고 `STOPPED_MIDWAY`가 종료 값 1, 재시도 불가, 도입 판 1.2이다.**== 검사기의 소스에 같은 표의 복제가 없다.
  - 실패 경로: 종료 값 2로 등록한 `STOPPED_MIDWAY`가 1.2 문서에서 확인 함수에 걸리고 1.1 문서에서는 걸리지 않는다.
- F3:
  - `flag_only_options` 1.1은 4개, 1.2는 8개이다. `option_doc`에 1.2 집합을 넘기면 `limit` 옵션이 `flagOnly: true`이다.
  - 실패 경로: 1.2 집합에 들어 있는데 `flagOnly`를 쓰지 않은 옵션이 있으면 `schema` 문서 검증이 실패한다.
- F4:
  - canon의 코드 표에 `STOPPED_MIDWAY` 행을 시험용으로 써 보면 모든 exit 2 행이 이유로 나온다. 순서가 바뀌어도 결과가 같다.
  - 실패 경로: 표에 없는 코드를 이유로 만들려는 시도는 컴파일되지 않거나 시험에서 실패한다.
- F5:
  - `Rules::for_minor("1.0")`은 대괄호 키를 거절하는 규칙이고 `"1.1"`과 `"1.2"`는 허용한다. 기존 필드 시험이 모두 통과한다.
  - 실패 경로: `for_minor("1.9")`는 거절된다. `canon-cli` 시험이 새 API로 바뀌고 옛 `Rules { v11: .. }` 구성이 없다.
- `cargo test --workspace`, 생성 문서 drift 시험이 통과하고 canon과 `atip check`의 출력이 0.34와 바이트까지 같다.

## Questions and assumptions

- 가정: `CONTRACT_VERSION`은 crate의 가장 새로운 판이고 도구의 선언이 아니다. 저자의 이슈는 "CONTRACT_VERSION"을 1.2 항목으로 적었다. canon이 이 변경만으로 1.2를 선언하게 되는 일을 피하려고 선언을 도구에 두었다. rust.md §3.1이 말하는 "도구가 상수를 1.2로 둔다"는 도구의 선언을 뜻한다고 읽었다.
- 가정: 예약 코드 표를 crate로 옮기는 것은 이 변경에 넣는다. `STOPPED_MIDWAY` 행과 같은 표이기 때문이다. 검사기의 시험은 같은 결과를 낸다.
- 가정: 이유 만들기의 구현 방식은 구현자가 고른다. 저자의 이슈가 `ReasonInfo::from(&CodeInfo)` 같은 const 함수를 예로 들었다.
- 정책 질문은 없다.

## Out of scope

- canon이 1.2를 선언하는 일과 canon의 레지스트리에 `STOPPED_MIDWAY`를 등록하는 일(`canon-0-35-canon-stopped-midway`, `canon-0-35-claim-l2`).
- 쪽 나누기 cursor(`canon-0-35-crate-cursor`), 계획 digest(`canon-0-35-crate-plan`), `jsonschema` 기능 분리(`canon-0-35-crate-validate`).
- 계약 스키마 사본의 갱신(`canon-0-35-check-1-2`).

Approved: Journey Hwang 2026-11-02T06:38:09Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:2f87d3b8b7b52a8b27e68fb3ca70d19a4b8f8ac45d3ac07414cd5419b2da424f)
