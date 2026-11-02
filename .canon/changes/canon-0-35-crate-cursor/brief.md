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
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
---

# crates/atip: atip::cursor가 판마다 cursor의 묶기, 부호화, 해독, 다음 쪽 argv를 정한다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 6.9와 15.10, approaches/typescript.md §3.8, issue 134 (둘째 댓글 2번)

## Intent

쪽 나누기 cursor의 묶기 규칙은 계약 판마다 다르다. 1.1은 `--limit`, `--cursor`, `--fields`만 뺀 모든 인수에 묶고, 1.2는 `--json`, `--limit`, `--all`, `--cursor`, `--fields`만 빼고 기본값을 명시한 인수와 환경 변수 값까지 묶는다. canon의 구현은 canon의 `paging.rs`에만 있고 `atip` crate에는 없다. 세 번째 구현은 같은 규칙을 직접 다시 짜야 한다. 그 결과 두 도구가 같은 인수로 서로 다른 cursor를 낸다.

## Desired outcome

- `atip::cursor`가 인수에서 묶기 값을 만들고, cursor를 불투명한 문자열 하나로 부호화하고, 해독해 검증하고, 다음 쪽의 argv를 만든다.
- 묶기 규칙은 판(1.1, 1.2)으로 정해진다. ==**같은 인수가 도구에 상관없이 같은 묶기 값을 낸다.**==
- 거절은 `CURSOR_INVALID`의 세 이유 `FOREIGN`, `ARGUMENTS_CHANGED`, `EXPIRED`로 구분된다.
- 이 변경은 crate만 바꾼다. canon의 `paging.rs`와 cursor 문자열은 `canon-0-35-canon-cursor-1-2`가 옮긴다.

## User flow

1. F1 도구 작성자는 호출이 준 인수를 `atip::cursor`에 넘긴다 → 판에 맞는 묶기 값이 나온다 → cursor를 만든다.
2. F2 도구는 쪽 하나를 낸 뒤 cursor를 부호화한다 → 묶기 값과 위치가 불투명한 문자열 하나에 든다 → `meta.page.nextCursor`에 싣는다.
3. F3 호출자가 cursor를 준다 → 도구가 해독하고 현재 인수의 묶기 값과 비교한다 → ==**맞으면 위치를 얻고 틀리면 이유가 든 `CURSOR_INVALID`를 얻는다.**==
4. F4 도구는 다음 쪽 항목을 만든다 → 호출이 명시한 인수만 정규화해 싣고 `--cursor <값>`을 붙인다 → 같은 환경에서 그 항목이 이어진다.
5. F5 검사기 작성자는 두 판의 시험을 돌린다 → 1.1 도구와 1.2 도구가 계약의 예에서 서로 다르게 묶인다 → 판에 맞는 규칙을 시험으로 확인한다.

## Step detail

### F1 묶기 값

- ==**입력은 호출이 준 인수의 목록이다.**== 항목은 이름, 정규화한 값, 출처(명시, 환경 변수)와 도구 자신의 flag-only 옵션 표시(canon의 `repo`와 `as`)를 가진다. 기본값으로 채운 값은 입력에 없다.
- 1.1: `limit`, `cursor`, `fields`를 뺀 모든 명시 인수와, 호출이 준 flag-only 옵션(`json` 포함)이 묶인다.
- 1.2: `json`, `limit`, `all`, `cursor`, `fields`를 뺀 명시 인수와 도구의 flag-only 옵션이 묶인다. 기본값을 명시한 인수는 준 것이다. `false`인 스위치와 `0`인 카운트는 주지 않은 것과 같아 빠진다(spec 10.7.2). 환경 변수 값은 출처 표시와 함께 묶인다. 환경 변수를 켜거나 끄거나 바꾸거나 같은 값을 옵션으로 명시하면 묶기 값이 바뀐다.
- 묶기 값은 인수 객체를 RFC 8785(JCS)로 쓴 JSON의 SHA-256 소문자 16진 문자열 전체이다. 값은 정규화한 JSON 값이므로 `--round=1`과 `--round 1`은 같고 `1`과 `1.0`도 같다.
- 인수의 순서는 묶기 값에 영향이 없다. 반복 옵션은 값의 순서가 있다. 판이 1.0이면 1.1 규칙을 쓴다.

### F2 부호화

- cursor는 도구가 준 영역(명령 id), 묶기 값, 도구의 위치 문자열을 담은 ASCII 문자열 하나이다. 공백과 따옴표 없이 argv에 그대로 쓸 수 있다.
- 호출자는 그 문자열을 해석하지 않는다. 형식은 계약이 정하지 않으며 crate의 다음 판이 바꿀 수 있다.
- 위치는 도구의 문자열이다. crate는 그 의미를 모른다.

### F3 해독과 검증

- 해독은 세 가지 결과이다. 도구의 cursor가 아니면 `FOREIGN`이다. 영역이 다르면 `FOREIGN`이다. 묶기 값이 현재 인수의 것과 다르면 `ARGUMENTS_CHANGED`이다.
- `EXPIRED`는 도구가 정한다. crate는 그 판정을 하지 않는다. 위치를 되살릴 수 없을 때 도구가 이 이유를 낸다. crate가 그 값을 만드는 함수를 준다.
- 변환은 `CliError`이다. 코드는 `CURSOR_INVALID`, `field`는 `--cursor`, `reason`은 위 셋 중 하나이다. 코드 행의 `reasons`에 쓸 상수 셋(`ReasonInfo`)도 crate가 낸다.
- 해독이 판을 안다. 1.1 cursor와 1.2 cursor는 서로 다른 묶기 값이므로 한 도구가 판을 올리면 이전 cursor는 `ARGUMENTS_CHANGED`나 `FOREIGN`이다.

### F4 다음 쪽 argv

- 입력은 호출이 명시한 인수와 방금 부호화한 cursor이다. 출력은 그 호출을 이어 가는 `argv`이다.
- 인수는 정규화해 argv 옵션으로 쓰고, 마지막에 `--cursor <값>`을 붙인다. 기본값과 환경 변수 값은 싣지 않는다. 앞에 오던 `--cursor`는 새 값으로 바뀐다.
- 인수가 `--input -`이나 파일로 왔으면 argv 옵션이나 인라인 `--input` 단어 하나로 다시 쓴다(spec 12.5). `--input -`는 싣지 않는다.
- 값이 `-`로 시작하면 `--name=value`로 쓴다(spec 9.12.1).

### F5 판 시험

- crate의 시험이 판마다 계약의 예를 확인한다. 같은 인수 목록에서 1.1은 `--all`을 묶고 1.2는 묶지 않는다. 1.2는 기본값을 명시하면 묶기 값이 바뀌고 1.1도 같다. 환경 변수는 1.2에서만 묶인다.

## Acceptance

- F1:
  - ==**같은 인수 목록이 순서와 철자와 정규화 전 표기가 달라도 같은 묶기 값을 낸다.**== `--round=1`과 `--round 1`, 정수 `1`과 `1.0`이 같다.
  - 1.2에서 `--all`, `--json`, `--limit`, `--fields`의 유무와 값이 묶기 값에 영향이 없다. 기본값을 명시한 인수와 환경 변수의 유무는 영향이 있다. `false` 스위치와 `0` 카운트는 영향이 없다.
  - 1.1에서 `--all`의 유무는 영향이 있다. 1.1에는 환경 변수 출처가 없다.
  - 실패 경로: 낯선 판은 거절된다. 인수 값이 JCS가 표현하지 못하는 수(안전한 정수 밖)면 오류를 낸다.
- F2:
  - 부호화한 cursor를 해독하면 같은 영역, 묶기 값, 위치가 나온다. 문자열이 ASCII이고 공백, 따옴표, 줄바꿈이 없다.
  - 실패 경로: 문자열을 한 글자 바꾸면 해독이 `FOREIGN`이다. 빈 문자열과 임의 문자열도 `FOREIGN`이다.
- F3:
  - 다른 영역의 cursor는 `FOREIGN`, 다른 인수의 cursor는 `ARGUMENTS_CHANGED`이다. 오류가 `CURSOR_INVALID`, `field` `--cursor`, 해당 `reason`을 가진다.
  - 실패 경로: 판을 올린 도구가 이전 판의 cursor를 받으면 거절된다. `EXPIRED`를 만드는 함수가 그 이유를 가진 오류를 낸다.
- F4:
  - 다음 쪽 argv에 기본값과 환경 변수 값이 없고 `--cursor <값>`이 있다. `--input -`로 온 인수가 argv 옵션이나 인라인 단어로 나오고 `--input -`는 없다. 같은 환경에서 그 argv를 도구에 주면 묶기 값이 같다.
  - 실패 경로: `-`로 시작하는 값은 `--name=value`로 쓰인다. 앞의 `--cursor`가 있던 입력에는 `--cursor`가 하나만 남는다.
- F5:
  - 계약 예시(`acme member list`)의 1.1과 1.2 호출에서 위의 판 차이가 시험으로 확인된다.
- `cargo test --workspace`, 생성 문서 drift 시험이 통과하고 canon과 `atip check`의 출력이 바뀌지 않는다.

## Questions and assumptions

- 가정: 묶기 값은 JCS와 SHA-256이다. 계약은 cursor의 형식을 정하지 않는다. approaches/typescript.md §3.8이 권하는 방식이다. 0.34까지 canon의 묶기 값은 8바이트 해시였고 이 값은 canon이 옮겨 올 때 바뀐다.
- 가정: 이 변경이 JCS 직렬화를 `atip::jcs` 모듈로 만든다. 새 의존성은 `sha2`이다. 계획 digest(`canon-0-35-crate-plan`)가 같은 모듈을 쓰므로 이 변경이 먼저 병합된다. RFC 8785의 부록 시험 벡터를 모듈의 시험으로 쓴다.
- 가정: 도구 자신의 flag-only 옵션(canon의 `repo`와 `as`)은 호출자가 표시해 넘긴다. crate는 도구의 옵션 이름을 모른다. 이 부분은 계약이 아닌 typescript.md의 조언이다.
- 가정: 위치 문자열은 도구의 것이다. 현재 canon처럼 키가 그대로 보이는 것은 이 변경의 몫이 아니다.
- 정책 질문은 없다.

## Out of scope

- canon의 `paging.rs`와 `json_out.rs`의 다음 쪽 항목 구성을 새 모듈로 옮기는 일(`canon-0-35-canon-cursor-1-2`).
- `meta.page` 구성, `--all`의 실행, `--limit`의 도메인.
- `jsonschema` 기능 분리(`canon-0-35-crate-validate`)와 계획 digest(`canon-0-35-crate-plan`).

Approved: Journey Hwang 2026-11-02T06:38:09Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:99bc160ef27a576ce001af99f33d4c24d31219b0ba99a17094d1f473258673b4)
