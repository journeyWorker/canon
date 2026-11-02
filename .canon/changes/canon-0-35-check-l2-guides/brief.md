---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "L2 안내와 스트림 검사 5개와 줄 읽기 규칙을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "다섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, 명령 이름, 스트림 줄의 키는 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, 명령 이름, 스트림 줄의 키는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "검사 id와 fixture 키는 결정 대상의 철자 자체다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
---

# atip check L2: 스트림, 안내, 안내 속 명령 줄, 검색, 도움말 첫 줄

Source: https://github.com/journeyWorker/atip tag v1.2.0, approaches/checker.md §6의 L2 표와 §7.1, spec 13.5, 14, 10.11, 4.8, 저자의 세 번째 답

## Intent

L2의 나머지 행 다섯 개는 에이전트가 도구를 쓰는 입구를 다룬다. 스트림 명령의 NDJSON 줄, 바이너리 안의 안내 `skills show`, 안내가 보여 주는 명령 줄, `schema --search`의 답, 도움말 첫 줄이다. 안내에 적힌 명령이 실제 파서로 거절되어도 도구의 시험이 놓치면 에이전트가 틀린 호출을 배운다. 이 변경이 그 다섯 행을 바이너리 밖에서 증명한다.

## Desired outcome

- `--level L2`에서 `L2.stream`, `L2.skills`, `L2.guide-commands`, `L2.search`, `L2.help-guide`가 돈다.
- 스트림 명령이나 `--search`가 없는 도구는 해당 행이 `unsupported`이다.
- ==**안내 속 도구 이름으로 시작하는 모든 명령 줄이 예시와 같은 규칙으로 파싱된다.**==
- 안내는 상태 없이 답하고 아무것도 쓰지 않는다.

## User flow

1. F1 도구 작성자는 `skills show` 결과를 읽는다 → 목록, 각 안내, 모르는 이름이 시험된다 → 안내 데이터와 거절을 고친다.
2. F2 도구 작성자는 안내 속 명령 줄의 결과를 읽는다 → 파싱되지 않는 줄이 줄 번호와 이유와 함께 나온다 → ==**안내나 파서를 고친다.**==
3. F3 도구 작성자는 도움말 첫 줄의 결과를 읽는다 → `skills show`를 가리키는지 본다 → 줄을 고친다.
4. F4 도구 작성자는 스트림 명령을 시험한다 → 시작 줄, 이벤트, 끝 줄이 계약대로이다 → 스트림 모양을 고친다.
5. F5 도구 작성자는 검색을 시험한다 → fixture의 질문에 기대한 명령이 처음 다섯 안에 있다 → 순위는 권고이다.

## Step detail

### F1 L2.skills

- ==**`skills show --json`을 부르고 `data`가 `guide.schema.json`(`{name, text, guides, full?}`)을 통과한다.**== 기본 안내의 `guides`가 모든 안내 이름을 싣는다.
- `guides`의 이름마다 `skills show <이름> --json`을 부르고 같은 스키마를 통과한다. `name`이 요청한 이름이다.
- 모르는 이름 `nosuch-xyz`는 exit 2, 등록된 코드, `expected`가 안내 이름들이다.
- 모든 호출이 빈 디렉터리와 최소 환경에서 돌고 아무것도 쓰지 않는다. 텍스트 모드 출력은 판정하지 않는다.

### F2 L2.guide-commands

- 모든 안내의 `text`에서 코드 블록의 모든 줄 중 도구 이름으로 시작하는 줄을 L1.examples와 같은 규칙으로 파싱한다. 도구 이름, 명령 낱말, 옵션 이름, 9.12 문법, 값의 도메인, `required`, 제약을 본다.
- `<UPPER_CASE>` 자리표시자는 그 인수가 받는 유효한 값으로 바꾼 뒤 본다. 열거형은 첫 값, 정수는 `1`, 문자열은 `x`이다. 안내 밖(`examples[]`, `next`)의 자리표시자는 실패이다.
- 줄 이음(`\` 줄바꿈)과 따옴표는 checker.md §7.1의 POSIX 부분집합으로 읽는다. `|`, `>`, `<`(자리표시자가 아닌), `;`, `&`, `$(`, 백틱이 든 줄은 spec 14.5의 대상 밖이라 건너뛰고 `unsupported`로 보고한다.
- 검사는 줄마다 결과를 `findings[]`에 싣는다. 안내 이름과 줄 번호가 있다.

### F3 L2.help-guide

- 루트 `--help`의 첫 줄이 `skills show`도 가리킨다. L1.help-line이 `schema`를 보는 것과 별개의 행이다. 첫 줄 하나에 두 안내 문구가 모두 있어야 한다.
- 그룹과 명령의 `--help`는 이 행의 대상이 아니다. L1.help-line이 정한 대로이다.

### F4 L2.stream

- 대상은 출력 종류가 `stream`인 명령과 선언된 모든 스트림 출력(`when`)이다. 각 명령을 `--json`으로 돈다.
- 첫 줄은 `start`이고 도구 이름, 도구 버전, 명령 id, `schemaVersion`을 싣는다. 명령이 출력을 여럿 선언하면 `output`이 스트림 출력의 이름이다. 끝 줄 `result`의 `meta.output`이 같다.
- 이벤트 줄의 `data`가 그 출력의 `eventRef`를 통과한다. 마지막 줄은 `end`이고 `result`는 보통의 봉투이다. 그 봉투가 L0.ok-exit, L1.output-data와 `meta.output` 검사를 통과한다.
- 시작 줄과 끝 줄이 도구, 명령, 버전에 대해 일치한다. 종료 값이 끝 봉투의 것이다. 시작 전의 사용 오류는 `start` 없는 평범한 실패 봉투 한 줄이고 그 줄은 거절이다.
- 서버처럼 끝나지 않는 명령(예: canon의 `dashboard`)은 스트림이 아니다. 계약 저자의 답이 정했다. 서버 명령은 보고서 출력이고 `notRun`으로 뺀다. 이 행은 신호로 끝난 호출에 끝 줄이 있다고 요구하지 않는다(spec 8.4).
- 스트림 명령이 없는 도구는 `unsupported`이다. 도구의 문서에서 정한다.

### F5 L2.search

- 도구가 `schema --search`를 제공하는 경우만 돈다(SHOULD). fixture의 `search` 질문마다 `schema --search <질문> --json`을 부른다.
- 답이 `query`를 가진 `tree`이고 기대한 명령이 처음 다섯 항목 안에 있다. 순위의 질은 계약 밖이라 이 행은 권고이다. 답의 모양은 L1.tree가 본다.
- 제공하지 않는 도구는 `unsupported`이다. 도구의 문서가 `--search`를 가졌는지로 정한다. fixture에 `search`가 없는데 제공하는 도구는 `unprovided`이고 SHOULD라 레벨에 영향이 없다.

## Acceptance

- F1:
  - 정상 가짜 도구가 `skills show`, 각 안내, 모르는 이름의 시험을 모두 통과한다.
  - 실패 경로: `guides`에서 안내 이름 하나를 빼면 `L2.skills`만 실패한다. 상태가 필요한 `skills show`(빈 디렉터리에서 거절)와 파일을 쓰는 `skills show`도 같다.
  - 모르는 이름이 `USAGE`이고 `expected`가 없으면 실패이다.
- F2:
  - 안내에 옵션 이름이 틀린 줄을 넣으면 `L2.guide-commands`만 실패하고 `findings[]`에 안내 이름과 줄 번호가 있다.
  - 파이프가 든 줄과 자리표시자가 아닌 `<`가 든 줄은 건너뛰고 `unsupported`로 보고한다. 자리표시자를 값으로 바꿔 파싱한 줄이 통과한다.
  - 실패 경로: 도구 이름으로 시작하지 않는 줄은 대상이 아니다. 열거 옵션의 자리표시자는 첫 값으로 바뀐다.
- F3:
  - 루트 `--help` 첫 줄에 `skills show`가 없으면 `L2.help-guide`만 실패한다. `schema`만 가리키는 첫 줄도 실패이다.
- F4:
  - 정상 가짜 스트림이 모든 선언된 출력에서 통과한다. 시작 줄이 `output`을 이름으로 싣고 끝 줄의 `meta.output`이 같다.
  - 실패 경로: 끝 줄이 없는 스트림, `eventRef`를 어기는 이벤트, 틀린 `retryable`의 끝 결과는 `L2.stream`만 실패한다. 시작 전의 사용 오류에 `start`가 있으면 실패이다.
  - 스트림 명령이 없는 도구는 `unsupported`이고 신호로 끝난 서버 명령에는 이 행이 끝 줄을 요구하지 않는다.
- F5:
  - fixture의 질문이 기대한 명령을 처음 다섯 안에 내면 통과이다. 여섯 번째면 권고이고 레벨이 통과한다.
  - 실패 경로: `schema --search`가 없는 도구는 `unsupported`이다.
- 공통:
  - ==**L2 안내 행 5개 모두에 위반 설정 시험이 있고 소유 행 하나만 실패한다.**== 고정한 예시 사본(`stream-*`, `guide` 예시)의 digest가 `v1.2.0` 값이다.
  - 실패 경로: 행 하나를 항상 통과하도록 바꾸면 그 행의 위반 시험이 실패한다.

## Questions and assumptions

- 결정(계약 저자 답): 서버 명령은 스트림이 아니다. spec 1.2는 서버를 범위 밖으로 두고(시작 명령이 출력과 `effectNote`를 선언하는 것만 요구) spec 8.4는 신호 종료 뒤 봉투를 약속하지 않는다. canon의 `dashboard`는 보고서 출력으로 남고 `notRun: ["dashboard"]`로 뺀다. 서버 명령의 기계 판독 선언은 계약 1.3 후보이다.
- 결정(계약 저자 답): L2 행과 fixture 키는 v1.2.0에서 안정이고 대기 중인 변경이 없다.
- 가정: `L2.stream`이 canon에서는 `unsupported`이다. 스트림 명령이 없다. 가짜 도구가 이 행을 시험한다.
- 가정: 안내 줄의 파싱에서 값의 도메인과 제약은 자리표시자 값으로 대체되어도 본다. 계약 14.5가 "유효한 호출"을 요구하기 때문이다.
- 정책 질문은 없다.

## Out of scope

- 입력 문서와 배치(`canon-0-35-check-l2-input`), 멱등성과 파괴적 계획과 비동기와 효과(`canon-0-35-check-l2-safety`).
- 스트림 프로토콜의 도구 쪽 구현. canon은 스트림 명령이 없다.
- `schema --search`를 canon에 더하는 일. 지금 0.34 계획에서 SHOULD 공백으로 남아 있다.
- 서버 명령을 위한 새 fixture 키. `notRun`은 `canon-0-35-check-fixture`가 정한다.

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:57a973b296fe6dd022d3fd0eaaf37f2b45bc37fdd0652fbab98ed7d21ca099a0)
