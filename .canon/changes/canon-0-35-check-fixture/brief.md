---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "세 가지 검사기 동작과 fixture 키를 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "네 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "fixture 키는 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "fixture 키는 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, fixture 키, 오류 코드는 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, fixture 키, 오류 코드는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "fixture 키와 오류 코드는 결정 대상의 철자 자체다"
---

# atip check: 도움말 줄의 옵션, 거절된 fixture 호출, fixture 키 notRun

Source: https://github.com/journeyWorker/canon/issues/133, issue 134의 검사기 항목과 저자의 세 번째 답, approaches/checker.md §2와 §5, atip tag v1.2.1의 `notRun` 문장과 CHANGELOG 1.2.0과 1.2.1의 Checker 항목

## Intent

`atip check`는 두 가지를 잘못 읽는다. `L1.defaults`는 `--help`의 한 줄이 설명하는 옵션을 줄 안 어디서든 첫 `--name`으로 잡는다. 한 옵션의 설명 속 `--json`을 그 줄의 옵션으로 읽어 `--json`의 기본값을 판정했고, 한 도구에서 명령 46개가 그렇게 틀렸다. 그리고 fixture가 성공한다고 적은 호출을 도구가 거절하면 그 이유가 상태나 환경의 선행 조건이어도 `L0.page`와 `L1.paging`을 `fail`로 보고한다. 자격 증명이나 닿는 백엔드가 없는 환경에서 돌린 검사가 도구의 잘못으로 읽힌다. 마지막으로 서버처럼 실행하면 끝나지 않는 명령은 fixture에서 빼는 방법이 없다.

## Desired outcome

- 도움말 줄이 설명하는 옵션은 줄 첫머리의 첫 `--name`이다. 다른 옵션의 설명 속 `--json`은 읽지 않는다.
- fixture가 성공한다고 적은 호출이 등록된 exit 2 선행 조건 코드로 거절되면 그 호출을 쓰는 검사는 `unprovided`이고 코드를 싣는다. 상태를 읽지 않는 단계의 거절은 `fail`이다.
- ==**`unprovided`는 여전히 MUST 검사의 레벨을 떨어뜨린다.**== 환경을 주지 않고 통과할 수 없다.
- fixture 키 `notRun`이 도구가 서버처럼 끝나지 않는 명령을 이름으로 뺀다. 그 명령은 한 번만 실행하고 첫 JSON 문서를 읽은 뒤 신호로 끝낸다. 뺀 명령의 나머지 호출은 보고에 건너뜀으로 남는다.

## User flow

1. F1 도구 작성자는 `L1.defaults`의 결과를 읽는다 → 옵션마다 자기 줄의 기본값만 판정된다 → 다른 옵션의 설명 속 옵션 이름 때문에 틀린 판정을 받지 않는다.
2. F2 도구 작성자는 자격 증명이 없는 환경에서 검사기를 돌린다 → fixture의 성공 호출이 선행 조건으로 거절된 검사가 `unprovided`이고 그 코드가 보고에 있다 → 환경을 주어 다시 돌린다.
3. F3 도구 작성자는 fixture의 호출이 잘못되었다는 결과를 읽는다 → 파싱과 입력과 제약과 도메인 단계의 거절은 `fail`이다 → fixture나 도구의 선언을 고친다.
4. F4 도구 작성자는 서버 명령을 fixture의 `notRun`에 쓴다 → 검사기가 그 명령을 한 번 `--json`으로 돌려 첫 JSON 문서를 읽고 SIGTERM으로 끝낸다 → ==**끝나지 않는 명령 때문에 검사가 멈추지 않고, 그 문서가 봉투 검사를 받는다.**==

## Step detail

### F1 도움말 줄

- ==**옵션이 어느 줄을 설명하는지는 줄 첫머리에서 정한다.**== 앞 공백과 짧은 옵션(`-l, `)을 건너뛴 첫 낱말이 `--name`이면 그 줄은 `--name`을 설명한다.
- 그 줄과 이어지는 들여쓴 줄들이 한 옵션의 블록이다. 다음 옵션 줄이나 빈 줄에서 블록이 끝난다. 이 부분은 지금과 같다.
- 블록 안의 설명 속 `--json`이나 다른 옵션 이름은 블록의 옵션을 바꾸지 않는다. 한 옵션이 여러 이름을 가져도 첫 `--name`이 옵션이다.
- 도움말에 첫머리의 `--name`이 없는 줄(소개 문장, 사용법)은 어떤 옵션의 블록도 시작하지 않는다.

### F2 선행 조건으로 거절된 호출

- 대상은 fixture가 성공한다고 적은 호출이다. `ok`, `paged`, `fields`, `preview`, 비동기 시작이 해당한다.
- 그 호출이 등록된 exit 2 선행 조건 코드로 거절되면, 그 호출을 쓰는 모든 검사가 `unprovided`이다. `L0.page`와 `L1.paging`만이 아니다.
- 보고의 `findings[]`에 `kind: "refused-precondition"`, `argv`, 거절한 `code`가 있고 `reason`이 코드를 말한다. `canon-0-35-check-findings`가 정한 `code` 멤버를 쓴다.
- 거절 전에 이미 위반이 관찰된 검사는 `fail`로 남는다. 검사가 자기 주제로 일으킨 거절(`L1.paging`이 일으키는 `CURSOR_INVALID`, 모르는 `--fields` 경로의 `USAGE`)은 그 검사가 직접 판정한다.

### F3 상태를 읽지 않는 단계의 거절

- 다음 거절은 `fail`이다. 상태를 읽지 않는 단계(spec 11.7의 1-6단계)에서 나오기 때문이다.
  - `USAGE`와 도구가 대신 등록한 더 세분된 코드, `UNKNOWN_COMMAND`, 모든 `INPUT_*` 코드.
  - 명령 문서의 제약이나 검증자가 선언한 코드. 환경 변수 값에 따라 달라지는 검증자도 해당한다.
  - `L1.domains`가 인수의 도메인 거절로 관찰한 코드. 도메인은 코드를 선언하지 않는다.
- L0에는 명령 문서가 없다. 그래서 위 목록의 예약 코드만 상태를 읽지 않는 단계로 안다. 다른 exit 2 코드는 레지스트리에 등록되지 않았거나 fixture에 레지스트리가 없어도 선행 조건으로 읽는다.
- L0에서 레지스트리가 없으면 `L0.ok-exit`가 `unprovided`이므로 그 실행은 어차피 레벨을 통과하지 못한다.

### F4 notRun

- fixture 키 `notRun`은 명령 id의 배열이다. 서버처럼 끝나지 않는 명령을 이름으로 뺀다. 그 명령은 다른 어떤 검사에서도 실행하지 않는다. 예시 호출, `next` 따라가기, 제약과 도메인의 증거 호출이 모두 해당한다.
- 검사기는 `notRun`의 명령마다 한 번만 실행한다.
  - 호출은 그 명령의 첫 `ok` 호출이고, `ok`가 없으면 첫 예시이다. `--json`을 붙인다.
  - 첫 JSON 문서가 나올 때까지만 읽는다. 문서를 받으면 프로세스가 아직 살아 있는지 확인한다.
  - 살아 있으면 SIGTERM을 보내고, 유예 시간 뒤에도 남아 있으면 SIGKILL을 보낸다.
- 그 문서는 `L0.envelope`, `L0.no-null`, `L0.contract-version`, `L0.next`, `L1.output-data`가 판정한다. `L0.ok-exit`와 `L0.one-document`는 적용하지 않는다. 종료 값이 신호로 끝난 것이고 문서가 하나만 오리라고 약속할 수 없기 때문이다.
- 결과는 세 경우로 나뉜다.
  - 문서를 읽기 전에 스스로 끝난 명령은 서버가 아니므로 fixture 오류이다. `FIXTURE_INVALID`, exit 2이고 아무 검사도 돌지 않는다.
  - 시간 제한 안에 문서가 없으면 `L0.envelope`가 실패이다.
  - `ok`도 예시도 없는 명령은 그 문서를 판정할 검사가 `unprovided`이다.
- `notRun` 명령의 다른 `ok`, `paged`, `fields`, `preview` 항목은 실행하지 않는다. 보고가 `notRun` 멤버로 명령 id를, 건너뛴 항목을 이름으로 싣는다. 결과를 바꾸지 않고 fixture가 명령을 조용히 숨길 수 없다.
- 예시 문법 검사처럼 실행하지 않는 판정은 그 명령에도 한다.
- `notRun`에 있는 명령을 `commands`에도 올리면 fixture 오류이다. L1 이상에서 도구의 문서에 없는 명령 id도 fixture 오류이다. 둘 다 `FIXTURE_INVALID`, exit 2이다.
- canon의 fixture는 `notRun: ["dashboard"]`를 쓴다. `dashboard --json`이 봉투 하나를 내고 계속 서비스하는 명령이므로 이 의미에 맞는다.


## Acceptance

- F1:
  - 한 옵션의 설명 속에 `--json`이 있는 가짜 도구의 `--help`에서 `L1.defaults`가 그 옵션의 기본값만 판정하고 `--json`의 기본값은 판정하지 않는다.
  - `-l, --limit`처럼 짧은 옵션이 앞서는 줄도 `--limit`의 줄로 읽는다. 첫머리에 `--name`이 없는 줄은 옵션 블록을 시작하지 않는다.
  - 실패 경로: 옵션의 첫 `--name`이 아닌 이름이 줄 안에 있어도 그 줄의 기본값은 첫 옵션의 것으로만 비교된다. 설명 속 이름의 기본값을 어기는 도구가 있어도 실패하지 않는다.
- F2:
  - 자격 증명이 없는 가짜 도구의 `ok` 호출이 등록된 exit 2 코드로 거절되면 `L0.page`와 `L1.paging`과 그 호출을 쓰는 다른 검사가 `unprovided`이고 `findings[]`에 `refused-precondition`과 코드가 있다. 레벨은 통과하지 못한다.
  - 환경으로 그 거절을 없애면 같은 검사가 통과한다.
  - 실패 경로: 거절 전에 이미 `meta.page`의 위반이 관찰되었다면 그 검사는 `fail`로 남는다. `L1.paging`이 일으킨 `CURSOR_INVALID`는 `unprovided`가 아니다.
- F3:
  - 가짜 도구가 fixture의 `ok` 호출을 `USAGE`, `UNKNOWN_COMMAND`, `INPUT_INVALID`, 선언된 제약의 코드, 관찰된 도메인 거절 코드로 거절하면 그 검사가 `fail`이다.
  - L0에서 레지스트리가 없는 도구가 낯선 exit 2 코드로 거절하면 `unprovided`이고 레벨은 통과하지 못한다. 예약 코드로 거절하면 `fail`이다.
  - 실패 경로: 선행 조건 코드가 검증자의 코드와 같은 이름이면 검증자가 선언한 것으로 보고 `fail`이다.
- F4:
  - `notRun: ["serve"]`인 fixture에서 가짜 도구의 `serve`가 `--json`으로 한 번만 실행된다. 첫 `ok` 호출이고, `ok`가 없으면 첫 예시이다. 첫 JSON 문서를 읽은 뒤 프로세스가 살아 있음을 확인하고 SIGTERM을 보낸다. 유예 시간 뒤에도 남으면 SIGKILL을 보낸다.
  - 그 문서는 `L0.envelope`, `L0.no-null`, `L0.contract-version`, `L0.next`, `L1.output-data`가 판정하고, 나쁜 문서를 내는 도구는 소유 검사 하나만 실패한다. `L0.ok-exit`와 `L0.one-document`는 그 호출에 적용되지 않는다.
  - 보고의 해당 검사에 `notRun: ["serve"]`가 있고 `serve`의 다른 `ok`, `paged`, `fields`, `preview` 항목이 건너뜀으로 이름이 남는다.
  - 실패 경로: 문서를 읽기 전에 스스로 끝나는 명령은 exit 2, `FIXTURE_INVALID`이고 아무 검사도 돌지 않는다. 시간 제한 안에 문서를 내지 않는 명령은 `L0.envelope`가 실패한다.
  - 실패 경로: `ok`도 예시도 없는 명령은 그 문서를 판정할 검사가 `unprovided`이고 레벨이 통과하지 못한다. `commands`에도 있는 `serve`와 도구의 문서에 없는 id는 exit 2, `FIXTURE_INVALID`이다.
  - ==**빠진 명령이 보고에서 사라지는 경우는 없다.**==
  - SIGTERM을 무시하는 가짜 도구는 유예 시간 뒤 SIGKILL로 끝나고 검사기가 멈추지 않는다.


## Questions and assumptions

- 계약 저자가 정했다. `notRun`은 atip 1.2.1의 의미를 따른다. 명령마다 한 번, 첫 `ok`나 첫 예시를 `--json`으로 돌려 첫 JSON 문서를 읽고 신호로 끝낸다. 문서는 봉투, 널, 판 선언, 다음 단계, 출력 스키마 검사가 판정한다.
- 가정: 보고에 `notRun`과 건너뛴 항목의 이름을 싣는 것은 계획자가 더했다. `prepare`와 `unlisted`가 같은 이유로 보고에 이름을 싣는다.
- 가정: 스스로 끝나는 `notRun` 명령의 오류 이름은 `FIXTURE_INVALID`로 둔다. 계약이 "fixture 오류"라고만 말한다. 알 수 없는 id도 같다. L0에는 명령 목록이 없으므로 L0에서는 확인하지 않는다.
- 가정: 선행 조건과 상태 없는 단계를 가르는 목록은 checker.md §5 그대로다. 구현자가 목록의 누락을 발견하면 같은 원칙으로 추가하고 기록한다.
- 정책 질문은 없다.

## Out of scope

- 발견의 구조화(`canon-0-35-check-findings`)와 1.2 검사 행(`canon-0-35-check-1-2`).
- `unprovided`로 바뀌는 결과가 레벨 판정을 바꾸는 일. `unprovided`는 지금도 MUST 검사의 레벨을 떨어뜨린다.
- L2 검사와 L2 전용 fixture 키(`canon-0-35-check-l2-*`).
- `dashboard`를 서버로 계속 돌려 두는 일. 이 변경은 canon의 fixture에 `notRun`을 쓰는 일만 한다.

Approved: Journey Hwang 2026-11-02T14:45:08Z — "휴가 중 승인 위임(2026-11-01 대화): atip/1 1.2.1 반영분" (relayed by orchestrator orchestrator-main via chat) (brief sha256:3234bdd58a728fc195a46f37d605ee5747c62e0db76af21f589b9be9f3e7eb01)
