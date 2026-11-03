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
---

# canon의 cursor가 atip::cursor를 쓰고 1.2 묶기 규칙과 flag-only 쪽 나누기 옵션을 따른다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 6.9, 6.10, 9.8, 11.6, 15.10, issue 134

## Intent

canon의 cursor는 0.34에서 이미 명시한 인수에 묶이고 `--limit`, `--all`, `--cursor`, `--fields`, `--json`을 뺀다. 그러나 묶는 값은 인수 원문 글자의 8바이트 해시다. 환경 변수 값은 묶이지 않아 `CANON_ROLE`을 바꿔도 이어진다. 다음 쪽 항목은 호출의 argv에서 만들어 원문을 그대로 쓴다. `limit`, `cursor`, `all`이 flag-only로 표시되지 않아 `export --input '{"limit":5}'`가 받아들여진다. 계약 1.2는 이 모두를 규칙으로 만들고 canon의 현재 동작을 상당 부분 그대로 규칙으로 삼는다.

## Desired outcome

- canon의 모든 쪽 나누기 명령(`query`, `finding list`, `divergence list`, `export`)이 `atip::cursor`의 1.2 묶기를 쓴다.
- 환경 변수 값, 기본값을 명시한 인수, 정규화한 값이 계약대로 묶이거나 빠진다.
- 다음 쪽 `next` 항목이 호출이 명시한 인수만 정규화해 싣는다.
- ==**`limit`, `cursor`, `all`, `fields`가 모든 명령에서 flag-only이고 문서 키로 거절된다.**==
- 0.34가 낸 cursor는 이 변경 뒤에 거절된다. 이는 업그레이드 노트에 싣는다.

## User flow

1. F1 에이전트는 쪽을 나누는 명령을 `--limit`으로 부른다 → `meta.next`의 첫 항목이 명시한 인수와 `--cursor`만 싣는다 → 그대로 실행해 다음 쪽을 읽는다.
2. F2 에이전트는 다음 쪽을 `--limit` 대신 `--all`로 부른다 → 이어서 남은 모두를 읽는다 → 한 번에 끝까지 읽는다.
3. F3 에이전트는 환경이나 인수를 바꿔 같은 cursor를 쓴다 → `CURSOR_INVALID`와 이유가 온다 → ==**처음부터 다시 부른다.**==
4. F4 에이전트는 `--fields`를 `--json` 없이 쓴다 → 같은 검증을 받는다 → 텍스트와 JSON에서 같은 규칙을 안다.
5. F5 에이전트는 `--input` 문서에 `limit`을 쓴다 → `INPUT_INVALID`와 `FLAG_ONLY_KEY`로 거절된다 → argv에 쓴다.
6. F6 사람은 0.34가 낸 cursor를 쓴다 → `CURSOR_INVALID`와 `FOREIGN`이 온다 → 처음부터 다시 부른다.

## Step detail

### F1 묶기와 다음 쪽

- ==**묶기는 `atip::cursor`가 한다.**== canon은 호출이 준 인수를 입력으로 넘긴다. 명시 인수, 환경 변수 출처, canon의 flag-only 옵션(`repo`, `as`)이 해당한다.
- `--limit`, `--all`, `--cursor`, `--fields`, `--json`은 묶이지 않는다. 다른 모든 명시 인수는 묶인다. 기본값을 명시한 인수는 준 것이다.
- 환경 변수 값은 묶인다. canon에서 `env`를 가진 옵션은 `--as`의 `CANON_ROLE`이다. 그 값을 바꾸거나 지우거나 옵션으로 명시하면 cursor가 거절된다. 다른 환경 변수가 옵션을 채우는 쪽 나누기 명령이 있으면 같다.
- 다음 쪽 항목은 호출이 명시한 인수를 정규화해 argv 옵션으로 쓰고 `--cursor <값>`을 붙인다. 기본값과 환경 변수 값을 싣지 않는다. `--input -`나 파일로 온 인수는 argv 옵션이나 인라인 `--input` 단어 하나로 쓰고 `--input -`는 싣지 않는다.
- cursor는 호출에 묶인 ASCII 낱말 하나이다. 암호화하지 않으므로 위치가 읽힐 수 있다. 호출자는 그것을 바꾸지 않고 그대로 돌려주고 해석하지 않는다. 한 글자라도 바뀌면 `FOREIGN`이다.

### F2 --all

- `meta.next`의 항목에서 `--limit`을 `--all`로 바꿔 이어 가면 거절 없이 남은 모두를 읽는다. `--limit`과 `--all`은 함께 줄 수 없다(`atMostOne`).
- `--all`을 가진 명령은 `finding list`, `divergence list`, `query`, `export`뿐이다. 그 밖의 쪽 나누기에는 영향이 없다.

### F3 거절

- 이유는 `FOREIGN`(이 명령이 낸 것이 아니거나 해독할 수 없다), `ARGUMENTS_CHANGED`(다른 인수나 환경으로 낸 것), `EXPIRED`(위치의 기록이 없어졌다)이다. 코드 행의 `reasons`는 crate가 내는 상수이다.
- 거절은 exit 2, `field` `--cursor`이고 아무것도 바꾸지 않는다. 거절은 `export`의 감사 줄도 쓰지 않는다.
- `--limit`과 `--fields`만 바꾼 호출은 거절되지 않고 같은 위치에서 이어진다.

### F4 --fields와 --json

- `--fields`는 `--json`과 상관없이 같은 검증을 한다. 모르는 경로는 두 모드 모두 `USAGE`이고 `field` `--fields`이다. 텍스트 모드의 출력은 canon이 정한다. 0.34가 이미 이렇게 동작하므로 이 부분은 시험으로 고정하는 일이다.

### F5 flag-only

- `limit`, `cursor`, `all`을 가진 명령의 `schema` 문서에서 세 옵션이 `flagOnly: true`이다. `fields`는 이미 그렇다. 조각 스키마에 이 키들이 없다.
- 입력 문서에 이 키를 주면 `INPUT_INVALID`, 이유 `FLAG_ONLY_KEY`, `field`가 키의 포인터이다. 지금은 `export`가 `limit`을 받아들인다.
- canon의 flag-only 집합은 `atip::flag_only_options`의 1.2 집합에 `repo`와 `as`를 더한 것이다. 소스에 따로 적은 목록이 남지 않는다.

### F6 업그레이드

- 0.34가 낸 cursor는 이 변경 뒤에 `FOREIGN`이다. 쪽을 나눠 읽는 도중에 canon을 올린 호출자는 처음부터 다시 읽는다. cursor는 오래 두고 쓰는 값이 아니다.
- CHANGELOG의 이 버전 항목에 같은 사실과 `FLAG_ONLY_KEY`로 바뀐 거절을 싣는다.

## Acceptance

- F1:
  - `canon query --kind subject --limit 1 --json`의 `meta.next[0].argv`가 명시한 인수와 `--cursor <값>`만 가진다. 기본값을 명시하지 않은 호출에는 기본값이 없다.
  - ==**`--as reviewer`를 명시하지 않은 호출에서 `CANON_ROLE`을 바꾸면 그 cursor는 `ARGUMENTS_CHANGED`로 거절된다.**==
  - 기본값을 명시한 인수를 더하거나 빼면 `ARGUMENTS_CHANGED`이다. `--limit`, `--fields`, `--json`만 바꾸면 이어진다.
  - 실패 경로: cursor는 공백과 따옴표가 없는 ASCII 낱말 하나이다. 문자열 한 글자를 바꾸면 `FOREIGN`이다. 암호화나 가림은 요구하지 않는다.
- F2:
  - `--limit`을 `--all`로 바꿔 이어 가면 남은 모두를 읽고 거절되지 않는다. 네 명령에서 시험한다.
  - 실패 경로: `--limit`과 `--all`을 함께 주면 exit 2, `USAGE`, `field`가 앞선 이름이다.
- F3:
  - 다른 필터, 다른 `--kind`, 다른 `--repo`로 쓴 cursor는 `ARGUMENTS_CHANGED`이다. 다른 명령의 cursor는 `FOREIGN`이다. 지운 기록을 가리키는 cursor는 `EXPIRED`이다.
  - 실패 경로: 거절이 어떤 기록도 쓰지 않는다. `export`의 거절은 감사 줄을 쓰지 않는다.
- F4:
  - `canon query --kind subject --fields nosuch`가 `--json` 없이도 exit 2, `USAGE`, `field` `--fields`이다. JSON 모드와 같은 코드와 `expected`이다.
- F5:
  - 네 명령(`query`, `finding list`, `divergence list`, `export`)의 `schema` 문서에서 `limit`, `cursor`, `all`, `fields`가 `flagOnly: true`이고 조각 스키마에 없다.
  - `canon export --input '{"limit":5}'`가 exit 2, `INPUT_INVALID`, 이유 `FLAG_ONLY_KEY`, `field` `/limit`이다. 아무것도 쓰이지 않는다.
  - 실패 경로: 소스에 `repo`와 `as`를 직접 적은 flag-only 목록이 없고 crate의 집합을 쓴다.
- F6:
  - 0.34 바이너리로 만든 cursor 문자열은 `FOREIGN`이다. CHANGELOG에 업그레이드 노트가 있다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다. 쪽 나누기와 `--fields`를 쓰던 소비자(스킬, 웹사이트, 대시보드, 평가 스크립트)가 갱신된다.

## Questions and assumptions

- 가정: canon이 1.2를 선언하는 일(`meta.contractVersion`)은 `canon-0-35-claim-l2`가 한다. 이 변경은 1.2의 cursor 규칙과 flag-only를 구현하고, 선언은 마지막에 켠다. 선언 전에도 규칙은 지킨다.
- 가정: 환경 변수 `CANON_ROLE`이 canon의 쪽 나누기 명령 모두에서 `--as`를 채우므로 바뀌면 cursor가 거절된다. 이는 계약 6.9(`env` 값은 묶인다)대로이고 0.34보다 엄격해진다.
- 가정: 0.34 cursor가 거절되는 것은 형식이 바뀌는 데서 오는 결과이다. 호환 해독기를 두지 않는다. 0.33 이후의 깨지는 릴리스 방식과 같다.
- 정책 질문은 없다.

## Out of scope

- crate의 cursor 모듈(`canon-0-35-crate-cursor`).
- `--input`의 처리 순서와 배치(`canon-0-35-input-order`, `canon-0-35-batch-pointers`). 이 변경은 flag-only 키의 거절 이유만 다룬다.
- 1.2 선언과 CI(`canon-0-35-claim-l2`).
- 새 쪽 나누기 명령을 더하는 일과 `retrieve`나 `search`의 `--limit` 의미를 바꾸는 일.

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:9d1ff29741e921b73d5627a75fed7d37ba4ee07fd4b70d6fe99c5dbd62f8f936)
