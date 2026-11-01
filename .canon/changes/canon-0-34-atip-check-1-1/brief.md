---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "검사 9개의 규칙, fixture 키, 소유 검사, 시험 방식을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "다섯 단계가 각자 입력, 환경, 판정 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "다섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "검사 id와 코드 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "검사 id와 옵션 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "검사 id와 옵션 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, 옵션, fixture 키, 코드 이름은 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, 옵션, 코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "검사 id와 fixture 키는 결정 대상의 철자 자체다"
---

# atip check가 atip/1 1.1 L0 검사 28개를 돌린다

Source: https://github.com/journeyWorker/atip tag v1.1.1, approaches/checker.md §1-§7, spec 15.7, 17

## Intent

0.33의 `atip check`는 1.0 L0 검사 19개를 돌린다. 계약 1.1의 검사기 문서는 L0에 검사 9개를 더하고 규칙을 고쳤다. 도구가 어느 판을 주장하는지 읽는 `--contract`와 fixture의 `contractVersion`이 없다. 봉투 스키마 검증은 `L0.no-null`과 섞여 있고, stderr는 MUST와 SHOULD로 나뉘지 않는다. 입력을 기다리는 프롬프트, 레지스트리와 경고 레지스트리의 일치, 힌트 형태, 그룹 뒤의 공통 옵션은 검사하지 않는다. canon의 fixture에는 쪽 나누기와 다음 단계를 내는 호출이 없어서 `L0.next`와 `L0.page`가 canon에 해당 없음으로 끝난다.

## Desired outcome

- `atip check --level L0`이 1.1 검사 id 28개를 계약 문서대로 돌린다. 도구가 1.1을 주장하면 `L0.contract`를 뺀 27개(MUST 21개, SHOULD 6개)가 적용된다.
- 도구의 문서가 주장한 판(`contractVersion`)을 선언하는지, 그 판 이후의 멤버를 쓰지 않는지 검사한다.
- ==**통과 조건 하나는 검사 하나만 소유하고, 한 위반은 그 소유 검사 하나만 실패시킨다.**==
- canon의 fixture가 쪽 나누기와 다음 단계를 내고, canon이 `canon-0-34-atip-1-1`이 출시된 뒤 27개를 모두 통과한다.

## User flow

1. F1 도구 작성자는 `atip check --bin <바이너리> --level L0 --fixture <파일> [--contract 1.1]`을 실행한다 → 주장한 판이 fixture의 `contractVersion`이나 `--contract`에서 정해진다 → 둘이 다르면 `USAGE`로 거절된다.
2. F2 도구 작성자는 검사 결과를 읽는다 → 1.1 검사 id의 결과, 소유 검사, 실패 이유가 있다 → 어느 규칙이 깨졌는지 안다.
3. F3 도구 작성자는 fixture에 `contractVersion`, `globalOptions`, 이유가 든 `registry`, `warnings`, `commands`를 쓴다 → 검사기가 필요한 항목이 없는 검사를 제공되지 않음으로 보고한다 → fixture를 빼서 통과할 수 없다.
4. F4 검사기 개발자는 프롬프트 검사를 돌린다 → pty에서 입력을 기다리는 도구는 실패한다 → 사람이 없는 환경에서 멈추는 도구를 잡는다.
5. F5 검사기 개발자는 시험을 돌린다 → 1.1 검사마다 위반을 심은 가짜 도구가 그 검사만 실패시킨다 → 검사기를 믿을 수 있다.
6. F6 canon 개발자는 CI에서 canon에 돌린다 → canon이 적용되는 27개를 통과한다 → ==**0.34가 1.1 L0를 주장할 수 있다.**==

## Step detail

### F1 판 주장

- 옵션은 `--bin`, `--level`(`L0`만), `--fixture`, `--contract <MAJOR.MINOR>`, `--json`이다. `--contract`는 fixture의 `contractVersion`을 덮는 것이 아니다. 둘이 모두 있고 다르면 exit 2, `USAGE`다. 둘 다 없으면 `1.0`이다.
- 주장이 `1.1`이면 도구의 모든 봉투가 `meta.contract`와 `meta.contractVersion: "1.1"`을 가져야 한다. 주장이 `1.0`이면 `contractVersion`이 어디에도 없고 1.1에서 생긴 멤버(`error.reason`, 레지스트리 `reasons`, 경고 레지스트리, 대괄호 경로)도 없어야 한다.
- `--level L1`과 `L2`의 거절은 이 변경에서 그대로다. L1은 `canon-0-34-atip-check-l1`이 연다.
- 검사기 자신의 봉투는 `contractVersion: "1.1"`을 가지고, 자기 레지스트리는 reason과 경고를 선언한다.

### F2 새 검사 9개

- 새로 생기는 것: `L0.envelope`(M), `L0.contract-version`(M), `L0.stderr-result`(M), `L0.stderr-quiet`(S), `L0.no-prompt`(M), `L0.registry`(M, 도구가 `schema --errors`를 제공할 때), `L0.usage-hint`(S), `L0.hint-form`(M, 1.1 주장), `L0.global-on-group`(S). 이미 있는 19개는 규칙이 아래처럼 바뀐다.
- 소유 검사는 다음과 같다.
  - `L0.envelope`는 봉투를 봉투 스키마로 검증한다. `null`과 주장보다 늦은 판의 멤버는 각 소유 검사가 보고한다.
  - `L0.no-null`은 meta와 error의 `null`만, `L0.ok-exit`는 종료 값과 `retryable`과 `ok`가 레지스트리와 같은지만 본다.
  - `L0.stderr-result`는 stderr에 오류나 결과가 실렸는지, `L0.stderr-quiet`는 JSON 모드에서 stderr가 비었는지를 본다. 0.33에서 `ok-exit`와 `unknown-command` 등에 흩어져 있던 stderr 판정이 여기로 모인다.
- `L0.contract-version`은 `schema` 답과 모든 봉투, 모든 종료 값의 호출을 대상으로 한다. `schema` 답의 `meta.schemaVersion`이 주장과 같은지도 본다.
- `L0.refusal-effects`는 0.33과 같이 거절의 부작용을 보고, 이제 `probe`가 알려 주는 원격 상태의 변화도 본다.
- `L0.hint-form`은 도구 이름과 공백으로 시작하는 모든 힌트가 `next[].command`처럼 쪼개지고 명령 한 줄만 담는지 본다. 1.0 주장이면 적용하지 않는다. 그 명령이 실제로 있는지는 L1의 `hint-command`가 본다.
- `L0.usage-hint`는 `field`가 없는 `USAGE`가 `canon schema <명령> --json` 꼴의 힌트(명령이 안 풀렸으면 `schema --json`)를 가지는지 본다. 그런 `USAGE`를 관찰하지 못하면 해당 없음이다.
- `L0.global-on-group`은 fixture의 `globalOptions` 항목마다 그룹 하나에 대해 `<도구> <그룹> --json`과 `<도구> <그룹> <옵션> --json`이 모두 `COMMAND_REQUIRED`인지 본다. fixture의 명령에서 그룹을 알 수 없으면 제공되지 않음이다.
- `L0.contract`는 1.0 주장에서만 적용된다. 1.1 주장에서는 해당 없음으로 보고한다.

### F3 fixture

- 키는 `contractVersion`, `setup`, `env`, `timeoutSeconds`, `probe`, `globalOptions`, `registry`, `warnings`, `commands`다. `registry`는 `{code, exit, retryable, reasons?}`의 배열이고 `reasons`는 이름 배열이다. 설명과 fix 문장은 fixture에 없다. `warnings`는 경고 코드 이름의 배열이다.
- `registry`에는 배열 대신 `"from-schema"`를 쓸 수 있다. 오류와 경고 레지스트리를 도구의 `schema --errors --json`과 `--warnings --json`에서 가져오고 fixture와 비교하지 않는다. 그 fixture에는 `warnings`가 없다. 도구가 `schema --errors`를 제공하지 않으면 레지스트리가 필요한 검사는 제공되지 않음이다. canon fixture는 이 값을 쓸 수 있다.
- `commands`에는 L0에 필요한 것만 쓴다. `missingValue`(필수 값 하나가 빠진 호출)와 `paged`(`meta.page.nextCursor`를 돌려주는 호출 목록)다. L1, L2 전용 키(`ok`, `domainRejects`, `fields` 등)는 있어도 무시한다.
- L0에서 어느 명령이 쪽을 나누는지는 관찰로 정한다. fixture에 올린 명령의 `ok` 호출에 `--cursor x`를 붙여 본다.
  - 모르는 옵션처럼 `field: "--cursor"`로 거절하면 그 명령은 쪽을 나누지 않는다.
  - `CURSOR_INVALID`로 거절하면 쪽을 나누는 명령이다.
  - `L0.page`는 쪽을 나누는 명령이 하나도 없을 때만 해당 없음이다. fixture에 올리지 않은 명령 때문에 해당 없음이 되지 않는다.
  - 쪽을 나누는 명령이 있는데 `paged` 호출도 관찰한 `nextCursor`도 없으면 제공되지 않음이다.
- 각 검사는 자기가 필요한 fixture 항목을 선언한다(`probe`, `registry`, `commands`). 항목이 없고 관찰한 위반도 없으면 제공되지 않음이다. ==**항목이 없어도 이미 관찰한 위반이 있으면 그 검사는 실패다.**== 예를 들어 `L0.registry`는 fixture `registry` 없이도 도구가 출력한 레지스트리에 없는 코드를 실패로 보고한다. fixture와의 비교만 제공되지 않음이다.
- `L0.registry`는 도구의 `schema --errors --json`, `--error <코드>`, 있으면 `--warnings --json`, `--warning <코드>`를 fixture와 비교한다. 코드, 종료 값, retryable, reason 이름이 같아야 한다. fixture가 `"from-schema"`이면 비교하지 않는다.
  - 항목은 `registry-entry`와 `warning-entry` 스키마를 통과해야 한다.
  - 실행 중 나온 모든 오류 코드, reason, 경고 코드가 등록되어 있어야 한다.
  - `--error <경고 전용 코드>`는 항목을 내지 않고 exit 2, `USAGE`(등록된 더 세분된 코드도 받는다), `field`가 `--error`여야 한다. `schema`를 제공하지 않으면 이 검사는 해당 없음이다.

### F4 프롬프트 검사

- 검사는 fixture의 `missingValue` 호출을 두 번 돌린다. 먼저 stdin을 `/dev/null`로 닫은 기준 실행, 다음에 stdin과 stdout을 pty에 붙인 실행이다. pty의 입력은 정규 모드(`ICANON`)이고 반향이 없고 출력은 변환하지 않는다.
- 통과는 pty 실행도 기준 실행처럼 거절하고 끝나는 것이다. 기한은 기준 시간의 두 배에 2초이고 최대 fixture의 시간 제한이다.
  - 실패는 기준 실행이 거절했는데 pty 실행이 기한까지 끝나지 않는 경우다. 이때 `VEOF`를 보내자 2초 안에 끝나거나 출력하면 프롬프트를 기다린 것이다.
  - 값을 묻는 출력이 있어도 실패다.
  - 기한까지 끝나지 않고 출력도 없고 `VEOF`에 반응도 없으면 제공되지 않음이다.
- pty 출력은 자식이 끝나고 주 쪽이 빌 때까지, 또는 EOF까지 읽은 뒤 닫는다. 출력이 잘리지 않는다. 터미널을 raw 모드로 바꾸는 도구는 `0x04` 바이트를 받는다.

### F5 시험

- 가짜 도구의 위반 설정이 1.1 검사 id마다 하나 이상 있다. 각각 소유 검사 하나만 실패하고 나머지 MUST 검사는 통과하거나 해당 없음이다. 예는 다음과 같다.
  - 1.1 도구의 평범한 답에 `contractVersion`이 없으면 `L0.contract-version`.
  - 오류를 stderr에 찍으면 `L0.stderr-result`, 진행 줄만 찍으면 `L0.stderr-quiet`(권고만).
  - 값이 빠지면 stdin에서 한 줄을 읽고 기다리는 도구는 `L0.no-prompt`.
  - 레지스트리에 없는 reason이나 경고 코드를 내면 `L0.registry`.
  - 도구 이름으로 시작하는 산문 힌트는 `L0.hint-form`(1.1 주장일 때만).
  - 그룹이 `--repo . --json`에 `USAGE`를 내면 `L0.global-on-group`(권고만).
- 1.0 주장의 가짜 도구가 도구 이름으로 시작하는 산문 힌트를 내도 통과한다. 1.0 도구가 1.1 멤버를 내면 `L0.contract-version`이 실패한다.
- 계약 저장소의 새 예시(`schema-errors-*`, `schema-warning*`, `stream-*`와 `invalid/`)를 시험 입력으로 쓴다. 봉투 예시는 `L0.envelope`를 통과하고 위반 예시는 거부된다. 사본은 계약 저장소의 태그 `v1.1.1`에 고정한다.
- 검사기는 자기 자신에게 돌아가고 통과한다.

### F6 canon에 대한 실행

- canon fixture는 `contractVersion: "1.1"`, `globalOptions`(`--repo .`), `missingValue`(필수 `--seq`가 빠진 `finding close`), `paged` 호출 하나와 다음 단계를 내는 호출 하나를 가진다. `paged` 호출은 fixture 준비가 만든 기록이 둘 이상이라 `meta.page.nextCursor`가 있다. 레지스트리는 `"registry": "from-schema"`로 쓰거나, 배열로 쓰면 `canon schema --errors --json`과 `--warnings --json`에서 생성하고 손으로 쓰지 않는다.
- CI 잡과 `just atip` 레시피가 1.1 주장으로 돌린다. canon이 27개 검사를 통과해야 잡이 초록이다. 이 단계는 `canon-0-34-atip-1-1`이 출시된 뒤에만 초록이 된다.

## Acceptance

- F1:
  - `--contract 1.1`이나 fixture의 `contractVersion: "1.1"`이 1.1 주장이다. 둘이 다르면 exit 2, `USAGE`이고 바이너리를 실행하지 않는다.
  - 1.1을 주장했는데 평범한 답에 `contractVersion`이 없는 가짜 도구는 `L0.contract-version` 실패다. 1.0을 주장한 도구가 `error.reason`을 내도 같다.
  - 검사기의 봉투가 계약 봉투 스키마를 통과한다. `--level L1`은 여전히 `LEVEL_UNSUPPORTED`이고 0.34 이후에 연다는 힌트가 있다.
- F2:
  - 정상 가짜 도구에 `--contract 1.1`은 종료 코드 0이고 `data.checks`에 28개가 있다. 적용되는 MUST 21개가 통과이고 `L0.contract`는 해당 없음이다.
  - 한 위반은 소유 검사 하나만 실패시킨다. 기존 19개의 소유 이동(stderr, 봉투 검증)은 시험에서 같은 규칙으로 확인된다.
  - 실패 경로: 소유 검사가 둘 이상인 위반이 하나라도 있으면 시험이 실패한다. 1.0 주장의 가짜 도구에서 `L0.hint-form`은 적용되지 않는다.
- F3:
  - fixture에서 `probe`나 `registry`나 `commands`를 빼면, 위반을 관찰하지 못한 해당 MUST 검사가 제공되지 않음이고 종료 코드는 1이다. 관찰한 위반이 있으면 실패다.
  - 도구가 낸 레지스트리와 fixture 배열이 다르면 `L0.registry`가 실패한다. 경고 코드나 reason 이름이 다를 때도 같다. `"from-schema"` fixture에서는 비교하지 않고, 실행 중 나온 코드가 도구의 레지스트리에 없을 때만 실패한다. `schema`를 제공하지 않는 도구는 이 검사가 해당 없음이다.
  - 경고 전용 코드로 `--error`를 부르면 `USAGE`, `field`가 `--error`, exit 2가 아닐 때 `L0.registry`가 실패한다.
  - 쪽을 나누는 명령이 `paged` 항목이 없고 관찰한 `nextCursor`도 없으면 `L0.page`가 제공되지 않음이다. 모든 올린 명령이 `--cursor`를 모르는 옵션으로 거절하면 해당 없음이다. fixture에 명령을 올리지 않아도 해당 없음이 되지 않는다.
  - 실패 경로: 모양이 틀린 fixture는 exit 2, `FIXTURE_INVALID`다. 레지스트리에 없는 reason을 내는 도구는 `L0.registry` 실패다.
- F4:
  - 값이 빠지면 프롬프트를 읽고 기다리는 가짜 도구는 `L0.no-prompt` 실패다. 거절하고 끝나는 도구는 통과한다. 오래 자면서 입력도 읽지 않고 출력도 없는 도구는 제공되지 않음이다.
  - pty 출력이 잘리지 않고 EOF까지 읽힌다. 출력이 긴 도구의 뒷부분이 보고에 남는다.
  - 실패 경로: `missingValue`가 fixture에 없으면 `L0.no-prompt`가 제공되지 않음이다.
- F5:
  - ==**1.1 검사 id 9개 모두에 대해 위반 설정 시험이 있다.**== 그 검사 하나만 실패한다.
  - 계약 저장소 v1.1.1의 봉투 예시가 모두 통과하고 거부해야 할 예시가 모두 거부된다. 검사기를 자기 자신에게 돌리면 통과한다.
  - 실패 경로: 검사 하나를 항상 통과하도록 바꾸면 그 검사의 위반 시험이 실패한다.
- F6:
  - 커밋된 canon fixture가 `"from-schema"`이거나, 배열이면 그 배열이 `schema` 답에서 생성한 것과 같다. `paged` 호출과 다음 단계 호출이 있어서 `L0.next`와 `L0.page`가 canon에서 통과로 나온다.
  - 실패 경로: canon의 힌트 하나를 도구 이름으로 시작하는 산문으로 바꾸면 CI 잡이 `L0.hint-form` 실패로 빨개진다.

## Questions and assumptions

- 계약 저자가 정했다. 검사기는 1.1 검사 id와 Needs 열, 소유 검사 원칙, `L0.next`, `L0.page`, `L0.refusal-effects`, `L0.no-prompt`, `L0.registry`, `L0.contract-version`을 따른다. L2 검사는 0.35에 온다.
- 가정: 1.1 L0 검사는 모두 28개이고 1.1 주장에서 `L0.contract`가 해당 없음이라 27개가 적용된다. 이 수는 검사기 문서 v1.1.1의 L0 표를 센 것이다. 1.1.1은 검사를 더하지 않는다. 힌트가 실제 명령인지는 L1의 `hint-command`로 옮겨 갔다.
- 가정: 새 오류 코드가 필요하면 기존 이름(`USAGE`, `BINARY_UNUSABLE`, `FIXTURE_INVALID`, `LEVEL_UNSUPPORTED`, `LEVEL_NOT_MET`)을 쓰고, `--contract`와 fixture가 다른 경우는 `USAGE`다.
- 가정: 프롬프트 검사의 기한 공식은 검사기 문서의 값(기준 시간의 두 배에 2초, 최대 fixture 시간 제한)을 그대로 쓴다.
- 가정: pty 검사는 macOS와 Linux에서만 돈다. canon의 CI 대상과 같다.
- 가정: `L1.reasons`, `L1.warnings`, `L2.input-usage`는 L1과 L2 검사라 이 변경이 아니다. L1은 `canon-0-34-atip-check-l1`이, L2는 0.35가 연다.
- 정책 질문은 없다. CI 잡의 판 주장은 gate가 요구하는 것을 바꾸지 않는다.

## Out of scope

- L1 검사(`canon-0-34-atip-check-l1`)와 L2 검사(0.35).
- ECMAScript 정규식 구현, 플랫폼별 바이너리 배포(npm), crates.io 게시, atip 레포로의 이동.
- 보고만 하고 실패하지 않는 모드, lute 같은 다른 도구에 대한 실행.
- 계약 문서, 스키마, 예시의 수정. 사본을 읽기만 한다.
- canon의 출력을 계약에 맞추는 일(`canon-0-34-atip-1-1`).

Approved: Journey Hwang 2026-11-01T11:53:57Z — "승인" (relayed by orchestrator orchestrator-main via ask) (brief sha256:541f43088c40fe7ecb0786173ac74c517dd02c19fab4ea5768a036c82add49d4)
