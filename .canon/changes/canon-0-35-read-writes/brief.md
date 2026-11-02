---
waive:
  - rule: "prose.length"
    section: "acceptance"
    reason: "다섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
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
    section: "brief"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "한 변경에서 정해야 하는 사실이 많아 줄이면 결정이 빠진다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
---

# 거절과 읽기 명령이 선언한 것 말고는 쓰지 않고 dashboard가 서버로 선언된다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 8.2, 12.1, 12.2, 3.6, approaches/migration.md §3.1 6번, approaches/checker.md §4, 계약 저자의 답(2026-11-02)

## Intent

계약 8.2는 거절(exit 2)이 아무것도 바꾸지 않는다고 정하고, 12.2는 거절과 미리보기, `--dry-run`, `--help`, `--version`, `schema`가 어떤 효과도 내지 않는다고 정한다. 읽기 명령은 `effectNote`가 이름으로 말하는 효과(감사 줄, 캐시)만 낼 수 있다. canon은 여러 명령이 핸들러 앞에서 시나리오 색인을 새로 쓴다(`refresh_stale_index`). 그래서 `subject show`, `subject move`(`--to` 없이), `evidence add`, `evidence audit`, `review`, `divergence accept`, `artifacts store` 뒤에 오는 거절(`SUBJECT_NOT_FOUND`, `SUBJECT_INVALID_TRANSITION` 등)이 이미 쓴 색인을 남긴다. `subject show`는 JSON 모드에서도 stderr에 줄을 쓴다. `dashboard`는 `--json`으로 봉투 하나를 낸 뒤 신호를 받을 때까지 서버로 도는데 서버라는 사실을 선언하지 않는다. `L0.refusal-effects`와 `L2.effects`가 canon에서 실패하거나 서버 명령 때문에 멈춘다.

## Desired outcome

- 모든 거절이 저장소를 바이트까지 그대로 둔다. 색인 새로 쓰기가 거절 앞에서 일어나지 않는다.
- `subject show`가 캐시(시나리오 색인)를 갱신하는 효과를 `effectNote`로 선언한다. JSON 모드의 stderr는 비어 있다.
- ==**`--help`, `--version`, `schema`, `skills show`, `--dry-run`이 아무것도 쓰지 않는다.**==
- `dashboard`가 서버 명령으로 선언된다. `--json`은 봉투 하나를 내고 `effectNote`가 신호를 받을 때까지 서비스한다고 말한다. 검사기는 이 명령을 실행하지 않는다.

## User flow

1. F1 에이전트는 존재하지 않는 subject로 `canon subject show`를 부른다 → exit 2와 `SUBJECT_NOT_FOUND`가 오고 저장소가 그대로이다 → 색인이 변하지 않았음을 안다.
2. F2 에이전트는 `canon subject show <id> --json`을 부른다 → 답과 함께 색인 갱신이 선언된 캐시 효과로 일어난다 → stderr는 비어 있다.
3. F3 에이전트는 쓰기 명령이 선행 조건으로 거절된 뒤 저장소를 본다 → 변한 것이 없다 → ==**안심하고 고쳐서 다시 부른다.**==
4. F4 에이전트는 `--dry-run`, `--help`, `--version`, `schema`를 부른다 → 아무것도 쓰이지 않는다 → 탐색이 안전하다.
5. F5 에이전트는 `canon dashboard --json`을 읽는다 → 봉투 하나와 `serves until signalled`라는 효과 설명이 있다 → 서버가 끝나지 않는다는 것을 안다.

## Step detail

### F1 거절 앞의 쓰기

- ==**색인 새로 쓰기는 명령이 모든 거절 지점을 지난 뒤에만 한다.**== 대상 객체의 존재, 전이 허용, 인수 유효성처럼 색인 없이 알 수 있는 거절은 색인을 쓰기 전에 낸다.
- 거절을 알려면 새 색인이 필요한 명령인 `evidence add`의 시나리오 확인은 색인을 메모리에서 계산해 판정하고, 성공할 때만 쓴다. 거절하는 호출은 색인 파일을 쓰지 않는다.
- 해당 호출 지점은 `subject show`와 `subject move`, `evidence add`, `evidence audit`, `review`, `divergence accept`, `artifacts store`이다. 구현자가 각 지점에서 거절이 앞서는지 확인한다.
- `subject move <id>`를 `--to` 없이 부르는 호출은 아무것도 쓰지 않는다고 `effectNote`가 말한다. 지금은 색인을 쓴다. 이 호출은 거절 경로와 같은 규칙을 따른다.

### F2 subject show의 캐시

- `subject show`는 읽기 명령이다. 시나리오 색인은 파생된 캐시이고 계약 12.1은 읽기 명령이 `effectNote`가 이름 붙인 캐시 효과를 낼 수 있다고 허용한다. 조건은 둘이다. `effectNote`가 색인 갱신을 이름으로 말해야 하고, 거절도 캐시를 쓰지 않아야 한다(spec 12.2). 갱신을 유지하되 `effectNote`가 "오래된 시나리오 색인을 새로 쓴다"고 말한다.
- JSON 모드에서 stderr에 한 줄을 쓰지 않는다. 텍스트 모드의 진행 줄은 그대로 둔다. 계약은 stderr가 비어 있기를 권고하고 JSON 모드의 진행 줄은 허용한다. 이 변경은 `subject show`의 JSON 모드에서 줄을 없앤다.
- canon의 fixture가 `subject show`의 `allowedEffects`로 색인 경로를 싣는다(`canon-0-35-claim-l2`).

### F3 거절의 불변

- 모든 거절 경로(exit 2)는 스냅샷이 같다. 저장소와 `HOME`이 그대로이다. 이 규칙은 `L0.refusal-effects`가 이미 MUST로 본다.
- 이 변경은 위 호출 지점들의 거절 시험을 만든다. 거절을 일으키는 호출마다 호출 전과 후의 파일 목록과 해시가 같다.

### F4 도움말, 버전, schema, skills show, --dry-run

- 이 호출들은 아무것도 쓰지 않는다. `--dry-run`을 가진 14개 명령 모두 해당한다. 색인 새로 쓰기, 접근 감사 줄, 캐시, 텔레메트리, 스크래치 파일이 없다.
- 시험은 모든 `--dry-run`과 `--help`와 `--version` 호출의 스냅샷을 비교한다. `report check`와 `selftest`가 운영체제 임시 디렉터리에 만드는 파일은 한 실행 안에서 지워지는 한 효과가 아니고, 거절과 실패 경로에서 남으면 효과이다.

### F5 dashboard

- `dashboard`는 보고서 출력 명령으로 남는다. `--json`은 봉투 하나를 낸 뒤 서비스를 계속한다. `effectNote`가 "신호를 받을 때까지 서비스한다"고 말한다(spec 3.6).
- 계약 저자의 답이 서버를 스트림으로 두지 않았다. 스트림 프로토콜(`start`, `event`, `end` 줄)은 canon에 없다. 이 결정에 따라 `dashboard`를 스트림으로 바꾸는 일은 하지 않는다.
- canon의 fixture가 `notRun: ["dashboard"]`로 이 명령을 빼고(`canon-0-35-check-fixture`의 키) 검사기는 이 명령을 실행하지 않는다. `dashboard`가 `POST /api/briefs/approve`로 브리프를 다시 쓰는 것은 `effect: write`의 범위로 둔다.

## Acceptance

- F1:
  - 존재하지 않는 subject로 `subject show`, `subject move`, `evidence add`, `evidence audit`, `review`, `divergence accept`, `artifacts store`를 부르면 exit 2이고 호출 전후의 파일 목록과 해시가 같다. 시나리오 색인이 오래되어 있어도 같다.
  - 실패 경로: 성공하는 호출은 색인을 새로 쓴다. `subject move <id>`를 `--to` 없이 부르면 아무것도 쓰이지 않는다.
- F2:
  - `canon subject show <id> --json`이 색인을 갱신하고 stderr가 비어 있다. `schema subject show --json`의 `effectNote`가 캐시 갱신을 말한다.
  - ==**거절이 앞서는 호출에서는 색인이 새로 쓰이지 않는다.**==
  - `report check`와 `selftest`의 거절과 실패 경로를 지난 뒤 운영체제 임시 디렉터리에 canon이 만든 파일이 남지 않는다. 성공 경로도 같다. 시험은 임시 디렉터리를 이 호출 전용 경로로 지정해 호출 전후의 목록을 비교한다.
  - 실패 경로: 텍스트 모드의 `subject show`는 진행 줄을 stderr에 쓸 수 있다. JSON 모드에서는 쓰지 않는다.
- F3:
  - 모든 거절 호출의 스냅샷이 같다. `atip check`의 `L0.refusal-effects`가 canon에서 통과한다.
- F4:
  - 모든 `--dry-run`, `--help`, `--version`, `schema`, `skills show` 호출의 스냅샷이 같다. `atip check`의 `L2.effects`가 canon에서 통과한다.
- F5:
  - `canon schema dashboard --json`의 `effectNote`가 신호를 받을 때까지 서비스한다고 말하고, 출력은 보고서 하나이다. 스트림 줄이 없다.
  - 실패 경로: 검사기는 `dashboard`를 실행하지 않고 보고에 `notRun`으로 이름이 남는다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 계약 저자가 정했다. `dashboard`는 스트림이 아니다. spec 1.2는 서버를 범위 밖으로 두고 시작 명령이 출력과 `effectNote`를 선언하는 것만 요구한다. 서버가 신호를 받으면 봉투가 없다는 것을 spec 8.4가 약속하지 않는다. `notRun`은 checker.md의 다음 패치에 들어간다. 서버 명령의 기계 판독 선언은 계약 1.3 후보이다.
- 계약 저자가 정했다. `subject show`는 읽기 명령으로 남고 선언한 캐시 효과로 색인을 갱신할 수 있다(12.1). `effectNote`가 색인 갱신을 말하고 거절이 캐시도 쓰지 않는 두 조건을 지킨다(12.2).
- 가정: 색인을 메모리에서 계산해 거절을 판정하는 방식은 구현자의 선택이다. 조건은 거절이 파일을 쓰지 않는 것이다.
- 계약 저자가 정했다. 한 실행 안에서 만들고 지운 운영체제 임시 파일은 효과가 아니다. 남은 파일은 효과이다. `report check`와 `selftest`가 거절과 실패 경로에서도 임시 파일을 지우는지 이 변경이 확인한다.
- 정책 질문은 없다.

## Out of scope

- 스트림 프로토콜과 `dashboard`의 이벤트 줄.
- 읽기 명령의 접근 감사 줄(`query`, `export`)이나 `search` 색인. `effectNote`와 `allowedEffects`로 선언한다.
- 색인의 형식이나 새로 쓰는 시점의 최적화.
- `--dry-run`을 가지지 않은 명령에 새로 `--dry-run`을 더하는 일.
