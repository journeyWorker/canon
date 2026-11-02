---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "1.2 주장, 다섯 검사 행의 1.2 절반, 새 행, 고정 갱신을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여섯 단계가 각자 검사 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "검사 id와 옵션 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "검사 id와 옵션 이름은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "검사 id, 옵션, 오류 코드는 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사 id, 옵션, 오류 코드는 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "검사 id와 fixture 키는 결정 대상의 철자 자체다"
---

# atip check가 atip/1 1.2를 주장으로 받고 L0와 L1의 1.2 행을 돌린다

Source: https://github.com/journeyWorker/atip tag v1.2.1, approaches/checker.md §2와 §5-§7, CHANGELOG 1.2.0의 Checker 항목, approaches/migration.md §7, issue 134

## Intent

`atip check`는 `--contract`로 `1.0`과 `1.1`만 받고 fixture의 `contractVersion: "1.2"`를 거절한다. 1.2를 선언하고 L0와 L1을 주장하는 도구는 이 검사기로 증명할 수 없다. 계약 1.2의 검사기 문서는 L1에 새 행 `L1.check-order`를 더했고 `L1.grammar`, `L1.registry`, `L1.paging`, `L1.fields`에 1.2 전용 절반을 더했다. 이 행들은 1.2를 선언한 도구에서만 돌아야 하고, 1.1과 1.0을 주장하는 도구는 지금과 같은 결과를 받아야 한다. 이 검사기에 묶인 계약 예시와 스키마 사본은 1.1.x 태그이다.

## Desired outcome

- `--contract 1.2`와 fixture의 `contractVersion: "1.2"`가 주장이다. 1.1 이상에 적용되던 규칙이 1.2에도 적용된다.
- 1.2를 주장하면 L1.check-order와 다섯 행의 1.2 절반이 돈다. ==**1.1과 1.0 주장에서는 새 행이 돌지 않고 결과가 바뀌지 않는다.**==
- 보고 데이터의 `contractVersion`이 주장을 그대로 싣는다. 검사기 자신의 봉투도 1.2를 선언하고 자기 검사를 1.2 주장으로 통과한다.
- 시험에 쓰는 계약 예시와 스키마 사본이 태그 `v1.2.1`이다.

## User flow

1. F1 도구 작성자는 fixture에 `contractVersion: "1.2"`를 쓰거나 `--contract 1.2`를 준다 → 검사기가 1.2 주장으로 돈다 → ==**1.2 행의 결과를 읽는다.**==
2. F2 도구 작성자는 1.1 주장으로 돌린다 → 1.2 전용 행과 절반은 돌지 않는다 → 이전과 같은 결과를 받는다.
3. F3 도구 작성자는 `L1.check-order`의 결과를 읽는다 → 두 단계를 한 번에 어긴 호출에서 이른 단계의 코드와 `field`가 답했는지 본다 → 상태를 읽지 않는 검사의 순서를 고친다.
4. F4 도구 작성자는 `L1.paging`과 `L1.fields`의 1.2 절반을 읽는다 → `--all`, 기본값을 준 옵션, 환경 변수, `--fields`만 있고 `--json`이 없는 호출이 시험된다 → 쪽 나누기와 선택의 규칙을 고친다.
5. F5 도구 작성자는 `L1.grammar`와 `L1.registry`의 1.2 절반을 읽는다 → `limit`, `cursor`, `all`, `fields`가 flag-only이고 `STOPPED_MIDWAY` 행이 계약대로이다 → 문서와 레지스트리를 고친다.
6. F6 검사기 개발자는 시험을 돌린다 → 1.2 행마다 위반을 심은 가짜 도구가 그 행 하나만 실패시킨다 → 검사기를 믿을 수 있다.

## Step detail

### F1 주장 1.2

- `--contract`의 값은 `1.0`, `1.1`, `1.2`이다. 도움말이 세 값을 말한다. fixture의 `contractVersion`과 다르면 `USAGE`이다. 둘 다 없으면 `1.0`이다.
- 1.1 이상에서 돌던 규칙은 1.2에서도 돈다. `L0.hint-form`, `L1.hint-command`, `L2.input-usage`와 봉투와 문서의 `contract`, `contractVersion` 규칙이 해당한다.
- 1.2는 멤버, 문서 종류, 열거 값을 더하지 않는다. `L0.contract-version`은 1.2 주장에서 모든 문서의 `contractVersion`이 `"1.2"`인지, 그 뒤의 판이 더한 것이 없는지 본다.
- 보고의 `contractVersion`은 주장이다. 검사기 자신의 봉투와 `atip.fixture.json`이 1.2이고, 검사기를 자기 자신에게 `--contract 1.2`로 돌려 통과한다.
- 검사기의 도움말과 오류 메시지에서 "1.0 or 1.1" 문구가 없다.

### F2 1.2 전용 행의 범위

- ==**1.2 전용은 `L1.check-order`와 `L1.grammar`**, `L1.registry`, `L1.paging`, `L1.fields`의 1.2 절반이다.== L2 행은 이 변경이 아니다.
- 이 행들은 주장이 1.2일 때만 돈다. 1.1이나 1.0 주장에서는 결과 목록에 없거나(`L1.check-order`) 이전과 같은 판정을 한다. 새 행이 `unsupported`로 나오지 않는다.
- 같은 도구와 같은 fixture에 1.1 주장과 1.2 주장으로 돌린 결과의 차이는 1.2 전용 행뿐이다.

### F3 L1.check-order

- 대상은 모든 `read` 명령과 fixture에 올라 있는 쓰기 명령이다. 통과하는 fixture 호출 하나를 바꿔 두 단계를 한 번에 어긴 호출을 만든다. 명령이 두 단계를 모두 가질 때만 그 쌍을 만든다.
- 쌍은 다음과 같다.
  - 파싱 실패 둘: 모르는 옵션 둘, 또는 모르는 옵션과 값 타입이 틀린 값.
  - 모르는 옵션과 빠진 필수 인수.
  - 빠진 필수 인수와 깨진 도메인이나 제약.
  - 깨진 도메인 둘, 깨진 도메인과 깨진 제약.
  - 코드가 다른 제약 둘, 코드가 다른 제약과 검증자.
- 이른 단계가 답해야 한다. 순서는 argv 파싱 실패(여럿이면 argv에서 가장 앞), `required`, 도메인, 제약, 검증자이다. `required`와 도메인에서는 피연산자를 순서대로 보고 옵션을 선언 순서로 본다. 제약과 검증자는 선언 순서이다.
- 답의 코드와 `field`가 이른 단계의 것이어야 한다. 세 번째 규칙을 함께 깨야 만들 수 있는 쌍은 쓰지 않는다.
- 만들 수 있는 쌍이 하나도 없으면 결과는 `unsupported`이다. `ok` 호출이 없어 만들 수 없으면 `unprovided`이다. 이 행은 MUST이다.

### F4 L1.paging과 L1.fields의 1.2 절반

- `L1.paging`은 1.2 주장에서 다음을 더 본다. 둘 다 `paged` 호출이 낸 `next` 항목에 대해서다.
  - `--limit 1`을 `--all`로 바꾼 항목은 `CURSOR_INVALID`로 거절되지 않는다.
  - 선언된 `null`이 아닌 기본값을 가진 옵션을 스위치나 카운트가 아닌 형태로 그 기본값으로 더하면 `CURSOR_INVALID`이다. `false`인 스위치와 `0`인 카운트는 주지 않은 것과 같다(spec 10.7.2).
  - 호출이 주지 않은 옵션의 `env` 변수를 다른 유효한 값으로 두면 `CURSOR_INVALID`이다.
- `input`을 가진 명령은 `paged` 호출을 `--input -`로도 돌린다. 그 쪽의 `next`에 `--input -`와 주지 않은 기본값, `env` 값이 없고 같은 환경에서 이어져야 한다. `--json`은 바꾸지 않는다. 텍스트를 비교하지 않기 때문이다.
- `--all`이 없는 명령, 해당 옵션이 없는 명령, 해당 변수가 없는 명령은 그 부분이 `unsupported`이다.
- `L1.fields`는 1.2 주장에서 `--json` 없는 첫 선택이 `--fields` 때문에 거절되지 않아야 하고 모르는 경로는 `--json` 없이도 `USAGE`이다. 텍스트는 비교하지 않는다.
- 1.1과 1.0 주장은 두 행의 1.2 절반을 시험하지 않는다.

### F5 L1.grammar와 L1.registry의 1.2 절반

- `L1.grammar`는 1.2 주장에서 `limit`, `cursor`, `all`, `fields`가 `flagOnly: true`인지 더 본다. 기존의 `json`, `input`, `commit`, `plan-digest` 규칙은 그대로이다.
- `L1.registry`는 1.2 주장에서 도구가 `STOPPED_MIDWAY`를 등록하면 종료 값 1, `retryable` 거짓인지 본다. 1.1과 1.0 주장에서는 그 이름이 도구 자신의 코드이므로 어떤 종료 값도 허용한다.
- `STOPPED_MIDWAY`를 등록하지 않는 도구는 이 부분이 통과이다. SHOULD이기 때문이다.

### F6 시험과 고정

- 가짜 도구에 1.2 위반 설정이 행마다 있다. 각각 소유한 행 하나만 실패한다.
  - 1.2 문서에서 `--limit`이 flag-only가 아니면 `L1.grammar`. 1.1 문서에서는 아무 행도 실패하지 않는다.
  - 1.2 레지스트리의 `STOPPED_MIDWAY`가 종료 값 2이면 `L1.registry`. 1.1 레지스트리의 재시도 가능한 `STOPPED_MIDWAY`는 통과한다.
  - `L1.paging`: `--all`로 바꾼 cursor를 거절하는 도구.
  - 같은 행: 기본값을 더한 cursor를 받아들이는 도구, 환경 변수가 바뀐 cursor를 받아들이는 도구.
  - 같은 행: `--input -` 뒤의 `next`가 기본값을 싣는 도구.
  - 1.1 도구는 위 `L1.paging` 위반을 해도 통과한다.
  - `--json` 없는 `--fields`를 거절하는 도구는 `L1.fields`.
  - `L1.check-order`: 깨진 도메인과 깨진 제약에 제약의 코드로 답하는 도구.
  - 같은 행: 빠진 필수 인수와 깨진 도메인에 도메인의 코드로 답하는 도구, 깨진 도메인 둘에 뒤 옵션의 `field`로 답하는 도구.
- 시험에 쓰는 계약 예시와 `schema-doc` 스키마, 안내 스키마 사본이 태그 `v1.2.1`이다. 새 예시 `acme`(1.2)와 `tally`(1.1)가 시험에 들어가고, 1.2 규칙이 `tally`에 적용되지 않음을 보인다. 스키마 사본의 digest 시험이 갱신된다.
- `crates/atip`가 묶어 둔 `schema-doc` 스키마 사본이 `v1.2.1`의 것이다. 바뀐 것은 `flagOnly`의 설명 문구뿐이다.

## Acceptance

- F1:
  - `--contract 1.2`와 fixture의 `1.2`가 1.2 주장이다. 둘이 다르면 exit 2, `USAGE`이고 바이너리를 실행하지 않는다. `--contract 1.3`은 `USAGE`이다.
  - 정상 가짜 도구에 `--contract 1.2`는 종료 코드 0이고 보고의 `contractVersion`이 `"1.2"`이다.
  - 검사기를 자기 자신에게 `--contract 1.2`로 돌리면 통과한다. 검사기의 봉투 `meta.contractVersion`이 `"1.2"`이다.
  - 실패 경로: 1.2를 주장했는데 평범한 답에 `contractVersion: "1.1"`을 내는 도구는 `L0.contract-version` 실패이다.
- F2:
  - 같은 가짜 도구를 1.1 주장으로 돌린 결과에 `L1.check-order`가 없고 다섯 행의 1.2 절반이 돌지 않는다. 1.2 위반 설정이 1.1 주장에서는 통과한다.
  - 실패 경로: 1.2 전용 위반 설정이 1.1 주장에서 어떤 행도 실패시키면 시험이 실패한다.
- F3:
  - `L1.check-order`의 위반 설정마다 `findings[]`가 쌍과 어긋난 `field`를 싣는다. 만들 수 없는 쌍은 쓰이지 않고 보고에 건너뛴 쌍으로 남지 않아도 된다.
  - 쌍이 하나도 없는 도구는 `unsupported`이고, `ok` 호출이 없는 도구는 `unprovided`이고 MUST이므로 레벨이 통과하지 못한다.
  - 실패 경로: 이른 단계의 코드와 `field`가 아닌 답이 하나라도 있으면 그 쌍의 `findings[]`가 실패로 보고된다.
- F4:
  - `--all` 변형, 기본값 변형, 환경 변수 변형, `--input -` 변형이 각각 시험되고 위반 설정이 `L1.paging`만 실패시킨다.
  - `--all`이나 `env` 옵션이 없는 도구는 해당 부분이 `unsupported`이고 행은 통과한다.
  - 실패 경로: `--json` 없이 `--fields`를 거절하는 도구는 `L1.fields`가 실패한다.
- F5:
  - 1.2 문서에서 `limit`, `cursor`, `all`, `fields` 중 하나가 flag-only가 아니면 `L1.grammar`가 그 옵션 이름을 싣고 실패한다.
  - 실패 경로: 1.2 레지스트리의 `STOPPED_MIDWAY`가 종료 값 2이거나 재시도 가능이면 `L1.registry`가 실패한다. 등록하지 않은 도구는 통과한다.
- F6:
  - ==**1.2 행 5개 모두에 위반 설정 시험이 있고 소유 행 하나만 실패한다.**== `tally` 예시는 1.2 행에서 아무것도 실패하지 않는다.
  - 고정한 계약 사본의 digest 시험이 `v1.2.1` 값과 일치하고, 사본 한 글자를 바꾸면 실패한다.
  - 실패 경로: 행 하나를 항상 통과하도록 바꾸면 그 행의 위반 시험이 실패한다.

## Questions and assumptions

- 가정: `L1.output-data`는 소스에 이미 있고 `data`를 출력 스키마로 검증한다. 세 번째 구현은 선택 필드가 필수로 표시된 스키마를 이 행이 잡지 못했다고 보고했다. 구현자는 보고자의 사례를 재현하고, 재현되면 고치고, 이미 잡히면 같은 사례를 시험으로 고정한다. 이 변경에 이 일이 들어간다.
- 가정: `L1.check-order`에서 만들 수 있는 쌍이 하나도 없으면 `unsupported`, `ok` 호출이 없어 만들지 못하면 `unprovided`로 한다. 검사기 문서가 이 경우의 결과를 말하지 않아 계획자가 정했다.
- 가정: `L1.registry`의 1.2 절반은 `STOPPED_MIDWAY`의 종료 값과 재시도 가능 여부만 본다. 이 행의 reason이 등록된 exit 2 코드의 이름인지는 보지 않는다. 계약 문서의 행이 그것을 요구하지 않는다. 필요하면 저자에게 묻는다.
- 가정: 검사기 자신의 선언은 1.2로 올린다. 자기 검사를 1.2 주장으로 통과해야 한다. 이것은 이 변경의 완료 조건이다.
- 가정: 새 예시 사본은 파일이 늘어난 만큼 표(`TABLE.md`)와 시험 입력 목록을 함께 갱신한다. 예시 `schema-command-acme-member-list-limit-not-flag-only`는 `L1.grammar`의 거부 시험에 쓴다.
- 정책 질문은 없다.

## Out of scope

- L2 행과 L2 전용 fixture 키(`canon-0-35-check-l2-*`). `L2.input-refusals`의 1.2 문서 키 시험도 거기에 있다.
- 발견의 구조화(`canon-0-35-check-findings`)와 거절된 fixture 호출, 도움말 줄, `notRun`(`canon-0-35-check-fixture`).
- canon 자신이 1.2를 선언하는 일(`canon-0-35-claim-l2`). 이 변경은 canon의 fixture를 1.2로 바꾸지 않는다.
- 계약 문서, 스키마, 예시의 수정. 사본을 읽기만 한다.

Approved: Journey Hwang 2026-11-02T06:38:09Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:f478563c2a7f6910caafdf75fa50e927db01549c5dc52801ed4e2b5576dc2a57)
