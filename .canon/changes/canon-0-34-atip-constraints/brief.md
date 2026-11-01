---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "인수 관계 표, 도메인, 검증자, 정수 범위를 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "여섯 단계가 각자 규칙 목록과 명령 이름을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "옵션 이름과 코드 이름은 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "옵션 이름과 규칙 이름은 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "옵션 이름, 규칙 이름, 코드 이름은 사용자가 쓰는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "옵션 이름과 코드 이름은 검증 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "옵션 이름과 코드 이름은 결정 대상의 철자 자체다"
---

# 명령 인수의 제약, 도메인, 검증자를 선언하고 실제 파서와 맞춘다

Source: https://github.com/journeyWorker/atip tag v1.1.1, spec 10.7.4, 10.7.5, 10.8, approaches/rust.md §3.3, §4.2-§4.4, approaches/migration.md §3.1 3번

## Intent

canon은 인수 사이의 관계와 값의 범위를 파서와 핸들러에서 검사하지만 `schema`는 일부만 싣는다. clap의 `conflicts_with`와 `requires`만 옵션 항목에 있고, `requires`는 clap의 `Debug` 문자열에서 읽는다. `evidence add`의 `--task`, `--scenario`, `--all-scenarios` 관계, `finding close`의 `--reason` 조건, `retrieve`의 `--regime`과 `--domain` 중 하나, 시각의 달력 검사, `--round`의 하한은 핸들러에서만 안다. `export --before`가 `--after`보다 앞서도 거절하지 않고, 64비트 정수 옵션은 2^53-1을 넘는 값도 받는다.

## Desired outcome

- 인수 사이의 모든 관계, 값 하나의 모든 제한, 그 밖의 상태 없는 검사가 명령 문서의 `constraints`, 인수의 `domain`, `validators`에 선언된다.
- ==**선언된 것은 실제 파서와 핸들러가 그대로 거절한다.**== 선언과 동작이 어긋나면 테스트가 실패한다.
- 2^53-1을 넘는 정수와 순서가 뒤집힌 `export` 기간은 거절된다.
- 거절하는 코드와 종료 값은 지금과 같다. 바뀌는 것은 위 두 새 거절뿐이다.

## User flow

1. F1 에이전트는 명령 문서의 `constraints`를 읽는다 → clap이 강제하는 관계가 `atMostOne`, `exactlyOne`, `atLeastOne`, `requires`, `forbids` 규칙으로 있다 → 같이 못 주는 옵션을 안다.
2. F2 에이전트는 핸들러가 강제하던 관계를 읽는다 → 같은 규칙 어휘와 그 위반 코드가 있다 → 호출해 보기 전에 필요한 조합을 안다.
3. F3 에이전트는 옵션의 `domain`을 읽는다 → 숫자 범위, 문자열 길이, 패턴이 있다 → 허용 값을 구성한다.
4. F4 에이전트는 `validators`를 읽는다 → 규칙과 도메인으로 못 쓰는 검사가 설명과 거절되는 호출 예와 함께 있다 → 달력에 없는 날짜나 뒤집힌 기간을 피한다.
5. F5 에이전트가 2^53-1을 넘는 정수를 준다 → 거절된다 → 반올림된 값으로 일이 일어나지 않는다.
6. F6 개발자는 테스트를 돌린다 → 선언한 제약, 도메인, 검증자가 실제 파서와 같은 답을 낸다 → ==**선언이 거짓이 될 수 없다.**==

## Step detail

### F1 clap의 관계

- 규칙은 계약 10.8의 다섯 가지뿐이다. 명령은 같은 관계를 한 번만 적는다.
  - `conflicts_with`와 `conflicts_with_all`은 `atMostOne`이다.
  - 필수이고 한 개만 받는 `ArgGroup`은 `exactlyOne`이고, 여러 개를 받는 필수 그룹은 `atLeastOne`이다.
  - `requires`는 `requires`이고, 지금 `Debug` 문자열에서 읽던 것을 규칙표에 직접 적는다.
  - `required_unless_present`는 `atLeastOne`이다. `evidence add`의 `--class`와 `--ref`가 `--report` 없이는 필수인 관계가 해당한다.
- `--commit`과 `--plan-digest`가 서로를 요구하는 관계는 `requires` 두 개다. `--limit`과 `--all`은 `atMostOne`이다. 이 두 관계는 5개 명령과 4개 목록 명령에 있다.
- 지금 필수 옵션으로만 보이는 것은 관계를 잃지 않게 `required`와 `constraints`를 함께 쓴다.

### F2 핸들러의 관계

- 핸들러가 상태를 읽지 않고 강제하는 관계는 모두 선언한다. 규칙에 코드를 붙이는데, 코드는 지금 핸들러가 내는 것이다(`EVIDENCE_INVALID_ARGUMENT`, `FINDING_INVALID_ARGUMENT`, `FINDING_REASON_REQUIRED`, `QUERY_INVALID_ARGUMENT`, `DISPATCH_INVALID_ARGUMENT`, `DIVERGENCE_INVALID_ARGUMENT`, `EVIDENCE_APPROVAL_INVALID`, `FINDING_INTRODUCED_REQUIRED`). `retrieve`의 `USAGE`는 그대로다.
- 알려진 관계는 다음과 같다. 위반 때의 종료 값과 코드는 지금과 같다.

| 명령 | 관계 | 어휘 |
|---|---|---|
| `retrieve` | `--regime`과 `--domain` 중 정확히 하나, `--subject`는 `--domain`을 요구 | `exactlyOne`, `requires` |
| `review scenario`, `review change` | `--upstream-ref`와 `--original-spec-ref` 중 정확히 하나. 핸들러의 같은 검사는 도달할 수 없으므로 지운다 | `exactlyOne` |
| `evidence add` | `--task`, `--scenario`, `--all-scenarios` 중 하나 이상, `--all-scenarios`는 `--task`와 `--scenario`와 함께 못 쓴다, `--scenario`는 `--project`를 요구, `--all-scenarios`는 `--report`와 `--project`를 요구, `--report-case`는 `--report`를 요구 | `atLeastOne`, `forbids`, `requires` |
| `evidence add` | 승인 옵션 네 개(`--approver`, `--approval-role`, `--signature-file`, `--approval-at`)는 하나를 주면 모두 필요 | `requires` 여러 개 |
| `evidence approval-payload` | `--task`와 `--scenario` 중 하나 이상, `--scenario`는 `--project`를 요구 | `atLeastOne`, `requires` |
| `dispatch begin` | `--skill-digest`는 `--skill`을 요구 | `requires` |
| `divergence add`, `divergence defer` | `--state deferred`이면 `--reason`과 `--expiry`가 필요 | `requires`의 `in` |
| `finding close` | `--disposition`이 `rejected`나 `deferred`이면 `--reason`이 필요 | `requires`의 `in` |
| `query` | `--change`, `--status`, `--domain`은 `--kind`가 허용하는 종류에서만 | 규칙으로 못 쓰면 검증자 |
| `finding add` | `--severity`가 `blocker`나 `should-fix`이면 `--introduced-rev`나 `--introduced-unknown` 중 하나가 필요, 둘을 함께는 못 줌 | `atMostOne` + 검증자 |

- 규칙 어휘로 못 쓰는 관계(한 값에 따라 둘 중 하나가 필요한 것 등)는 `validators`로 선언하고 `rejects` 호출을 가진다. 위 표는 시작점이다. 구현자가 모든 `*_INVALID_ARGUMENT`류 핸들러 검사를 전수 조사해 빠진 것을 더하고, 조사 결과를 변경 기록에 남긴다.
- 상태를 읽는 검사는 제약이 아니다. `subject move`의 `--reason`은 저장된 상태에 달려 있어 해당 없다.

### F3 도메인

- `domain`은 인수 하나의 값만 제한한다. 계약의 형식만 쓴다. 숫자는 `minimum`, `maximum` 등, 문자열은 `minLength`, `maxLength`, `pattern`이다.
- 알려진 도메인은 다음과 같다.
  - `--round`와 `--seq`(`finding add`, `finding close`)는 `minimum: 1`이다. 위반은 지금처럼 `FINDING_INVALID_ARGUMENT`다.
  - `--limit`(쪽 나누기 4곳, `search`)과 `--max-artifact-mib`(5곳)는 `minimum: 1`이다.
  - id 종류의 문법은 `pattern`이다.
  - 대상은 change, subject, project, scenario, session id다.
  - role, run, task, strategy, review id도 같다.
  - 패턴은 계약의 제한된 I-Regexp 형식이고 Rust `regex`로도 컴파일된다.
  - 패턴으로 못 쓰는 문법은 길이 제한과 검증자로 나눈다.
  - 비어 있으면 안 되는 문자열과 한 줄이어야 하는 문자열은 `minLength`와 한 줄 패턴이다.
- 모든 도메인은 인수의 기본값을 만족한다. 도메인과 실제 파서가 경계값 바로 안과 밖에서 같은 답을 내야 한다.
- 이 변경은 지금 제한이 없는 인수(`retrieve --limit` 등)에 새 제한을 만들지 않는다.

### F4 검증자

- 검증자는 `{arguments, code, description, rejects}`다. `rejects`는 파싱되고 모든 필수와 제약과 도메인을 만족하지만 이 검증자가 거절하는 완전한 호출 하나 이상이다. `examples`에는 없다.
- 검증자는 `--input` 유무와 상관없이 모든 호출에서, 선행 조건이나 상태 읽기보다 먼저 돈다. 저장소가 없는 빈 디렉터리에서 `rejects` 호출을 돌려도 선행 조건 코드 대신 검증자의 코드가 나온다.
- 시각 옵션 12개(`--since`, `--before`, `--after`, `--expiry`, `--as-of`, `--approval-at` 등)는 RFC 3339나 `YYYY-MM-DD`다. 도메인 패턴은 모양만 받고, 달력에 없는 날짜(`2026-02-30`)는 검증자가 거절한다. 거절 코드는 지금 파서가 내는 것이다.
- `export --after`가 `--before`보다 늦으면 거절한다. 지금은 빈 결과로 성공한다. 새 거절의 코드는 `USAGE`이고 `field`가 `--after`다. ==**이것이 이 변경에서 사용자가 보는 새 거절이다.**==
- `retrieve`와 `dispatch begin`에서 `--role`이 `--regime`의 역할과 다르면 거절하는 검사는 값끼리의 관계라 검증자다.
- 표현 못 하는 id 문법, `NAME@VERSION`, `FORMAT:PATH`, `--digest`의 `<change>=<digest>` 형식도 같다.

### F5 정수 범위

- `integer` 인수는 ±(2^53-1) 안에 있어야 한다. 밖의 값은 거절하고 반올림하지 않는다. argv에서는 그 인수의 코드 또는 `USAGE`, `field`가 그 인수다.
- 대상은 64비트 정수로 파싱하던 `--max-artifact-mib`, `--interval-secs`, 쪽 나누기 `--limit`, `retrieve --limit`이다. 32비트와 16비트 정수는 해당 없다.
- 2^53-1을 넘을 수 있는 식별자는 `string`과 패턴으로 선언한다. canon에는 해당하는 것이 없다.

### F6 테스트와 소비자

- 선언한 모든 제약마다 그 하나만 어기는 호출이 선언된 코드로 거절되고, 만족하는 호출은 그 이유로 거절되지 않는다. 모든 도메인은 경계값 바로 안과 밖에서 실제 파서와 같은 답을 낸다. 모든 검증자의 `rejects`가 `setup` 없이 만든 환경에서 거절된다.
- 모든 `examples`가 제약과 도메인을 만족한다. 명세 파일과 생성 문서(웹사이트 CLI 표, 스킬의 명령 목록)가 새 멤버로 다시 생성된다.
- CHANGELOG는 새 거절 둘(`export` 기간 순서, 2^53-1 초과)과 선언된 관계 목록을 싣는다.

## Acceptance

- F1:
  - ==**`canon schema init --json`의 `constraints`에 `--check`와 충돌하는 옵션들, `--policy`와 `--no-policy`가 `atMostOne`으로 있다.**== `canon schema review scenario --json`에 `exactlyOne` `--upstream-ref`, `--original-spec-ref`가 있다.
  - 같은 관계가 한 명령에 두 번 적히지 않는다. 그 규칙을 어긴 선언은 테스트에서 실패한다.
  - 실패 경로: `--commit`만 주고 `--plan-digest`를 빼면 exit 2, `USAGE`, `field`가 `--plan-digest`다.
- F2:
  - `canon schema retrieve --json`에 `exactlyOne` `--regime`, `--domain`과 `requires` `--subject`→`--domain`이 있다. `canon schema finding close --json`에 `--disposition`이 `rejected`나 `deferred`일 때 `--reason`을 요구하는 규칙이 있고 코드가 `FINDING_REASON_REQUIRED`다.
  - 위 표의 모든 관계마다 위반 호출이 선언된 코드와 종료 값으로 거절되고 아무것도 쓰이지 않는다.
  - 실패 경로: 핸들러에만 있고 선언에 없는 관계를 하나 더하면 전수 조사 테스트가 실패한다.
- F3:
  - `canon schema finding close --json`의 `round`와 `seq`에 `domain: {minimum: 1}`이 있다. `finding close --round 0`은 지금처럼 `FINDING_INVALID_ARGUMENT`이고 `canon query --limit 0`은 `USAGE`다.
  - 모든 `pattern`이 제한된 형식이고 Rust `regex`로 컴파일된다.
  - 실패 경로: 기본값이 도메인을 어기는 선언은 테스트에서 실패한다.
- F4:
  - `canon schema export --json`의 `validators`에 기간 순서 검증자가 있고 `rejects`가 거절된다. `canon export --before 2026-01-02 --after 2026-02-01 --json`이 exit 2다. 달력에 없는 날짜는 시각 옵션 12개 모두에서 거절된다.
  - 모든 검증자의 `rejects`를 초기화하지 않은 빈 디렉터리에서 돌려도 검증자의 코드가 나온다. 실패 경로: `rejects`가 거절되지 않거나, 거절 전에 저장소를 읽어 선행 조건 코드를 내는 검증자는 테스트가 실패한다. 검증자가 선언 없이 거절하는 새 검사를 더해도 전수 조사 테스트가 실패한다.
- F5:
  - `--max-artifact-mib 9007199254740992`와 `-9007199254740992`는 exit 2다. `9007199254740991`은 받는다. `9007199254740993`이 `9007199254740992`로 반올림되어 받아들여지지 않는다.
- F6:
  - `cargo test --workspace`, conformance, 생성 문서 drift 테스트가 통과한다. 선언한 모든 제약, 도메인 경계, 검증자의 거절이 테스트로 확인된다.
  - 실패 경로: 제약 선언은 두고 핸들러 검사를 지우면 그 제약의 위반 테스트가 실패한다.

## Questions and assumptions

- 가정: 이 변경의 사용자에게 보이는 새 거절은 `export`의 기간 순서와 2^53-1 초과 정수 둘이다. 그 밖에는 코드와 종료 값이 지금과 같다. 지금 한도가 없는 `retrieve --limit`에 새 제한을 만들지 않는다.
- 계약 저자가 정했다. `finding add`에서 `--severity`가 `blocker`나 `should-fix`이면 `--introduced-rev`와 `--introduced-unknown` 중 정확히 하나가 필요하다. `note`는 둘 다 선택이고, 둘을 함께 주는 것은 항상 거절된다. 표현은 `atMostOne`과 검증자다. 값에 따른 조건부 `exactlyOne`은 계약 1.2 후보다.
- 가정: 검증자의 `code`는 계약이 등록된 코드와 exit 2를 요구하고 `USAGE`를 허용한다. 지금 `*_INVALID_ARGUMENT`류가 exit 2인지 구현자가 확인한다. exit 1인 코드가 나오면 보고한다.
- 가정: id 문법을 제한된 `pattern`으로 쓸 수 없으면 도메인에는 길이 제한만 싣고 문법 검사는 id 종류별 검증자로 둔다.
- 가정: 이 변경의 규칙 목록은 clap 소스와 핸들러의 전수 조사로 확정한다. 이 문서의 표는 조사한 결과이고, 조사에서 빠진 것이 나오면 같은 규칙으로 추가한다. 새 정책 결정은 아니다.
- 정책 질문은 없다. gate가 요구하는 것과 승인자는 바뀌지 않는다.

## Out of scope

- `schema` 트리, 그룹과 명령 문서의 틀, 값 타입, 기본값(`canon-0-34-atip-schema-doc`).
- 쪽 나누기와 `--fields`(`canon-0-34-atip-paging-fields`).
- 검사기의 제약, 도메인, 검증자, 숫자 검사(`canon-0-34-atip-check-l1`).
- 지금 제한이 없는 인수에 새 제한을 만드는 일, 상태를 읽는 검사의 선언.
- `--input` 문서의 숫자 처리와 입력 오류의 포인터(0.35).

Approved: Journey Hwang 2026-11-01T06:36:14Z — "승인 (6개 모두 0.34, 깨는 변경 괜찮아)" (relayed by orchestrator orchestrator-main via ask) (brief sha256:e57141e2340a81e9739d02464c36788661ac9db8450e4a876b67b19fdf87649a)
