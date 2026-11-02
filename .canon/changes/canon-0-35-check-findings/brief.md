---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, 보고 멤버, 조건 이름은 래퍼가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "검사 id와 보고 멤버 이름은 결정 대상의 철자 자체다"
---

# atip check --json이 검사마다 모든 발견을 호출과 종류로 싣는다

Source: https://github.com/journeyWorker/canon/issues/134 (저자 두 번째 댓글 1번과 첫 댓글 1번), approaches/checker.md §2와 §6

## Intent

`atip check --json`의 검사 한 개는 실패 이유를 `reason` 문자열 하나로만 싣는다. 처음 세 항목만 남기고 각 항목을 240자에서 자르고 "and N more"를 붙인다. 그래서 `L0.contract-version`이 세 항목만 보이고 나머지는 잘린다. 이 검사기를 감싸는 스크립트는 어느 호출이 어느 조건에서 틀렸는지 걸러낼 수 없고, 세 번째 구현은 래퍼를 쓰지 못했다.

## Desired outcome

- `fail`이거나 `unprovided`인 검사의 보고에 `findings[]`가 있다. 항목은 조건의 종류(`kind`), 사람이 읽는 `message`, 그 발견을 낳은 호출의 `argv`를 가진다.
- ==**모든 발견이 잘리지 않고 실린다.**==
- `reason`은 지금처럼 사람이 읽는 요약이고 바뀌지 않는다. 텍스트 보고의 모양은 같다.
- 검사 결과, 레벨 판정, 종료 코드는 바뀌지 않는다.

## User flow

1. F1 래퍼 작성자는 `--json` 보고에서 실패한 검사를 읽는다 → 검사의 `findings[]`에 발견마다 `kind`와 `argv`가 있다 → 호출과 종류로 걸러서 자기 결과로 옮긴다.
2. F2 래퍼 작성자는 발견이 많은 검사를 읽는다 → 모든 발견이 있고 "and N more"로 잘리지 않는다 → 목록을 직접 센다.
3. F3 도구 작성자는 텍스트 보고를 읽는다 → 검사마다 요약 한 줄과 처음 몇 개의 이유가 지금과 같다 → 읽는 방식이 바뀌지 않는다.
4. F4 검사기 개발자는 시험을 돌린다 → 심은 위반마다 그 검사의 `findings[]`에 위반의 `kind`가 있다 → ==**발견이 판정과 같은 원천에서 만들어진다는 것을 안다.**==

## Step detail

### F1 발견 한 개

- `findings[]` 항목의 멤버는 다음과 같다.
  - `kind`는 검사가 그 판정에 쓰는 조건의 이름이다. 예를 들어 `paged-next-cursor`와 `returned-at-most-limit`가 있다. 지금 소스에 86개가 있고 모두 소문자와 하이픈이다.
  - `message`는 지금 `reason`을 만드는 문장 그대로다. 자르지 않는다.
  - `argv`는 발견이 한 호출에서 나왔을 때 그 호출의 argv 배열이다. `observed`가 싣는 argv와 같다. 문서 전체를 보고 한 호출로 좁힐 수 없는 발견에는 없다.
  - `code`는 선택이다. 발견이 도구가 낸 등록된 오류 코드를 이유로 삼을 때만 있다. `canon-0-35-check-fixture`가 이 멤버를 쓴다.
- `unprovided`인 검사의 `findings[]`는 fixture에 없는 항목을 `kind`로 가리킨다. 값은 `missing-fixture-entry`이고 `message`가 항목 이름이다.
- `pass`, `unsupported`, `entailed`인 검사에는 `findings`가 없다. SHOULD 검사의 `fail`에는 `findings`가 있고, 권고만 하는 stderr 출력의 `advisories`는 그대로 따로 둔다.
- ==**`kind`는 보고의 공개 철자이다.**== 조건 이름을 바꾸는 변경은 CHANGELOG에 옛 이름과 새 이름을 싣는다.

### F2 잘림 없음

- `reason`의 세 항목과 240자 제한은 그대로 두되 `findings[]`는 어디에서도 자르지 않는다.
- `reason`은 `findings[]`의 처음 세 `message`를 이은 것이다. 그 뒤에 `and N more`가 오는 경우 N은 `findings[]`의 나머지 개수와 같다.
- 한 검사의 발견이 몇 개든 보고가 한 번에 출력되고 상한은 두지 않는다.

### F3 텍스트

- 텍스트 보고는 검사 줄, `reason`, `notes`, `unlisted`를 지금처럼 낸다. `findings`는 텍스트에 싣지 않는다.
- `reason`이 잘렸을 때 줄 끝의 `and N more` 뒤에 `(all in --json findings)`를 붙인다.

### F4 시험

- 위반 설정을 가진 모든 시험이 소유 검사의 `findings[]`에서 그 위반의 `kind`를 찾는다.
- 보고 JSON이 계약 봉투 스키마를 통과하고 `data.checks[].findings[]`의 멤버가 위 넷뿐이다.

## Acceptance

- F1:
  - `L0.contract-version`을 어기는 가짜 도구에 `--json`을 돌리면 그 검사의 `findings[]`에 발견마다 `kind`, `message`, `argv`가 있다. `argv`는 `observed`의 호출과 일치한다.
  - 호출 하나로 좁혀지지 않는 발견은 `argv` 없이 나온다. `code`는 `canon-0-35-check-fixture`가 정한 경우 외에는 없다.
  - 실패 경로: fixture에 필요한 항목이 없어 `unprovided`인 검사는 `kind`가 `missing-fixture-entry`이고 `message`가 그 항목 이름이다. `pass`인 검사에는 `findings`가 없다.
- F2:
  - 발견이 다섯 개 이상이 되도록 심은 도구에서 `findings[]`의 개수가 발견 수와 같다. `reason`의 `and N more`의 N이 `findings[]`의 나머지 개수와 같다.
  - 실패 경로: 발견 한 개의 `message`가 240자를 넘어도 `findings[]`에는 자르지 않은 전체가 있고 `reason`에서만 잘린다.
- F3:
  - 텍스트 보고에 `findings`가 없고, 잘린 `reason`의 줄 끝에 `(all in --json findings)`가 있다. 잘리지 않은 경우에는 붙지 않는다.
- F4:
  - 위반 시험이 모두 소유 검사의 `findings[]`에서 해당 `kind`를 찾는다.
  - 실패 경로: 조건 이름을 바꾸고 시험의 기대는 그대로 두면 그 시험이 실패한다.
  - ==**검사 결과, 레벨 판정, 종료 코드는 이 변경 전후의 시험에서 같다.**==

## Questions and assumptions

- 가정: `kind`는 소스의 조건 이름을 그대로 쓴다. 저자가 `findings[]` 모양(argv와 종류)만 요구했고 이름 규칙은 정하지 않았다. 다른 철자를 원하면 알려 달라.
- 가정: `message`는 자르지 않고 `argv`는 호출을 모두 싣는다. 호출 하나의 argv가 길어도 줄이지 않는다.
- 가정: 보고 데이터에 새 멤버가 늘 뿐이라 검사기 자신의 `schemaVersion`은 올리지 않는다. 검사기의 보고는 `atip check` 전용 출력이고 `schema`가 선언하지 않는다.
- 정책 질문은 없다.

## Out of scope

- 검사 결과, 판정 규칙, 종료 코드의 변경. `canon-0-35-check-fixture`가 거절된 fixture 호출과 도움말 줄 읽기를 바꾼다.
- 텍스트 보고에 발견 전체를 싣는 일.
- 1.2 주장과 새 검사 행(`canon-0-35-check-1-2`), L2 검사(`canon-0-35-check-l2-*`).
