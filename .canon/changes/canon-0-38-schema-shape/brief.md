---
waive:
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "멤버 이름과 값 이름은 이 변경이 다루는 스키마 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "멤버 이름과 값 이름은 스키마 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "멤버 이름과 값 이름은 스키마 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "멤버 이름과 값 이름은 검증 대상의 철자 자체다"
---

# 아이디 인자는 자기 이름을 갖고, 스키마는 모든 역할이 쓰는 명령의 역할 목록을 뺀다

Source: canon 이슈 #141의 둘째 항목, ATIP 태그 v1.5.0 annex-schema S1 3과 4, spec 9.5와 9.10.2, `canon schema --json`을 0.37.0 바이너리로 잰 값

## Intent

`canon schema`가 말하는 인자 이름 중 `id`가 여섯 명령(`context-pack show`와 `verify`, `subject move`, `new`, `show`, `trace`)의 인자 이름이다. 서로 다른 뜻이다. `trace`의 인자는 값 이름이 없어 도움말에 `<ID>`로 나온다. 값 이름이 그냥 `ID`인 옵션도 셋이다(`--skill`, `--evaluator`, `--contradicting-trajectory`). 독자는 이름만 보고 어떤 아이디인지 모른다.

스키마 트리는 명령 78개마다 `x-canon`에 역할 목록을 단다. 네 역할이 모두 쓰는 명령 13개도 같은 네 역할을 적고, 이 항목이 트리 25,709바이트 가운데 936바이트다.

## Desired outcome

- 아이디를 받는 인자와 옵션 값은 무엇의 아이디인지 말하는 값 이름을 갖는다. `PACK_ID`, `SUBJECT_ID`, `TRACE_ID`, `SKILL_ID`, `EVALUATOR_ID`, `TRAJECTORY_ID`이다.
- 인자 이름 `id`는 스키마에 남지 않는다. 인자는 아이디가 가리키는 대상의 이름(`pack`, `subject`, `trace`)을 갖는다.
- ==**네 역할이 모두 쓰는 명령은 `x-canon`의 역할 목록을 싣지 않고, 목록이 없으면 네 역할 모두라는 뜻이다.**==
- 그 뜻은 트리가 한 번 말한다.
- 입력 문서와 명령 줄의 동작은 바뀌지 않는다.

## User flow

1. F1 에이전트는 `canon trace --help`나 `canon schema trace`를 읽는다 → 인자의 값 이름이 `TRACE_ID`다 → 무엇을 넣을지 안다.
2. F2 에이전트는 `canon schema`로 `subject`와 `context-pack`의 인자를 읽는다 → 인자 이름이 대상의 이름이다 → 이름과 값 이름이 어긋나지 않는다.
3. F3 에이전트는 `--skill`, `--evaluator`, `--contradicting-trajectory`의 값 이름을 읽는다 → 각자 자기 이름이다 → 값을 알맞게 고른다.
4. F4 에이전트는 `canon schema`로 역할별 명령을 고른다 → 역할 목록이 없는 명령은 네 역할 모두의 것이다 → ==**이전과 같은 명령 목록을 얻는다.**==
5. F5 사람은 CHANGELOG를 읽는다 → 이름이 바뀐 곳과 사라진 멤버가 있다 → 스키마를 읽는 도구를 고친다.

## Step detail

### F1 trace 인자

- 인자는 리뷰 아이디, 커밋 해시, 발견 키 셋 가운데 하나를 받는다. ==**값 이름 `TRACE_ID`는 이 셋을 모두 뜻한다.**==
- 도움말의 `<ID>`는 `<TRACE_ID>`가 된다. 인자가 받는 값과 그 거절은 그대로다.

### F2 인자 이름

- `subject move`, `new`, `show`의 인자 이름은 `subject`, `context-pack show`와 `verify`는 `pack`, `trace`는 `trace`이다. 값 이름 `SUBJECT_ID`와 `PACK_ID`는 그대로다.
- 인자 이름은 입력 문서에 나타나지 않는다. 문서는 `operands` 배열을 쓰고 이 규칙은 그대로다.
- 인자 오류의 `field`는 값 이름이다. `trace`의 값 이름이 바뀌므로 그 오류의 `field`가 `TRACE_ID`로 바뀐다.

### F3 옵션 값 이름

- `--skill`은 `SKILL_ID`, `--evaluator`는 `EVALUATOR_ID`, `--contradicting-trajectory`는 `TRAJECTORY_ID`이다. 옵션 이름, 값 형태, 도메인은 그대로다.

### F4 역할 목록

- 역할 목록을 싣지 않는 명령은 지금 네 역할을 모두 적던 13개이고, 트리와 명령 하나의 문서 모두 같다. 역할이 하나라도 빠진 명령과 사람 전용 명령은 목록을 싣는다. 사람 전용의 목록은 빈 배열 그대로다.
- `x-canon`에 역할 목록 말고 다른 멤버가 없으면 `x-canon` 전체를 싣지 않는다.
- 역할을 고른 `canon schema`는 목록 없는 명령을 모든 역할의 목록에 넣는다. `canon context`, `canon status`, 역할 안내문의 명령 블록은 이전과 같은 명령을 낸다.
- 트리 최상위의 `x-canon`이 한 번 `rolesDefault`로 말한다. 값은 네 역할 이름이다. 이 멤버가 없는 트리는 지금까지처럼 모든 명령이 목록을 싣는다.

### F5 변경 기록

- CHANGELOG의 Breaking 항목에 바뀐 이름을 표로 싣는다. 인자 이름이 바뀐 명령 여섯, 값 이름이 바뀐 옵션 셋, 사라진 `x-canon` 멤버가 행이 된다.
- 도구가 읽는 방법은 "멤버 없음은 네 역할"이다.

## Acceptance

- F1:
  - `canon schema trace --json`의 인자 값 이름이 `TRACE_ID`이고 도움말도 같다.
  - 실패 경로: 틀린 값을 주면 오류의 `field`가 `TRACE_ID`이고 받는 값은 그대로다.
- F2:
  - 여섯 명령의 인자 이름이 `subject`, `pack`, `trace`이고 `id`는 트리 어디에도 없다.
  - 실패 경로: `--input`에 `operands` 배열로 준 호출은 이전과 같이 통한다.
- F3:
  - 세 옵션의 값 이름이 새 이름이고 값 이름 `ID`는 트리에 남지 않는다.
  - 실패 경로: 틀린 값의 거절 코드와 도메인은 0.37과 같다.
- F4:
  - 13개 명령에 `x-canon`이 없고 트리가 900바이트 넘게 작다. ==**역할을 고른 명령 목록과 `canon context`의 목록은 0.37과 같다.**==
  - 실패 경로: 역할이 셋뿐인 명령이나 사람 전용 명령이 목록을 잃으면 시험이 실패한다.
- F5:
  - CHANGELOG 표가 바뀐 이름마다 한 행을 갖는다.
  - 실패 경로: 행이 빠지면 릴리스 확인 시험이 실패한다.
- 생성 문서 drift 시험, `just atip-deep`, 적합성 시험이 통과한다.

## Questions and assumptions

- 결정: 역할 목록은 ATIP가 정한 멤버가 아니다. `x-` 멤버의 뜻은 도구가 정한다(spec 9.10.2). 그래서 없음이 네 역할이라는 규칙은 canon이 정하고 트리가 한 번 말한다. 호출자는 모르는 멤버를 무시하므로 계약에는 어긋나지 않는다.
- 결정: 절감은 936바이트이고 트리 크기의 3.6%이다. 트리 최상위 멤버가 되돌리는 몫을 빼면 900바이트가 약간 넘는다. 이 크기가 소비 도구의 고침 비용에 값하지 않는다고 사람이 보면 F4를 뺀다.
- 결정: 인자 이름 `id`를 걷는 범위는 아이디 인자다. `NAME`, `TEXT` 같은 일반 값 이름은 이 변경 밖이다.
- 정책 질문: 스키마를 읽는 도구가 인자 이름과 모든 명령의 역할 목록에 기대고 있으면 깨진다. 사람이 이 변경을 승인한다.

## Out of scope

- 옵션 이름의 규칙(annex-schema S1 4는 옵션 이름만 묶는다)과 옵션 이름의 변경.
- 일반 값 이름(`NAME`, `TEXT`, `FILE`).
- 스키마의 다른 크기 절감.
- 역할을 강제하는 일.
