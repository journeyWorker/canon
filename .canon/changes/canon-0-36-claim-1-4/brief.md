---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "이름과 철자는 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "이름과 철자는 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "이름과 철자는 호출자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "이름과 철자는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "이름과 철자는 결정 대상의 철자 자체다"
  - rule: "prose.length"
    section: "detail"
    reason: "선언, 트리, 명령 문서, schemas 답, 소비자, CI를 각자 정해야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "brief"
    reason: "선언, 형식, 소비자, CI를 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
---

# canon이 atip/1 1.4를 선언하고 자기 설명을 1.4 형식으로 낸다

Source: https://github.com/journeyWorker/atip tag v1.4.2 spec 4.4.1, 5.1, 7.3, 9, 15.7, 15.8, approaches/migration.md §9, approaches/checker.md의 L0.compact, L1.schemas, L1.help-json, CHANGELOG 1.4.0, canon issue #134와 #135

## Intent

canon은 `meta.contractVersion` 1.2와 L2를 선언한다. `canon schema <명령> --json`은 1.3까지의 형식으로 그 명령의 모든 옵션과 코드, 기본값, 내장 JSON Schema, 선언된 입력 스키마를 한 문서에 싣는다. `finding close` 하나가 16 KB를 넘는다. 에이전트는 명령 하나를 부르려고 그 전부를 읽는다. 계약 1.4는 자기 설명을 호출 표면만 담는 작은 문서로 바꾸고 스키마를 `--schemas` 답으로 나눈다. canon은 이 형식을 내고 1.4를 선언하며 L2를 유지한다. 1.4 선언은 1.3의 코드 이름 규칙과 `STOPPED_MIDWAY` 퇴장을 전제하므로 앞선 변경이 끝난 뒤에만 한다.

## Desired outcome

- 모든 봉투와 `schema` 답이 `contractVersion` `"1.4"`를 말하고, 트리의 `level`은 `L2`이다.
- 트리는 공유 옵션을 `options`에 한 번만 정의한다. `path`와 기본값인 멤버가 없다.
- 명령 문서는 호출 표면만 담는다. 내장 스키마, `path`, `input.schemaRef`, `input.option`, `rejects`, 내재된 옵션과 코드가 없다. `--commit`과 `--plan-digest`는 제공하는 명령에 이름으로 있다.
- ==**`canon schema <명령> --schemas --json`이 명령이 가리키는 스키마, 유도된 입력 스키마 `Input`, 검증기의 `rejects`를 낸다.**==
- 모든 JSON 문서에 공백이 없다. CI의 `atip check --contract 1.4 --level L2 --deep`이 통과한다.
- 소비자(안내, 웹사이트, 평가, conformance, 시험)가 1.4를 말한다.

## User flow

1. F1 에이전트는 `canon schema --json`으로 트리를 한 번 읽는다 → 공유 옵션, 명령과 그룹, `level` L2가 작은 문서로 온다 → ==**공유 옵션 정의를 도구 단위로 한 번 읽어 둔다.**==
2. F2 에이전트는 `canon schema finding close --json`을 읽는다 → 호출 표면만 오고 스키마와 내재 옵션이 없다 → 호출을 만든다.
3. F3 에이전트는 `canon schema finding close --schemas --json`을 읽는다 → 스키마, `Input`, `rejects`가 온다 → `data`를 검증하거나 입력 문서를 만든다.
4. F4 에이전트는 아무 명령이나 부른다 → 모든 봉투가 1.4를 선언하고 공백이 없다 → 선언된 minor의 규칙으로 읽는다.
5. F5 에이전트나 사람은 안내와 웹사이트를 읽는다 → 1.4 형식과 선언이 말해져 있다 → 형식대로 읽는다.
6. F6 릴리스 담당은 CI를 본다 → `atip check`가 1.4 L2 `--deep`으로 통과한다 → 선언이 증명되었다.
7. F7 사람은 CHANGELOG를 읽는다 → 무엇이 바뀌었고 1.3 형식을 기대한 호출자가 무엇을 해야 하는지 있다 → 소비자를 옮긴다.

## Step detail

### F1 선언과 트리

- ==**선언 상수가 `"1.4"`이고 모든 봉투, 거절, `schema` 답이 그 값을 말한다.**== `meta.schemaVersion`은 `schema` 답에서만 선언 minor이다. 다른 출력의 버전은 그 출력의 것이다. 수준은 L2이다.
- 트리 항목은 `command`와 `summary`이고 명령에는 `effect`, 그룹에는 `kind: "group"`이 있다. 명령 항목의 `kind: "command"`는 없다. canon 자신의 확장 `x-canon`(예: `roles`)은 그대로 둔다.
- `options`는 명령 둘 이상이 같은 정의로 쓰는 옵션이다. 정의가 같은지는 같은 직렬화로 판단하고, 명령은 이름으로 가리킨다. 예상 대상은 `--repo`, `--as`, `--actor`, `--session`, `--role`이다. 구현자가 실제 공유를 정한다. 어느 명령도 쓰지 않는 정의는 없다.
- 기본값인 멤버는 없다. `required`, `repeatable`, `variadic`, `flagOnly`, `async`가 거짓이면 없다. `default`가 null이면 없고 스위치는 거짓, 개수는 0이 기본이다. `valueType`이 `string`, `valueName`이 대문자 이름, 출력의 `kind`가 `report`, `schemaVersion`이 `1.0`, `input.batch`가 거짓, 빈 목록이면 없다.
- 같은 빌드의 `schema` 답은 매 실행 바이트가 같고 저장소, 설정, 네트워크, 환경 변수 없이 답한다. 쓰는 것이 없다.

### F2 명령 문서

- 명령 문서는 `summary`, `effect`, `idempotency`(쓰기와 파괴 명령), `effectNote`, `operands`, `options`, `constraints`, `validators`, `errors`, `outputs`, `input`, `examples`이다. 내장 JSON Schema가 없다. 참조는 `#/schemas/<이름>`의 형태를 유지하고 `--schemas` 답에서 풀린다.
- 내재된 옵션은 싣지 않는다: `--json`은 항상, `--input`은 `input`이 있을 때, `--fields`는 출력이 `fields`를 가질 때. 그 밖의 예약 옵션은 제공하면 싣는다. `--commit`과 `--plan-digest`는 이름으로 싣는다. 파괴 명령 다섯(`purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age`)이 이를 싣는다. 그래서 문서만 보고 맨 호출이 실행인지 미리보기인지 안다.
- 내재된 코드는 `errors`에 싣지 않는다: `USAGE`와 등록된 더 잘게 가른 대체 코드, `INTERNAL`, `input`이 있으면 `INPUT_*`, 배치가 있으면 `BATCH_INCOMPLETE`, `--commit`이 있으면 `PLAN_STALE`, `--cursor`가 있으면 `CURSOR_INVALID`이다. 이 명령이 내는 나머지 코드만 싣는다.
- 예시는 지금처럼 완전한 호출이고 `--json`이 경로 뒤에 있다.

### F3 `--schemas` 답

- 답은 `kind: "schemas"`, `command`, `schemas`, `rejects`를 갖는다. `schemas`는 명령 문서의 참조가 가리키는 스키마와 그것이 `#/schemas/<이름>`으로 가리키는 스키마를 정확히 싣는다. 각각은 draft 2020-12로 컴파일된다.
- 명령에 `input`이 있으면 `Input`이라는 이름으로 유도된 입력 스키마를 싣는다. 객체이고 `required`가 없으며 `additionalProperties: false`이다. 속성은 `operands`(배열, 피연산자마다 최대 하나, 마지막이 가변이면 여럿, 최소 하나)와 플래그 전용이 아닌 모든 옵션의 camelCase 이름이다. 반복 옵션은 값의 배열이고 `minItems`가 없다. 도메인은 싣지 않는다.
- `rejects`는 검증기마다 한 목록이다. 명령 문서의 검증기 순서와 같다. 이 목록은 시험 자료이고 호출 표면이 아니다.
- 경로 없이, 그룹 경로로, 또는 `--errors`, `--error`, `--warnings`, `--warning`과 함께 부르면 종료 값 2의 `USAGE`, `field` `--schemas`이다. 모르는 경로는 `UNKNOWN_COMMAND`이다.

### F4 선언의 앞선 조건

- 이 변경은 `canon-0-36-codes-table`(코드 이름 규칙, `STOPPED_MIDWAY` 퇴장, 경고 등록), `canon-0-36-help-json`(`--help --json` 거절), `canon-0-36-input-1-3`(1.3의 입력 규칙)이 끝난 뒤에만 시작한다.
- 모든 JSON 문서는 공백(스페이스, 탭, CR, LF)이 문자열 밖에 없다. canon은 이미 그렇게 쓰고, 이 변경이 모든 봉투와 `schema` 답에 대해 시험으로 고정한다. 숫자 철자와 이스케이프는 canon의 것이다.

### F5 소비자

- 안내: 생성된 `Commands` 블록과 `canon schema` 읽는 법을 말하는 문장이 새 형식과 내재 옵션 규칙을 말한다. 생성 블록은 `just docs`로 다시 만든다.
- 웹사이트 `cli.mdx` 영어와 한국어: `schema` 답의 형식을 설명하는 절(트리, 그룹, 명령, `schemas`, `schemaRef`, `contractVersion`)과 봉투 예시를 고친다. 두 언어가 같은 사실을 말한다.
- conformance: `schema` 답을 기대하는 케이스(`atip-1-1-*`와 그 밖)와 모든 봉투가 `contractVersion`을 담는 기대 출력을 `just conformance`로 다시 만든다. 차이를 눈으로 확인한다.
- 시험과 평가: 명령 문서의 `schemas`, `path`, `rejects`, `schemaRef`를 읽던 시험이 `--schemas` 답이나 새 형식을 읽는다. 선언 문구가 1.2인 곳(안내, 웹사이트, 평가 스크립트, `atip.claiml2` 계열 시험)은 1.4로 고친다.
- 생성 명세 `docs/cli/canon.cli.json`: 자기 설명 문서를 담는 부분은 새 형식을 따르고 drift 시험이 통과한다.
- 시나리오 문장이 1.2 선언이나 옛 형식을 말하면 새 선언과 형식으로 고친다. 고친 시나리오의 리뷰는 stale이고 다른 행위자가 다시 리뷰한다.

### F6 CI

- 검사기 픽스처 `canon.fixture.json`의 `contractVersion`이 `"1.4"`이다. CI의 canon 준수 job은 `atip check --level L2 --contract 1.4 --deep`이다(앞선 `canon-0-36-atip-dep`가 정한 형태). `just atip`이 같은 실행을 한다.
- 이 실행이 `L0.compact`, `L1.code-names`, `L1.schemas`, `L1.help-json`, `L1.registry`, `L1.warnings`와 L2의 입력, 배치, 안전, 안내 행을 모두 통과한다. 통과해야 하는 `L2.input-refusals`는 `[]`에 어느 쪽 답을 했는지 보고서에 남긴다(canon은 거절).
- 보고서 요약과 실행 시간을 evidence에 남긴다.

### F7 변경 기록

- CHANGELOG의 이번 릴리스 항목에 "canon이 atip/1 1.4를 선언한다"를 한 항목으로 싣는다. 표로 전후를 보인다: 선언, `schema` 답의 형식(트리, 그룹, 명령), `--schemas`, `--help --json`.
- 크기를 싣는다. `schema finding close`, `finding list`, `purge`, `skills show`, 트리의 전후 바이트와, 트리 한 번에 명령 하나의 합을 실측으로 적는다.
- 호출자 노트: 1.3 형식을 기대해 자기 설명을 1.3 스키마로 검증한 호출자는 1.4 문서를 거절한다. 검증하지 않고 읽는 호출자는 빠진 멤버를 이전 minor의 기본값으로 읽고 도구를 덜 쓸 뿐 더 쓰지 않는다(계약 15.8).

## Acceptance

- F1:
  - 아무 명령의 봉투와 모든 `schema` 답이 `contractVersion` `"1.4"`이고 `schema` 답의 `schemaVersion`이 `"1.4"`, 트리의 `level`이 `L2`이다. 트리에 `options`가 있고 `path`와 기본값인 멤버가 없다.
  - 실패 경로: 저장소 밖에서, 환경 변수 없이 부른 `canon schema --json`이 같은 바이트를 낸다. 명령 항목에 `kind: "command"`가 있으면 시험이 실패한다.
- F2:
  - `canon schema finding close --json`에 `schemas`, `path`, `input.schemaRef`가 없고 `--json`, `--input`, `--fields`와 `USAGE`, `INTERNAL`, `INPUT_*`가 `options`와 `errors`에 없다. 파괴 명령 다섯의 문서에 `--commit`과 `--plan-digest`가 있다.
  - ==**문서가 `--commit`을 싣는 파괴 명령의 맨 호출은 미리보기이고 실제로 그렇다.**== 문서만 보고 이를 알 수 있다.
  - 실패 경로: 문서가 내재 옵션이나 기본값인 멤버를 싣으면 시험이 실패한다.
- F3:
  - `canon schema finding close --schemas --json`이 `kind: "schemas"`, 참조된 스키마 정확히, `Input`, `rejects`를 낸다. 모든 스키마가 컴파일되고 참조가 풀린다. `Input`이 옵션에서 유도한 것과 같다.
  - 실패 경로: 경로 없는 `--schemas`, 그룹 경로, `--errors`와 함께 부른 `--schemas`는 종료 값 2의 `USAGE`, `field` `--schemas`이다.
- F4:
  - 모든 봉투와 `schema` 답에 공백이 없다. 시험이 모든 명령의 문서를 훑는다.
  - 실패 경로: 앞선 변경 셋 중 하나가 빠졌으면 `atip check` 1.4 실행이 해당 행에서 실패한다.
- F5:
  - 옛 형식의 멤버를 말하는 안내와 웹사이트 문장이 없다. `just docs`와 `just conformance`의 차이가 이 변경의 것뿐이다. 선언을 1.2로 적은 곳이 없다.
  - 실패 경로: 시험이 선언 문구를 1.2나 1.1로 적은 소비자를 찾아 실패한다.
- F6:
  - CI의 `atip check --contract 1.4 --level L2 --deep`이 통과한다. 보고서에 `L1.schemas`, `L1.help-json`, `L0.compact`, `L1.code-names`가 통과로 있다.
  - 실패 경로: 선언을 1.3으로 낮추거나 픽스처 `contractVersion`을 바꾸면 같은 실행이 실패한다.
- F7:
  - CHANGELOG 표가 전후 크기를 싣는다. 명령 다섯과 트리를 실측한다. 명령 문서는 각각 이전보다 작다. 트리는 공유 옵션이 트리로 모이면서 커질 수 있다. 대신 트리 한 번과 명령 하나의 합은 이전보다 작다. CHANGELOG는 트리가 커진 이유를 적는다.
- `cargo test --workspace`, conformance, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: 공유 옵션은 구현자가 정의가 같은 옵션을 기준으로 정한다. CHANGELOG 1.4.0의 계산은 canon에서 `repo`, `as`, `actor`, `role`, `session`을 공유로 본다.
- 가정: 검사기의 기본 실행은 이전보다 시작 횟수가 늘지만(1.4에서 18회) CI의 `--deep`은 계약 저자가 canon에서 유지하기로 했다. 실행 시간이 CI의 시간 제한에 가까우면 사람에게 알린다.
- 가정: 생성 명세 `docs/cli/canon.cli.json`은 자기 설명 문서를 새 형식으로 담는다. 이 파일이 canon 내부 모델의 완전한 형식을 담아야 하면 구현자가 그 내부 형식을 따로 유지하고 출력만 새 형식으로 내도 된다. 어느 쪽이든 drift 시험이 통과한다.
- 가정: 1.4로 선언하면 1.3까지의 호출자가 자기 설명 스키마 검증에서 거절할 수 있다. 계약 15.8이 이를 허용한다. 이 영향은 CHANGELOG에 싣는다.
- 가정: 시나리오 문장 중 1.2 선언이나 옛 형식을 말하는 것을 고치면 그 리뷰가 stale이 된다. 리뷰어가 차이표를 보고 다시 리뷰한다. 이 가정이 맞지 않으면 사람이 정책을 정해야 한다.
- 정책 질문: 이 변경은 canon이 계약에 대해 하는 주장(1.4와 L2)을 바꾼다. 사람이 이 선언을 승인한다.

## Out of scope

- 코드 이름과 이유(`canon-0-36-codes-*`, `canon-0-36-code-reasons`), `--help --json`(`canon-0-36-help-json`), 입력의 1.3 규칙(`canon-0-36-input-1-3`).
- L3 같은 더 높은 수준의 주장.
- `schema --search`와 같은 선택 기능을 더하는 일.
- 출력 데이터(`data`)의 형식 변경.

Approved: Journey Hwang 2026-11-04T16:41:36Z — "휴가 중 승인 위임(2026-11-01 대화): 공유 옵션이 트리로 모여 트리는 커질 수 있음, 트리+명령 합 기준으로 조정" (relayed by orchestrator orchestrator-main via chat) (brief sha256:a1efc7845e354a1941da7a17759437f61da80e865039b37c5ce09deec5c86c08)
