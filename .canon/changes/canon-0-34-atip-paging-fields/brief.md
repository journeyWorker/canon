---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "cursor 묶기, 투영 규칙, 경로 문법, 보고서 스키마 조정을 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "다섯 단계가 각자 규칙과 목록을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "questions"
    reason: "계약 저자의 결정과 가정이 서로 다른 결정이라 함께 적어야 한다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "다섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "옵션 이름과 meta 키는 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "옵션 이름과 meta 키는 호출자가 읽는 계약의 철자이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "옵션 이름, meta 키, 코드 이름은 호출자가 읽는 철자 자체이고 이 변경이 바꾸는 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "옵션 이름, meta 키, 코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "옵션 이름과 코드 이름은 결정 대상의 철자 자체다"
---

# 쪽 나누기 cursor를 인수에 묶고 --fields를 atip/1 투영으로 낸다

Source: https://github.com/journeyWorker/atip tag v1.1.1, spec 6.9, 6.10, 7.2.3, 10.9, 10.13 규칙 11과 15, approaches/migration.md §3.1 2번과 5번

## Intent

쪽을 나누는 명령(`finding list`, `divergence list`, `query`, `export`)과 `--fields`를 받는 명령(이 넷과 `status`, `context`)은 계약의 L1 규칙과 다르다. cursor는 명령과 `--kind`에만 묶여서 `--change`나 `--since` 같은 필터를 바꿔도 기록을 건너뛰거나 되풀이하는 것을 알리지 않는다. `--fields`는 snake_case와 camelCase를 모두 받고, 답의 `meta`에 고른 경로가 없고, 명령 문서에 고를 수 있는 경로의 선언이 없다. 잘못된 cursor의 이유는 메시지 문장뿐이다.

## Desired outcome

- cursor는 발급한 호출의 인수에 묶인다. `--limit`, `--cursor`, `--fields`만 바꿀 수 있고, 다른 인수가 바뀌면 `CURSOR_INVALID`로 거절된다. 거절에는 이유(reason)가 있다.
- `--fields`는 계약의 경로 문법과 투영 규칙으로 동작한다. 고를 수 있는 경로는 명령 문서가 선언하고, 답의 `meta.fields`가 실제로 고른 경로를 싣는다.
- ==**투영한 답이 그 출력의 스키마에서 `required`만 줄인 스키마를 통과한다.**==
- 쪽과 `--fields`를 쓰는 답의 meta에 계약이 정한 멤버만 있다.

## User flow

1. F1 에이전트는 쪽을 나누는 명령을 `--limit`으로 부른다 → `meta.page`와 다음 쪽을 부르는 `meta.next` 항목이 온다 → ==**그 항목을 그대로 실행해 다음 쪽을 읽는다.**==
2. F2 에이전트가 필터를 바꿔 같은 cursor를 쓴다 → `CURSOR_INVALID`와 이유가 온다 → 처음부터 다시 부른다.
3. F3 에이전트는 명령 문서의 출력에서 선택할 수 있는 경로를 읽는다 → `fields`의 `records`, `paths`, `keep`, `default`가 있다 → 고를 경로를 안다.
4. F4 에이전트는 `--fields`로 경로를 고른다 → 고른 경로와 항상 남는 경로만 오고 `meta.fields`가 그 목록이다 → 필요한 필드만 읽는다.
5. F5 에이전트가 없는 경로를 고른다 → `USAGE`와 고를 수 있는 경로가 온다 → 경로를 고쳐 다시 부른다.

## Step detail

### F1 쪽 나누기

- 쪽 정보는 `meta.page`의 `returned`, `nextCursor`, `total`이다. `nextCursor`는 더 있을 때만, `total`은 알 때만 있다. 0.33과 같다.
- `nextCursor`가 있으면 `meta.next`에 같은 호출에 `--cursor <값>`을 붙인 항목이 있다. 0.33과 같다.
- `meta.page`가 보고서의 `data` 안의 쪽 멤버와 다르면 `meta.page`가 맞다. ==**canon의 보고서는 자기 쪽 멤버를 두지 않는다.**==
- `--limit`은 정수 `N`이고 기본값은 100, 하한은 1이다. 쪽 나누기 명령은 `--limit`, `--cursor`, `--all`을 받고 `--limit`과 `--all`을 함께 줄 수 없다.

### F2 cursor 묶기

- cursor는 발급한 호출의 명령과 모든 인수에 묶인다. 예외는 `--limit`, `--cursor`, `--fields`다. 필터, `--kind`, `--since`, `--repo`가 바뀌면 거절한다.
- 거절은 exit 2, `CURSOR_INVALID`, `field`가 `--cursor`이고 reason이 있다.
  - `FOREIGN`: 이 명령이 발급한 것이 아니거나 cursor가 아니다.
  - `ARGUMENTS_CHANGED`: 다른 인수로 발급되었다.
  - `EXPIRED`: 이 명령이 이 인수로 발급했지만 가리키는 기록이 없어졌다. 지금 "the record it points after is gone"에 해당한다.
- 0.33이 cursor와 함께 다른 필터를 거절하는 경우(명령, `--kind`)는 같은 코드와 새 reason으로 계속 거절한다.
- reason 세 개는 `CURSOR_INVALID`의 레지스트리 항목에 `reasons`로 등록된다. 이 변경이 내는 reason만 등록한다.

### F3 선언

- `--fields`를 받는 명령의 출력은 `fields`를 가진다: `records`(데이터 안에서 기록 배열을 가리키는 포인터), `paths`(고를 수 있는 경로), `keep`(항상 남는 경로. 고를 수 없다), `default`(`--fields` 없이 오는 선택, `paths`의 부분집합).
- 경로는 기록의 멤버를 점으로 이은 것이다. 배열은 `name[]`로만 지난다. 이름이 식별자(`[A-Za-z_][A-Za-z0-9_]*`)가 아닌 멤버는 따옴표 대괄호(`attributes["http.status-code"]`)로 쓴다. 식별자인 이름은 대괄호로 쓰지 않는다. 선언한 map의 키도 한 단계 선택할 수 있다.
- 경로의 대소문자는 기록의 키 그대로다. 임베드된 기록은 snake_case다. 0.33이 받던 camelCase 별칭은 없어진다.
- 투영되는 출력의 보고서 스키마는 계약의 투영 가능 조건을 지킨다. 기록, 기록 위 조상, 선언한 경로의 조상이 `oneOf`, `allOf`, `const`, `enum`이나 `uniqueItems`를 쓰지 않는다.
- `fields`는 출력마다 선언한다. 답이 두 모양 중 하나인 명령(`export`의 세 변형)은 두 방식 중 하나를 고른다. 보고서 출력을 모양마다 두고 각자 `fields`와 `when`을 주거나, 합집합 모양 하나를 그대로 두고 `fields`를 선언하지 않고 `--fields`를 받지 않는다.
- `--fields`는 그 호출이 고르는 출력에 대해 읽는다. 모양을 바꾸는 보고서는 `schemaVersion`을 올린다. 대상 출력은 `finding list`, `divergence list`, `query`, `status`, `context`와, 나눈 경우의 `export`다.

### F4 투영

- 남기는 경로는 고른 경로와 `keep`이다. 고른 경로가 덮는 멤버는 통째로 남고, 고른 경로의 조상은 그 경로를 지나는 멤버만 남긴다. 기록에 있는 선택된 멤버는 그대로 있고 없는 멤버는 만들지 않는다.
- `meta.fields`는 남긴 경로 목록이다. `--fields`가 없으면 `default`가 있고, 선언한 `default`가 없으면 `meta.fields`가 없다.
- 투영한 `data`는 그 출력의 스키마를 통과하되, 기록과 그 아래의 객체에서 `required`는 남은 멤버로 줄인다. 기록 위의 `required`는 줄지 않는다.
- `--fields`와 쪽 나누기를 함께 쓰는 답의 meta에는 `page`, `fields`, `next`, `context`, `warnings`, `x-canon`처럼 계약이 정의한 멤버만 있다.
- 크기 때문에 잘리는 답은 canon에 없다고 본다. 있으면 `meta.truncated`를 계약대로 낸다.

### F5 잘못된 경로

- 없는 경로는 exit 2, `USAGE`, `field`가 `--fields`, `expected`가 고를 수 있는 경로 목록이다. 0.33의 `UNKNOWN_FIELD`는 이미 `USAGE`다.
- 배열을 `[]` 없이 지나는 경로(`owners.email`), `keep`에 든 경로, 대괄호 문법이 틀린 경로, NUL이 든 키도 같다. `--fields`를 빈 값으로 주면 `USAGE`다.
- 한 호출의 `--fields`에서 같은 경로가 두 번 나와도 한 번만 남긴다.

## Acceptance

- F1:
  - ==**`canon query --kind subject --limit 1 --json`이 `meta.page`와 `--cursor`가 붙은 `meta.next` 항목을 낸다.**== 그 항목을 그대로 실행하면 다음 쪽이 오고 마지막 쪽에는 둘 다 없다.
  - 실패 경로: `--limit 0`은 exit 2, `USAGE`, `field`가 `--limit`이다. `--limit 1 --all`은 `USAGE`다.
- F2:
  - 같은 cursor로 `--kind`를 바꾸면 `CURSOR_INVALID`, reason이 `ARGUMENTS_CHANGED`다. 다른 필터(`--since`, `finding list --change`)를 바꿔도 같다.
  - 다른 명령의 cursor는 `FOREIGN`이고 임의 문자열도 `FOREIGN`이다. 가리키는 기록을 지운 뒤의 cursor는 `EXPIRED`다.
  - `--limit`과 `--fields`만 바꾼 호출은 거절되지 않고 같은 쪽 위치에서 이어진다.
  - 실패 경로: 거절은 아무것도 쓰지 않고, 모든 `CURSOR_INVALID`가 reason을 가지고 레지스트리에 등록되어 있다.
- F3:
  - `canon schema query --json`, `finding list`, `divergence list`, `status`, `context`의 출력에 `fields`가 있고 `default`가 `paths`의 부분집합, `keep`과 `paths`가 서로소다. `export`를 모양마다 나눴다면 출력마다 `fields`와 `when`이 있고, 나누지 않았다면 `fields`가 없고 `--fields`는 `USAGE`다.
  - 선언된 모든 경로가 기록 스키마의 멤버이고 배열은 `[]`로만 지난다. 식별자가 아닌 이름이 선언에 있으면 대괄호로 쓴다.
  - 실패 경로: 투영 가능 조건을 어기는 스키마나 기록 스키마에 없는 경로를 선언하면 테스트가 실패한다.
- F4:
  - `canon query --kind subject --fields subject_id,status --json`의 기록이 두 키만 갖고 `meta.fields`가 `keep`을 더한 경로 목록이다. 고르지 않은 멤버가 어떤 깊이에도 없다.
  - 투영한 `data`가 `required`를 줄인 스키마를 통과한다. 선택한 멤버는 투영하지 않은 같은 호출의 답에 있을 때마다 있다.
  - 대괄호 경로와 `[]` 경로 선택이 단위 테스트에서 계약의 예와 같이 투영된다.
  - 실패 경로: 투영 가능 조건을 어긴 스키마의 출력에서 선택하면 테스트가 실패하고, meta에 정의 밖 키가 있으면 봉투 검증이 실패한다.
- F5:
  - `--fields nosuch`는 exit 2, `USAGE`, `field`가 `--fields`이고 `expected`가 경로 목록이다. camelCase로 쓴 `changeIds`는 거절된다. `--fields ""`와 `--fields subject_id[]`도 거절된다.
  - 거절은 아무것도 쓰지 않는다. 실패 경로 자체가 이 단계의 기대다.
- 공통: `cargo test --workspace`, conformance, 생성 문서 drift 테스트가 통과하고 옛 camelCase 필드 별칭을 읽던 소비자의 테스트가 갱신된다.

## Questions and assumptions

- 계약 저자가 정했다. 쪽 나누기(6.9)와 `--fields`(6.10)는 L0 규칙이고, 그 기능을 제공하는 도구에는 MUST다. 이 변경이 0.34 범위다.
- 계약 저자가 정했다. cursor는 `--limit`, `--cursor`, `--fields`를 뺀 모든 인수에 묶는다. camelCase 필드 이름 별칭은 없앤다. 멤버마다 철자는 하나이고(6.10) 별칭은 두지 않는다(9.10). 이 별칭에 의존하는 호출이 깨지므로 CHANGELOG의 대체 표에 싣는다.
- 가정: 투영 가능 조건을 지키려고 모양이 바뀌는 보고서(합집합 변형을 가진 `export` 등)는 그 출력의 `schemaVersion` 주 버전을 올린다. 데이터가 같고 모양만 달라지는 경우 구현자가 minor와 major를 계약 15.3으로 정한다. `export`를 모양마다 출력으로 나눌지, 합집합 하나로 두고 `fields`를 선언하지 않을지는 구현자가 정하고 변경 기록에 남긴다. 계약 1.1.1은 두 방식을 모두 허용한다.
- 가정: canon에는 크기로 잘리는 답이 없다. 구현자가 확인하고, 있으면 `meta.truncated`를 이 변경에서 낸다.
- 가정: canon의 어떤 `--fields` 출력도 식별자가 아닌 이름의 멤버나 선언할 map을 갖지 않는다. 경로 문법의 대괄호와 map 키는 `crates/atip`에 구현하고 단위 테스트로 증명한다. 구현자가 canon 출력에서 해당하는 곳을 찾으면 선언에 쓴다.
- 가정: `CURSOR_INVALID`의 reason은 위 세 개만 등록한다. 계약의 예약 이름이다.
- 정책 질문은 없다. 읽기 출력의 모양이 바뀌어도 gate가 요구하는 것과 승인자는 바뀌지 않는다.

## Out of scope

- 1.1 채택의 `contractVersion`, `INPUT_*`의 reason, 경고 레지스트리(`canon-0-34-atip-1-1`).
- 트리, 그룹, 명령 문서의 틀과 값 타입(`canon-0-34-atip-schema-doc`). 이 변경은 그 문서에 `fields` 멤버를 더한다.
- 제약, 도메인, 검증자(`canon-0-34-atip-constraints`).
- 검사기의 `L1.paging`과 `L1.fields`(`canon-0-34-atip-check-l1`).
- 0.35: cursor를 쓰는 배치, `--input`으로 주는 `--fields`, stream.

Approved: Journey Hwang 2026-11-01T11:53:57Z — "승인" (relayed by orchestrator orchestrator-main via ask) (brief sha256:ef07ccb9ba778dbbe086c1b4c0ebf69a3a885263682c8e52a76838357e84b4bf)
