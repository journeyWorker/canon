---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "함수 이름과 쪽지 문자열은 읽는 쪽이 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "함수 이름과 쪽지 문자열은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "이름과 철자는 읽는 쪽이 쓰는 철자 자체이고 이 변경의 대상이다"
---

# crates/atip: split_argv가 quote가 쓴 문자열을 되읽는다

Source: https://github.com/journeyWorker/canon/issues/134 (첫 댓글 2번), spec 6.8, approaches/checker.md §7.1

## Intent

`atip::quote`는 작은따옴표가 든 낱말을 `'it'\''s'`로 쓴다. `atip::split_argv`는 따옴표 밖의 `\`를 읽지 못하고 그대로 글자로 넣는다. 그래서 `quote("it's")`를 다시 쪼개면 `it\s`가 나온다. spec 6.8은 `meta.next[].command`를 POSIX 셸처럼 쪼갠 결과가 `argv`와 같아야 한다고 정한다. 이 버그가 있으면 올바른 도구의 다음 단계를 검사기가 틀렸다고 보고한다. 0.33의 crate 추출 때도 이 비대칭이 기록되었다.

## Desired outcome

- 모든 낱말 목록 `words`에 대해 `split_argv(join(words.map(quote)))`가 `words`이다.
- ==**`quote`가 쓰는 문자열은 한 글자도 바뀌지 않는다.**== canon이 이미 낸 `command` 문자열과 같다.
- `split_argv`는 POSIX 셸의 따옴표 밖 `\` 규칙을 읽는다. 검사기와 canon이 같은 함수를 쓴다.

## User flow

1. F1 도구는 `quote`로 `command` 문자열을 만든다 → 작은따옴표나 `\`가 든 낱말도 들어 있다 → 어떤 낱말도 깨지지 않는다.
2. F2 호출자는 `split_argv`로 `command`를 쪼갠다 → `argv`와 같은 낱말 목록을 얻는다 → ==**같은 호출을 다시 만든다.**==
3. F3 검사기는 `meta.next`의 `command`와 `argv`를 비교한다 → 올바른 도구의 작은따옴표 낱말에서 거짓 실패가 없다 → 도구를 잘못 보고하지 않는다.

## Step detail

### F1 quote

- ==**`quote`의 출력은 그대로이다.**== 안전한 글자만 있는 낱말은 그대로, 나머지는 작은따옴표로 묶고 안의 `'`를 `'\''`로 쓴다. 빈 낱말은 `''`이다.
- 이 함수의 동작이 canon의 `command` 문자열과 보고서 출력에 이미 쓰이므로 바꾸지 않는다.

### F2 split_argv

- 따옴표 밖에서 `\`는 바로 다음 글자를 글자 그대로 넣는다. `\`와 줄바꿈이 이어지면 둘 다 버려 줄을 잇는다. 입력 끝의 외로운 `\`는 글자 `\` 하나로 남긴다.
- 작은따옴표 안은 모두 글자 그대로이다. 큰따옴표 안에서는 `\"`와 `\\`만 한 글자로 읽고, `\`와 그 밖의 글자는 둘 다 남긴다. 지금과 같다.
- 공백은 낱말을 나누고, 따옴표로 묶은 빈 문자열은 빈 낱말이다. 닫히지 않은 따옴표는 끝까지 읽는다.
- 확장(`$`, 백틱, `*`)은 하지 않는다. 셸 연산자가 든 문자열은 이 함수의 대상이 아니다.

### F3 호출 지점

- 호출자는 `canon-cli`의 다음 단계와 `safety`, `migrate_names`, 그리고 `atip-check`의 세 곳이다. 이 변경은 호출 지점을 바꾸지 않는다. 함수의 의미만 바뀐다.

## Acceptance

- F1:
  - ==**`quote("it's")`는 `'it'\''s'`이고 `split_argv`로 되돌리면 `["it's"]`이다.**==
  - `quote`의 기존 시험(안전한 낱말은 그대로, 나머지는 따옴표)이 그대로 통과한다.
- F2:
  - 글자 `a`, `'`, `"`, `\`, 공백, `$`, 백틱, 줄바꿈, 탭으로 길이 0에서 4까지 모든 낱말 목록을 만든다.
  - 그 목록을 `quote`로 이은 문자열을 되쪼개면 원래 목록과 같다. 시험은 새 의존성을 쓰지 않는다.
  - 따옴표 밖의 `a\ b`는 낱말 하나 `a b`, `a\\b`는 `a\b`, 줄 끝의 `\`와 줄바꿈은 줄 이음, 입력 끝의 `\`는 `\`이다.
  - 실패 경로: 큰따옴표 안의 `\x`는 `\x` 둘 다 남고, 작은따옴표 안의 `\`는 글자 그대로이다. 닫히지 않은 따옴표가 있어도 패닉하지 않는다.
- F3:
  - 기존 호출 지점의 시험이 모두 통과하고 canon의 보고서 출력, `command` 문자열, 조건 이름이 바뀌지 않는다.
  - 실패 경로: 작은따옴표가 든 값을 `meta.next`에 싣는 가짜 도구에서 검사기의 `L0.next`가 실패하지 않는다.

## Questions and assumptions

- 가정: 이 일은 `atip` crate의 오류 수정이고 spec의 변경이 아니다. 저자가 이 일을 crate 항목으로 보고했다. 0.35에서 crate와 검사기의 동작이 바뀌는 점은 CHANGELOG에 싣는다.
- 가정: 줄 잇기와 외로운 `\`는 계약의 검사기 문서 §7.1(`a trailing \ joins the next line`)에 맞춘다. 문서가 외로운 `\`를 말하지 않아 POSIX의 보통 동작을 따른다.
- 정책 질문은 없다.

## Out of scope

- `quote`의 출력 형식 바꾸기. 큰따옴표 인용으로 바꾸지 않는다.
- 셸 확장, 파이프, 연산자 해석. 계약 14.5가 그 줄을 대상 밖으로 둔다.
- 다음 단계의 `command` 문자열 형식이나 `argv` 구성(canon-0-35-crate-cursor, canon-0-35-crate-plan).

Approved: Journey Hwang 2026-11-02T06:38:09Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:10e798d50c5344d106218f10f2fa3ccc17a340553681bb29e8a053bb87d87968)
