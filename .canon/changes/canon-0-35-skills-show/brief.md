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
  - rule: "prose.length"
    section: "intent"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
---

# canon skills show가 선택 NAME과 안내 목록을 내고 도움말이 이를 가리킨다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 4.8, 14, schema/guide.schema.json, approaches/migration.md §3.1 6번

## Intent

에이전트가 canon을 처음 쓰는 입구는 `skills show`이다. 계약 14는 `skills show [NAME] [--json]`을 정하고 `NAME`을 선택으로 둔다. 이름이 없으면 기본 안내가 모든 안내의 이름을 싣고, 모르는 이름은 `expected`에 안내 이름들을 싣고 거절한다. `--json`의 `data`는 `{name, text, guides, full?}`이다. canon은 `NAME`이 필수라 이름 없이 부르면 `USAGE`이고, `--json`의 `data`가 `{name, version, markdown}`이라 계약 스키마와 다르고, `canon --help`의 첫 줄이 `schema`만 가리킨다. 그래서 L2의 `L2.skills`와 `L2.help-guide`가 canon에서 실패한다.

## Desired outcome

- `canon skills show`가 이름 없이도 기본 안내를 낸다. 기본 안내가 모든 안내의 이름을 싣는다.
- `--json`의 `data`가 계약의 안내 스키마를 통과한다.
- 모르는 이름은 등록된 코드로 거절하고 `expected`가 안내 이름들이다.
- ==**루트 `--help`의 첫 줄이 `canon skills show`도 가리킨다.**==
- 모든 안내의 `canon `으로 시작하는 명령 줄이 실제 파서로 유효하다.

## User flow

1. F1 에이전트는 `canon skills show`를 이름 없이 부른다 → 기본 안내와 모든 안내의 이름이 온다 → 읽을 안내를 고른다.
2. F2 에이전트는 `canon skills show <이름> --json`을 부른다 → `{name, text, guides, full?}`가 온다 → 안내를 Markdown으로 읽는다.
3. F3 에이전트는 모르는 이름을 준다 → exit 2와 `expected`로 안내 이름들이 온다 → 이름을 고른다.
4. F4 에이전트는 `canon --help`의 첫 줄을 읽는다 → `schema`와 `skills show`가 모두 있다 → 두 입구를 안다.
5. F5 에이전트는 안내의 명령을 그대로 실행한다 → 파싱 오류가 나지 않는다 → ==**안내를 믿고 쓴다.**==
6. F6 사람은 설치한 스킬과 웹사이트를 본다 → 새 이름과 모양이다 → 옛 이름이 남은 곳이 없다.

## Step detail

### F1 기본 안내

- ==**이름이 없으면 기본 안내를 낸다.**== 기본 안내는 canon의 시작 안내이고 `guides`가 모든 안내의 이름을 싣는다. 이름은 소문자와 하이픈이다.
- 텍스트 모드 출력은 안내의 Markdown이다. 이름 없는 텍스트 출력은 기본 안내이다.
- 상태 없이 답하고(빈 디렉터리에서도) 아무것도 쓰지 않는다. 지금의 `--full` 옵션은 그대로이고 `full` 멤버가 그 여부를 싣는다.

### F2 JSON 모양

- `data`는 `name`(안내 이름), `text`(Markdown), `guides`(모든 안내 이름, 이 안내 포함), 선택적 `full`이다. 지금의 `markdown`은 `text`가 되고 `version`은 `x-canon.version`으로 옮긴다. 안내 스키마가 `x-` 멤버를 허용한다.
- 출력은 `artifact` 종류이고 `meta.output`이 선언한 보고서 이름이다. 보고서 스키마가 crate의 안내 스키마를 참조한다.
- 같은 안내는 같은 바이트이다. 다음에 나오는 안내 목록의 순서는 정해진 순서이다.

### F3 모르는 이름

- `skills show nosuch-xyz`는 exit 2이다. 코드는 지금 쓰는 `USAGE`나 canon이 등록한 더 세분된 코드이다. `field`가 `NAME`이고 `expected`가 안내 이름들이다.
- 이름 문법에 맞지 않는 값(대문자, 공백)도 같은 거절이다.

### F4 도움말 첫 줄

- 루트 `canon --help`의 첫 줄은 `schema`와 `skills show`를 모두 가리킨다. 한 줄에 둘 다 있다. 문구는 구현자가 정한다. 예를 들어 `Agents: "canon schema --json" describes every command; "canon skills show" prints the guide.`
- 그룹과 명령의 `--help` 첫 줄도 같은 줄이다. 소개 문장은 그 줄 아래로 간다. 도움말 텍스트에서 검사 대상은 첫 줄뿐이다(L1.help-line, L2.help-guide).

### F5 안내 속 명령 줄

- 모든 안내의 코드 블록 중 `canon `으로 시작하는 줄이 spec 9.12 문법으로 유효하다. 안내에서 `<UPPER_CASE>` 자리표시자는 허용된다. 줄은 실제 파서와 `schema`의 명령 문서로 모두 파싱된다.
- 파이프나 리다이렉션이 든 줄은 대상 밖이다. 그런 줄을 쓰는 안내는 그대로 둔다.
- 유효하지 않은 줄이 있으면 안내를 고치거나 파서를 고친다. 구현자가 고친 줄의 목록을 변경 기록에 싣는다.
- 이 규칙을 지키는 시험(`guide_commands_parse`)이 모든 안내에서 돈다. 새 안내와 새 명령에도 적용된다.

### F6 소비자

- `skills show`의 출력을 읽는 소비자(설치한 스킬 파일, 훅, 웹사이트의 설명, 평가 스크립트)가 새 이름과 모양으로 옮겨 간다.
- 설치한 스킬 파일은 `skills show`를 가리키는 뼈대이다(spec 14.6). 이 변경은 설치 형식을 바꾸지 않는다.

## Acceptance

- F1:
  - `canon skills show --json`이 이름 없이 exit 0이고 `data.guides`가 모든 안내 이름을 싣는다. 빈 디렉터리에서도 같다.
  - 실패 경로: 아무것도 쓰이지 않는다. 스냅샷이 같다.
- F2:
  - 각 안내의 `--json`이 `{name, text, guides, full?}`이고 계약의 안내 스키마를 통과한다. `name`이 요청한 이름이다. `x-canon.version`이 있다.
  - ==**기본 안내와 모든 이름의 안내가 안내 스키마를 통과한다.**==
  - 실패 경로: `markdown`과 최상위 `version` 멤버가 없다. 스키마 사본에 한 글자를 바꿔도 시험이 실패한다.
- F3:
  - `canon skills show nosuch-xyz --json`이 exit 2이고 `field`가 `NAME`, `expected`가 안내 이름들이다. 대문자와 공백이 든 값도 같다.
- F4:
  - `canon --help`의 첫 줄에 `schema`와 `skills show`가 모두 있다. 한 그룹의 `--help`도 같다.
  - 실패 경로: 첫 줄에 `skills show`가 없으면 시험이 실패한다.
- F5:
  - 모든 안내의 모든 `canon ` 명령 줄이 시험에서 파싱된다. 자리표시자를 유효한 값으로 바꾼 뒤 본다. 줄 하나를 틀린 옵션으로 바꾸면 시험이 실패한다.
- F6:
  - 옛 이름(`markdown`, 최상위 `version`)을 읽는 소비자가 없음을 시험이 확인한다.
- `atip check`의 `L2.skills`, `L2.guide-commands`, `L2.help-guide`가 canon에서 통과한다. `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: 안내의 `version`은 `x-canon.version`으로 옮긴다. 안내 스키마가 최상위 멤버를 닫아 두고 `x-` 멤버를 허용하기 때문이다. 이 멤버를 읽는 소비자가 없으면 지운다.
- 가정: 이름 없는 `skills show`의 기본 안내는 canon의 시작 안내이다. 계약이 어느 안내가 기본인지 정하지 않는다. 구현자가 정하고 기록한다.
- 가정: `skills show`는 지금 `USAGE`로 모르는 이름을 거절한다. 계약은 등록된 코드이기만 하면 된다. 세분된 코드를 새로 만들지 않는다.
- 정책 질문은 없다.

## Out of scope

- 안내 본문의 내용과 구조를 바꾸는 일. 명령 줄을 고치는 것 말고는 하지 않는다.
- 설치한 스킬 파일의 형식과 `skills install`, `skills check`의 동작.
- `schema --search`.
- 1.2 선언과 L2 CI(`canon-0-35-claim-l2`).

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:7d3ea6c9a1f57816cf495c66d076bafd95ff2fdd29a2374cd9644b1cc4cd7464)
