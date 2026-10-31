---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "봉투, 코드 표, 종료 코드, 소비자 이동을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여덟 단계가 각자 멤버 이름과 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여덟 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "계약 요약에 없던 항목과 가정이 서로 다른 결정이라 함께 적어야 한다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "멤버 이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "코드 이름과 meta 키는 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "코드 이름과 meta 키는 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "코드 이름과 meta 키는 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "코드 이름, meta 키, 보고서 이름은 호출자가 읽는 철자 자체이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "코드 이름, meta 키, 보고서 이름은 호출자가 읽는 철자 자체이고 검증 대상이다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "코드 이름과 계약 번호는 결정 대상의 철자 자체다"
---

# canon의 출력 계약을 atip/1 L0로 맞춘다

Source: https://github.com/journeyWorker/atip (atip/1 1.0) spec 4.1-4.6, 5, 6, 7, 8, 15, 16; approaches/migration.md §1 L0 표, §3.1 2번

## Intent

canon의 `--json` 출력은 atip/1 L0와 다르다. meta에 도구 이름과 계약 id가 없고, 명령을 못 찾아도 `command: "canon"`을 적고, 값 없는 항목이 `hint: null`로 나온다. 쪽 정보와 결과가 meta에 펼쳐지고, 오류 코드 이름(`PLAN_CHANGED`, 열 가지 `INPUT_*` 등)과 종료 코드 규칙이 계약과 다르다. 그래서 에이전트는 canon 전용 규칙을 따로 배워야 하고, 0.33의 `atip check`도 통과하지 못한다.

## Desired outcome

- 모든 `--json` 출력이 atip/1 L0의 봉투 규칙을 따른다. meta 멤버가 계약대로이고, 값 없는 멤버는 생략되고, 쪽 정보는 `meta.page`에 있고, 명령 결과는 data에 있다.
- 오류 코드는 계약의 예약 코드를 쓴다. 종료 코드는 코드의 레지스트리 행에서만 나온다.
- 잘못된 호출은 JSON 모드에서 `UNKNOWN_COMMAND`, `COMMAND_REQUIRED`, `USAGE`로 답한다. 루트와 그룹에서도 `--json`이 먹는다.
- ==**옛 이름은 이 릴리스에서 지워지고 별칭이 없다.**== canon JSON을 읽는 모든 소비자가 같은 릴리스에서 새 이름으로 옮겨 간다.

## User flow

1. F1 호출자는 `canon <명령> --json`의 봉투를 읽는다 → meta에서 도구 이름, 도구 버전, 계약 id, 명령, 출력 이름, 컨텍스트를 본다 → canon 전용 규칙 없이 어느 출력인지 안다.
2. F2 호출자는 봉투와 오류 개체의 선택 멤버를 읽는다 → 값이 없는 멤버는 아예 없다 → null과 빈 배열을 거르지 않아도 되고, 봉투 전체가 계약 스키마를 통과한다.
3. F3 호출자는 목록을 쪽 단위로 읽는다 → 쪽 정보는 `meta.page`에, 명령 결과는 data에 있다 → 다음 쪽 호출이 `meta.next` 항목으로 온다.
4. F4 호출자는 다음 단계와 경고를 읽는다 → 다음 단계는 `--json`이 붙은 실행 가능한 argv이고, 경고는 코드를 가진다 → 경고를 코드로 가르고 다음 단계를 그대로 실행한다.
5. F5 호출자가 잘못된 명령을 부른다 → 모르는 명령, 명령 없는 호출, 그룹만 부른 호출이 각자의 코드와 `expected`로 답한다 → 호출을 고친다.
6. F6 호출자는 오류 코드로 분기한다 → 계약이 예약한 코드만 만난다 → `INPUT_*` 열 가지를 따로 외우지 않는다.
7. F7 호출자는 종료 코드를 읽는다 → 종료 코드가 코드의 레지스트리 행과 같다 → 실행된 뒤의 실패(1)와 실행 거부(2)를 종료 코드로 가른다.
8. F8 에이전트와 사람은 canon의 안내, 문서, 대시보드, conformance를 본다 → 모두 새 이름과 모양이다 → ==**옛 이름이 남은 곳이 없다.**==

## Step detail

### F1 meta 멤버

- meta의 `schemaVersion`은 지금과 같다. 새로 `tool`(`canon`), `toolVersion`(지금 `canonVersion`이 싣는 값), `contract`(`atip/1`)가 모든 봉투에 들어간다. `canonVersion`은 없어지고 별칭도 없다.
- `command`는 풀린 명령 id다. 모르는 하위 명령이면 거기까지 풀린 경로(`finding`)다. 첫 단어도 풀리지 않으면 생략한다. 지금은 `"canon"`을 적는다. 이때 `schemaVersion`은 `1.0`이다.
- `context`의 키는 canon의 컨텍스트 옵션 이름이다. 지금은 `repo` 하나이고, 값은 작업 디렉터리 기준 상대 경로다. `role`과 `actor` 키는 없다. `--as`와 actor 환경 변수는 컨텍스트 옵션이 아니기 때문이다.
- `output`은 data가 있고 명령이 출력을 둘 이상 선언할 때 있다. 값은 명령 명세 파일이 그 출력에 적은 보고서 이름이다(`BriefApproveReport`, `InitCheckReport`). 대상은 `brief approve`, `init`, 배치 입력을 받는 쓰기 명령 전부다. 배치 입력의 결과는 배치 출력 이름(`BatchFindingCloseReport`)이다.
- canon 전용 meta는 `x-canon` 아래에만 둔다. meta의 멤버는 계약이 정한 것과 `x-canon`뿐이다.

### F2 값 없는 멤버

- 값이 없는 선택 멤버는 생략한다. 대상은 오류의 `hint`, `field`, `expected`, `received`, `suggestions`, `details`와 meta의 `next`, `warnings`, `context`다. 빈 배열도 생략한다.
- 오류의 `retryable`은 항상 있고 레지스트리 행과 같다.
- 계약 스키마 세 개(봉투, 오류, 레지스트리 행)를 원본 그대로 `crates/atip`에 둔다. 가져온 원본의 digest를 고정하고, 다르면 테스트가 실패한다. 디버그 빌드의 봉투 검증과 테스트가 이 스키마를 쓴다. canon 전용 봉투 스키마는 없앤다.
- 보고서 data 안의 null과 빈 배열은 보고서 스키마가 정한다. 이 변경은 건드리지 않는다.

### F3 쪽 정보와 펼쳐진 결과

- 쪽 정보는 `meta.page`이고 멤버는 `returned`, `nextCursor`(더 있을 때만), `total`(알 때만)이다. `hasMore`는 없어진다. 대상은 `finding list`, `divergence list`, `query`, `export`다.
- `nextCursor`가 있으면 `meta.next`에 같은 호출에 `--cursor <값>`을 붙인 항목이 있다. 마지막 쪽에는 둘 다 없다.
- meta에 펼쳐져 있던 `unchanged`, `waitingOn`, `nextOmitted`와 그 밖의 평평한 멤버(`hotStore`, `deprecated` 등)는 해당 보고서의 data 멤버가 된다. 구현자가 meta에 펼쳐진 멤버를 전수 조사해 목록을 CHANGELOG에 싣는다. data에 이미 있는 멤버는 겹쳐 두지 않는다.
- data 멤버가 늘어난 보고서는 `schemaVersion`의 minor를 올린다. 배치 입력의 항목이 모두 바뀐 것이 없으면 배치 보고서의 data에 `unchanged`가 있다.
- data에 둘 수 없는 canon 전용 값은 `x-canon`으로 간다.
- `--fields` 출력과 쪽 출력을 포함해 meta에는 계약이 정의한 멤버와 `x-canon`만 남는다. 계약 검사가 모든 봉투를 봉투 스키마로 검증하기 때문이다.

### F4 다음 단계와 경고

- 다음 단계 항목은 `command`, `argv`, `reason`(선택), `x-canon`(선택)이다. 지금의 `why`는 `reason`이 되고, `actor`와 `role` 표시는 `x-canon` 안으로 들어간다.
- `argv`는 비어 있지 않고 모든 단어가 그대로 실행된다. `command`를 셸처럼 쪼개면 도구 이름 뒤에 `argv`가 나온다.
- JSON 모드의 호출이 낸 다음 단계는 그 명령이 `--json`을 받는 한 `argv`에 `--json`을 가진다.
- 자리표시자 `<…>`가 필요한 단계는 다음 단계 항목이 아니다. 같은 안내는 오류의 `hint`나 보고서의 문장으로 간다.
- `meta.warnings`는 `[{code, message, field?}]`이다. 경고를 내는 곳 약 30곳이 각자 코드를 가진다. 같은 상황은 같은 코드이고, 코드는 오류 코드와 같은 레지스트리에 행(exit, retryable, meaning, fix)을 가진다. 중복 경고를 한 번만 내는 동작은 그대로다.

### F5 잘못된 호출

- JSON 모드는 argv에서 `--` 앞 어디든 `--json`이 있으면 켜진다. 루트와 그룹도 `--json`을 받는다. 지금은 `unexpected argument '--json'`으로 거부된다.
- 모르는 명령: exit 2, `UNKNOWN_COMMAND`, `expected`는 그 단계의 유효한 단어, `received`, `suggestions`. `canon schema`의 모르는 경로가 이미 쓰는 코드와 같은 코드로 합친다.
- 명령 없는 호출(`canon --json`, `canon finding --json`): exit 2, `COMMAND_REQUIRED`, `expected`는 그 단계의 명령 목록.
- 모르는 옵션, 값 누락, 틀린 값, 모르는 `--fields` 이름, 어긴 제약: exit 2, `USAGE`, `field`는 그 인수. 모르는 `--fields` 이름의 `expected`는 고를 수 있는 필드 목록이다.
- 텍스트 모드의 오류는 stdout을 비우고 stderr 첫 줄이 `canon: <CODE>: <메시지>`다. `field`, `expected`, `hint`는 이어지는 줄에 둔다. clap 문구를 그대로 내던 모르는 명령과 모르는 옵션도 같다.
- 철자가 제거된 옛 명령의 거절은 지금처럼 `USAGE`와 대체 철자 hint다.

### F6 예약 코드

- 코드 변경은 아래 표와 같다. 옛 코드는 어디에도 남지 않는다.

| 지금 | 새 코드 | exit |
|---|---|---|
| `PLAN_CHANGED` | `PLAN_STALE` | 2 |
| `INPUT_ELEMENT_FAILED` | `BATCH_INCOMPLETE` | 1 |
| `INPUT_BATCH_UNSUPPORTED`, `INPUT_INVALID_TYPE`, `INPUT_UNKNOWN_FIELD`, `INPUT_UNREADABLE`, `INPUT_STDIN_TTY` | `INPUT_INVALID` | 2 |
| `INPUT_STDIN_CONFLICT` | `USAGE`, `field`는 `--input` | 2 |
| `--input`이 두 번 오거나 값이 없는 호출 | `USAGE`, `field`는 `--input` | 2 |
| `INPUT_INVALID`, `INPUT_CONFLICT`, `INPUT_TOO_LARGE` | 그대로 | 2 |
| `INVALID_CURSOR` | `CURSOR_INVALID` | 2 |
| `UNKNOWN_FIELD` | `USAGE` | 2 |
| `USAGE`(모르는 명령, 명령 없는 호출) | `UNKNOWN_COMMAND`, `COMMAND_REQUIRED` | 2 |
| (없음) | `INTERNAL` | 1 |

- `INPUT_INVALID`는 `--input`의 값 자체를 읽거나 나눌 수 없을 때와 입력 문서가 조각 스키마를 어길 때만 쓴다(계약 11.7 1-2단계). `--input`이 두 번 오거나 값이 없는 호출과, 다른 옵션도 stdin을 읽는 호출(`INPUT_STDIN_CONFLICT`)은 모양 오류라 `USAGE`이고 `field`가 `--input`이다. `INPUT_STDIN_TTY`는 `INPUT_INVALID`이고 `field`가 `--input`이다. `BATCH_INCOMPLETE`의 `data` 모양과 입력 문서 포인터는 지금 그대로다.
- `INPUT_INVALID` 오류에는 `--input` 값의 형식 문제 다섯 가지를 가르는 `x-canon: {reason}`이 붙는다. 값은 `unreadable`, `tty`, `empty`, `syntax`, `batch-unsupported` 중 하나다. 계약의 오류 스키마가 `x-<tool>` 멤버를 허용한다. 입력 문서가 조각 스키마를 어기는 경우는 `field`가 문서 포인터이고 `reason`이 없다.
- canon이 만들 수 있는 예약 코드는 레지스트리에 계약의 종료 값과 retryable로 등록된다. 예약 코드 12개 중 `OPERATION_REUSED`만 canon에 `--operation`이 없어 등록하지 않는다.
- 버그로 panic이 나면 JSON 모드에서 `INTERNAL`(exit 1) 봉투가 나온다. 메시지가 보고 방법을 알려 준다. 지금은 exit 101이다. 닫힌 stdout으로 SIGPIPE를 받아 죽는 경우는 0.20처럼 신호 종료(128+n)로 남는다. 신호 종료는 레지스트리 밖이다.
- 코드 이름이 바뀐 곳은 명령 명세 파일의 오류 표, 명령별 코드 목록, 스키마 응답, CHANGELOG의 대체표다.

### F7 종료 코드

- 오류의 종료 코드는 ==**그 코드의 레지스트리 행에서만 나온다.**== 호출 지점이 바꾸는 `with_exit`를 없앤다.
- 지금 이런 곳이 10곳이다. `ADAPTER_REFUSED`는 1과 2를 쓴다. subject 상태 이동의 gate 위반이 3곳이고, `brief approve`와 evidence 배치는 값을 계산한다. 배치 항목 실패(1)와 배치 사전 검사 실패(2)도 있다.
- 한 코드가 두 종료 값을 쓰던 상황은 코드를 나눈다. 새 코드 이름은 구현자가 정하고 CHANGELOG에 대체표로 싣는다.
- `WRITE_FAILED`는 exit 1이다. 작업이 시작된 뒤 쓰기에 실패한 경우다. 작업이 시작되기 전에 알 수 있는 실패(쓸 수 없는 대상, 닿지 않는 저장소)는 새 사전 조건 코드(기본 이름 `WRITE_UNAVAILABLE`, exit 2)로 나눈다. 사전 조건은 인수 해석, 컨텍스트와 설정, 선행 조건, 이름이 가리키는 객체 확인이다.
- 레지스트리에서 exit 2이면서 이름이 작업 중 실패를 가리키는 코드 13개(`*_FAILED`, `*_UNREACHABLE`, `*_UNAVAILABLE`류)를 점검한다. 같은 상황이 작업 전과 작업 중에 모두 날 수 있을 때만 코드를 둘로 나눈다. 작업 전에만 나면 exit 2로 두고, 작업 중에만 나면 exit 1로 바꾼다.
- 0, 1, 2 밖의 종료 값은 없다. 신호 종료는 예외다.

### F8 소비자 이동

- 새 이름과 모양으로 옮기는 곳은 다음과 같다.
  - 바이너리에 내장된 스킬과 가이드 12개 파일.
  - 웹사이트 영어와 한국어 문서 12개 파일(생성 표 포함).
  - 대시보드 소스와 테스트.
  - 에이전트 dogfood 평가 스크립트.
  - conformance 케이스 52개 파일.
  - 명령 명세 파일과 설계 문서.
  - 스펙 코퍼스의 시나리오 문장(`.canon/specs`).
  - npm 런처와 canon이 쓰는 훅 문구.
  - CHANGELOG.
- CHANGELOG의 이 버전 항목은 이름이 바뀐 코드, 키, 종료 코드, 사라진 meta 멤버를 옛 이름과 대체 이름으로 모두 싣는다. 생성 표가 있으면 생성기를 쓴다.
- 안내와 문서에서 옛 이름(`canonVersion`, `PLAN_CHANGED`, `INPUT_ELEMENT_FAILED`, 합쳐진 `INPUT_*`, `INVALID_CURSOR`, `hasMore`)을 찾는 테스트가 있다. CHANGELOG의 대체표와 이 변경의 이력은 예외다.

## Acceptance

- F1:
  - `canon status --json`의 meta에 `tool: "canon"`, `toolVersion`(`canon --version`의 버전과 같다), `contract: "atip/1"`이 있고 `canonVersion`은 없다.
  - `brief approve`, `init`, 배치 입력 쓰기 명령은 `meta.output`이 명세 파일의 보고서 이름과 같다.
  - 모르는 최상위 명령의 봉투에는 `meta.command`가 없고 `schemaVersion`이 `1.0`이다. `canon finding nosuch --json`의 `meta.command`는 `finding`이다.
  - `meta.context`의 키는 `repo`뿐이고 값은 상대 경로다.
- F2:
  - `hint`가 없는 오류에 `hint` 키가 없다. 같은 방식으로 빈 `suggestions`, `next`, `warnings`가 없다.
  - conformance 케이스와 단위 테스트가 낸 모든 봉투가 `crates/atip`의 계약 스키마를 통과한다.
  - 스키마 사본의 한 글자를 바꾸면 digest 테스트가 실패한다. `meta`에 null을 넣은 봉투는 검증에서 거부된다.
- F3:
  - `canon query --kind subject --limit 1 --json`은 `meta.page`에 `returned`, `total`, `nextCursor`를 싣고 `hasMore`, `returned`, `total`이 meta 최상위에 없다. `meta.next`에 같은 호출 + `--cursor` 항목이 있다.
  - 마지막 쪽에는 `nextCursor`도 cursor 다음 단계도 없다.
  - 같은 `evidence add`를 두 번 실행하면 두 번째는 `data.unchanged`가 `true`이고 meta에는 `unchanged`가 없다. `waitingOn`도 data에 있다.
  - `--fields`를 준 `canon query ... --json`과 쪽 출력의 meta 멤버가 모두 봉투 스키마가 허용한 것이다. meta에 정의 밖 키를 하나 넣으면 봉투 검증이 실패한다.
  - 실패 경로: 발급받지 않은 cursor는 exit 2, `CURSOR_INVALID`.
- F4:
  - 다음 단계 항목에 `why` 키가 없고 `reason`이 있다. 각 `argv`에 `<`를 가진 단어가 없고, JSON 모드에서는 `--json`을 가진다. `command`를 셸처럼 쪼갠 결과가 `canon`과 `argv`를 이은 것과 같다.
  - 경고는 모두 `{code, message}` 객체이고 코드가 레지스트리에 있다. 경고를 만드는 모든 곳의 코드가 레지스트리에 있음을 테스트가 확인한다.
  - 실패 경로: 자리표시자가 필요한 안내는 다음 단계에 나오지 않고 hint에 나온다.
- F5:
  - `canon nosuch --json`은 exit 2, `UNKNOWN_COMMAND`, `expected`가 최상위 명령 목록이다.
  - `canon --json`과 `canon finding --json`은 exit 2, `COMMAND_REQUIRED`다.
  - `canon status --bogus --json`은 exit 2, `USAGE`, `field`가 `--bogus`이고 `meta.command`가 `status`다. 모르는 `--fields` 이름도 `USAGE`다.
  - `--json` 없는 `canon nosuch`는 stdout이 비고 stderr 첫 줄이 `canon: UNKNOWN_COMMAND: `로 시작한다.
- F6:
  - 레지스트리에 `PLAN_STALE`, `BATCH_INCOMPLETE`, `CURSOR_INVALID`, `COMMAND_REQUIRED`, `INTERNAL`이 있고 표의 옛 코드는 없다. 각 예약 코드의 종료 값이 계약의 값과 같다.
  - 옛 코드 문자열이 어떤 소스, 명세 파일, 스킬에도 없음을 테스트가 확인한다.
  - 실패 경로: 시험용으로 panic을 일으키면 JSON 모드에서 `INTERNAL` 봉투와 exit 1이 나온다. 종료 값 101은 나오지 않는다.
  - 닫힌 stdout으로 SIGPIPE를 받아 죽으면 신호 종료 값이고 레지스트리 코드가 아니다.
  - `--input -`를 쓰면서 다른 옵션도 stdin을 읽는 호출은 `INPUT_STDIN_CONFLICT`였고 이제 exit 2, `USAGE`, `field`가 `--input`이다. `--input`을 두 번 주거나 값 없이 주는 호출도 exit 2, `USAGE`, `field`가 `--input`이다. 터미널에서 `--input -`를 쓰는 호출은 `INPUT_INVALID`, `field`가 `--input`이다.
  - `INPUT_INVALID`가 되는 형식 문제 다섯 가지는 각 오류에 해당 `x-canon.reason`이 있다. 읽을 수 없는 파일은 `unreadable`, 터미널은 `tty`, 빈 값은 `empty`, JSON 문법 오류는 `syntax`, 단일 명령에 준 배열은 `batch-unsupported`다. 오류 개체는 계약 오류 스키마를 통과한다.
- F7:
  - 소스에서 종료 값을 덮어쓰는 호출이 없음을 테스트가 확인한다. ==**모든 오류 코드는 레지스트리의 종료 값으로 끝난다.**==
  - 쓸 수 없는 대상에 쓰는 명령은 exit 2와 사전 조건 코드이고 아무것도 바뀌지 않는다. 첫 기록을 쓴 뒤 쓰기가 실패하는 시험은 exit 1, `WRITE_FAILED`다.
  - 점검한 13개 코드의 결과가 표로 CHANGELOG에 있다. 나눈 코드, exit 1로 바꾼 코드, exit 2로 둔 코드가 갈려 있다.
- F8:
  - `cargo test --workspace`, conformance, 생성 문서 drift 테스트가 통과하고 conformance 재생성 diff는 계약 변경분뿐이다.
  - 옛 이름 검사 테스트가 안내, 웹사이트, 대시보드, 평가 스크립트에서 통과한다. 옛 이름을 하나 되살리면 실패한다.
  - 실패 경로: 소비자 한 곳이 옛 키를 읽으면 그 소비자의 테스트가 실패한다. 대시보드와 런처 테스트가 해당한다.

## Questions and assumptions

- 계약 저자가 정했다. `INPUT_*`는 F6 표대로 간다. `--input` 모양 오류(두 번, 값 없음, 다른 옵션도 stdin을 읽음)는 `USAGE`이고 `field`가 `--input`이다. `INPUT_STDIN_TTY`는 `INPUT_INVALID`다. `INPUT_INVALID`의 형식 문제 다섯 가지는 코드를 나누지 않고 `x-canon.reason`으로 가른다. 정식 분리는 atip/2 후보다. `INPUT_MISSING`은 레지스트리에 없고 시나리오 문장에만 나오므로 문장을 고친다.
- 계약 저자가 정했다. panic은 `INTERNAL`(exit 1)이고 메시지가 보고 방법을 알려 준다. SIGPIPE 종료는 신호 종료로 남는다. 계약 8.4가 신호 종료를 레지스트리 밖에 둔다.
- 계약 저자가 정했다. `*_FAILED`류 13개는 같은 상황이 작업 전과 중에 모두 날 때만 나눈다. 작업 전에만 나면 exit 2, 작업 중에만 나면 exit 1이다. `WRITE_FAILED`는 exit 1이다.
- 계약 저자가 정했다. 다음 단계의 `why`는 `reason`, `actor`와 `role` 표시는 `x-canon`이다. 자리표시자가 필요한 단계는 다음 단계에서 뺀다. 역할별 안내에서 이 단계를 읽던 에이전트 흐름이 바뀐다.
- 계약 저자가 정했다. `INVALID_CURSOR`는 `CURSOR_INVALID`가 되고, cursor 다음 단계 항목과 텍스트 모드 오류 형식을 이 변경에 넣는다. 계약 검사가 모든 봉투를 봉투 스키마로 검증하므로 `--fields`와 쪽 출력에 정의 밖 meta 키가 남지 않게 한다.
- 계약 저자가 정했다. 계약 6.9의 "cursor는 발급한 호출의 인수에 묶인다"와 6.10의 투영 규칙은 이 변경에서 하지 않고 0.34에서 한다. 이 변경이 쓰는 meta 키는 정의된 것뿐이다.
- 계약 저자가 정했다. 배치의 `data` 모양, `BATCH_INCOMPLETE`의 항목별 상태, 입력 문서 포인터는 그대로이고 0.35 일이다.
- 결정: meta에 펼쳐진 나머지 멤버는 data로 가고, 늘어난 보고서는 minor를 올린다. 계약 요약의 방향이다.
- 가정: 사전 조건 코드의 기본 이름은 `WRITE_UNAVAILABLE`이다. 계약의 예시는 `STORE_UNAVAILABLE`이다. 다른 이름을 원하면 알려 달라.
- 가정: 이 변경은 `atip-contract`와 시나리오 문장에 바뀐 이름이 나오는 출시된 subject 12개에 함께 속한다. 게이트가 다른 subject의 evidence가 오래됐다고 알리면 오케스트레이터가 그 subject에도 붙인다.
- 가정: 0.33은 breaking이다. 별칭과 호환 옵션은 없다. 순서는 `canon-0-33-atip-crate`, 이 변경, `canon-0-33-atip-check` 순이다.
- 정책 질문은 없다. 종료 코드와 코드 이름이 바뀌어도 gate가 요구하는 것과 승인자는 바뀌지 않는다.

## Out of scope

- L1(0.34): `schema` 트리, 그룹 문서, `level`, 값 타입, 타입 있는 기본값, 제약, 도메인, 검증자, 출력 스키마, `--errors` 응답 모양, help의 안내 줄, cursor의 인수 묶임, `meta.fields`와 투영, `meta.truncated`.
- L2(0.35): 입력 병합 순서와 조각 스키마, 배치 보고서(`data.items`)와 포인터, 계획 digest 형식과 미리보기 다음 단계, `--operation`, 스트림 프로토콜(`dashboard`), `skills show`의 data 모양.
- `atip check` 검사기와 canon CI 연결(`canon-0-33-atip-check`).
- `crates/atip`로 옮기는 코드(`canon-0-33-atip-crate`)와 그 밖의 모듈 추출.
- lute 전환, crates.io 게시, atip 레포로의 이동.

Approved: Journey Hwang 2026-10-31T14:55:58Z — "승인" (relayed by orchestrator orchestrator-main via ask) (brief sha256:5bd4d4f3f6682beabd6d0a9e2e041b5ed9eb240a6235d2092c015c41749eb9a7)
