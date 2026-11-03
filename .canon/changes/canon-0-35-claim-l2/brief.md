---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "선언, fixture, CI, 소비자 이동, 업그레이드 노트를 한 변경에서 정해야 해서 줄이면 결정이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "다섯 단계가 각자 선언과 fixture와 CI 규칙을 가져야 해서 줄이면 사실이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "다섯 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "meta 키와 검사 id는 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "이름과 철자는 사용자가 읽는 철자 자체이고 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "이름과 철자는 검증 대상의 철자 자체다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
---

# canon이 atip/1 1.2와 L2를 선언하고 CI가 L2를 통과시킨다

Source: https://github.com/journeyWorker/atip tag v1.2.0, spec 15.7, 17, approaches/migration.md §3.1 7번과 §7, approaches/checker.md §5와 §8, issue 134

## Intent

0.35의 나머지 변경이 canon을 1.2의 L2 규칙에 맞춘다. 이 마지막 변경이 canon이 그것을 선언하고 증명한다. canon의 출력은 지금 `meta.contractVersion: "1.1"`이고 `schema`의 `level`은 `L1`이다. CI는 L0와 L1을 1.1 주장으로 돈다. 계약 17은 레벨 주장에 검사기의 MUST 통과와 도구 자신의 시험 증거를 함께 요구한다. 이 변경이 선언을 올리고, fixture에 L2 항목을 쓰고, CI에 L2 잡을 더하고, 업그레이드 노트를 모아 0.35.0의 출고 준비를 끝낸다.

## Desired outcome

- 모든 canon 봉투와 `schema` 답이 `meta.contractVersion: "1.2"`를 선언하고 트리의 `level`이 `L2`이다.
- canon의 fixture가 1.2 L2 주장으로 `atip check`를 돌린다. 적용되는 모든 MUST 행이 통과한다.
- ==**CI가 canon에 대해 L0, L1, L2를 1.2 주장으로 돌리고 통과해야 초록이다.**==
- 계약 17.2가 요구하는 도구 자신의 시험 증거가 목록으로 변경 기록에 있다.
- CHANGELOG의 0.35.0 항목이 이 릴리스의 모든 호출자 표면 변경을 한 곳에 싣는다.

## User flow

1. F1 에이전트는 canon의 아무 봉투나 읽는다 → `contractVersion`이 `"1.2"`이다 → 1.2 규칙이 적용됨을 안다.
2. F2 에이전트는 `canon schema --json`을 읽는다 → `level`이 `L2`이다 → 입력, 안전, 안내의 L2 규칙이 지켜짐을 안다.
3. F3 개발자는 CI를 돌린다 → L0, L1, L2 잡이 1.2 주장으로 canon에 대해 통과한다 → ==**선언이 증명된다.**==
4. F4 개발자는 canon fixture를 읽는다 → `prepare`, `notRun`, `preview`, 배치 시험 항목이 있다 → 검사기가 필요한 호출을 안다.
5. F5 사용자는 CHANGELOG를 읽는다 → 이 릴리스의 모든 깨지는 변경과 업그레이드 노트가 한 곳에 있다 → 자기 스크립트를 고친다.

## Step detail

### F1 선언

- ==**canon의 선언 상수가 `"1.2"`가 된다.**== `canon-cli`와 `atip-check`가 각자 선언을 읽는다. 선언은 각자의 상수이고 crate의 최신 판 상수와 무관하다(`canon-0-35-crate-1-2`).
- `schema` 답의 `meta.schemaVersion`이 `"1.2"`이다. 다른 출력의 `schemaVersion`은 그 출력의 판이다.
- 트리의 `level`이 `L2`이다. 이 값은 L2 검사 통과가 확인된 뒤에만 켠다. 선언 상수와 `level`을 한 변경에서 함께 바꾼다.
- 선언을 읽는 소비자(스킬, 웹사이트, 대시보드, 런처, 평가 스크립트, conformance 케이스, 시나리오 문장)가 새 값을 쓴다. 하나라도 `"1.1"`을 하드코드하면 그 소비자의 시험이 실패한다.

### F2 L2의 적용 범위

- canon에 적용되지 않는 L2 행은 도구의 문서에서 정해진 `unsupported`이다. `L2.stream`(스트림 명령 없음), `L2.async`와 `L2.operation-key`(비동기와 `--operation` 없음), `L2.search`(`schema --search` 없음)가 해당한다.
- 적용되는 L2 MUST는 입력 15개와 안전 10개 중 해당하는 것과 안내 3개(`skills`, `guide-commands`, `help-guide`)와 `effects`이다. 모두 통과해야 한다.
- `L2.search`는 SHOULD이고 canon에서 `unsupported`이다. 이 변경은 `schema --search`를 더하지 않는다.

### F3 CI

- `just atip`이 세 단계로 돈다. L0, L1, L2를 모두 `--contract 1.2`와 canon fixture로 canon 바이너리에 돈다. 검사기 자신은 L0에서 `--contract 1.2`로 돈다.
- GitHub 워크플로의 `atip-l0`, `atip-l1` 잡에 `atip-l2` 잡을 더한다. 잡의 이름과 구성은 기존 패턴을 따른다. 집계 레시피 `just ci`에 들어간다.
- 잡이 실패하면 어떤 행이 왜 실패했는지 `--json`의 `findings[]`가 보인다(`canon-0-35-check-findings`).
- 이 잡은 `canon-0-35-check-1-2`, `canon-0-35-check-l2-input`, `canon-0-35-check-l2-safety`, `canon-0-35-check-l2-guides`와 canon 쪽 변경이 모두 병합된 뒤에만 초록이 된다.

### F4 canon fixture

- `crates/canon-cli/tests/atip/canon.fixture.json`이 `contractVersion: "1.2"`, `prepare`(git 저장소와 빈 커밋 하나), `setup`, `env`, `probe`, `globalOptions`, `registry: "from-schema"`, `commands`를 가진다.
- `commands`는 모든 `read` 명령과 쓰기 명령 일부에 `ok`, `refused`, `missingValue`, `fields`, `paged`, `domainRejects`를 싣는다. 새로 `preview`(여섯 파괴적 명령), `batchFailsAt`, `batchFailsFirst`, `allowedEffects`(색인과 감사 줄 경로), `notRun: ["dashboard"]`를 싣는다.
- fixture 생성 규칙은 그대로이다. `from-schema`이므로 레지스트리를 손으로 쓰지 않는다.
- fixture에 올리지 않은 쓰기 명령은 검사기가 돌리지 않고 이름을 보고에 싣는다. canon 자신의 시험(`constraints_hold`)이 그 명령의 제약을 증명한다.

### F5 증거와 업그레이드 노트

- 계약 17.2가 요구하는 도구의 시험 증거를 목록으로 싣는다. 항목마다 canon의 시험 이름이 있다.
  - 트리 완전성(`cli_spec`의 파서와 `schema` 일치 시험).
  - 종료 값을 덮어쓰는 호출이 없고 낸 코드가 모두 등록됨(`schema_codes`).
  - 쓰기 명령의 제약이 파서와 맞음(`constraints_hold`).
  - 미리보기의 효과가 확정이 하는 일과 같음.
  - 비밀이 오류, `next`, 보고서에 나오지 않음.
  - 안내와 예시가 실제 파서로 파싱됨.
  - `next`가 이식 가능한 문법을 쓰고 값이 `-`로 시작하면 `--name=value`로 씀.
  - 경고 코드가 등록됨. 단계 중단이 `STOPPED_MIDWAY`로 나옴(옮긴 명령이 있을 때).
- CHANGELOG의 0.35.0 항목이 이 릴리스의 모든 깨지는 변경을 싣는다. 한 곳이므로 호출자가 읽기 쉽다.
  - cursor 형식과 묶기 규칙(0.34 cursor 거절), `limit`, `cursor`, `all`의 flag-only.
  - 계획 digest 형식(`sha256:`, JCS, 입력 포함)과 이동 전 미리보기의 `PLAN_STALE`.
  - `--input`의 열거형 위반 코드, NDJSON의 `#` 줄, argv의 `+1`.
  - 배치의 포인터와 사전 `--dry-run`의 제거.
  - `skills show`의 JSON 모양과 선택 `NAME`, 도움말 첫 줄.
  - 옮겨진 `STOPPED_MIDWAY` 코드의 대체표, 읽기와 거절의 쓰기.
  - 선언 `1.2`와 `level` `L2`. 검사기의 1.2 지원과 L2.
- 버전 올리기와 태그는 릴리스 담당의 일이다. 이 변경은 `just release-check`가 통과하는 상태에서 끝난다.

## Acceptance

- F1:
  - `canon status --json`과 `canon nosuch --json`의 `meta.contractVersion`이 `"1.2"`이다. `canon schema --json`의 `data.level`이 `L2`이고 `meta.schemaVersion`이 `"1.2"`이다.
  - 모든 봉투가 계약의 봉투 스키마를 통과한다. `contractVersion: "1.1"`을 하드코드한 소비자가 없음을 시험이 확인한다.
  - 실패 경로: `level`을 `L2`로 선언했는데 L2 검사가 실패하면 CI가 빨갛다. 선언이 증명 없이 나가지 않는다.
- F2:
  - `atip check --level L2 --contract 1.2`가 canon에서 `data.checks`에 적용되는 모든 MUST 행을 통과로 싣는다. `L2.stream`, `L2.async`, `L2.operation-key`, `L2.search`는 `unsupported`이다.
  - 실패 경로: 적용되는 행 하나가 `unprovided`이면 레벨이 통과하지 못하고 종료 코드는 1이다.
- F3:
  - `just atip`이 L0, L1, L2 세 단계를 canon에서 돌려 통과하고 검사기 자신의 L0도 통과한다. CI의 `atip-l2` 잡이 같은 명령을 돈다.
  - ==**canon의 출력 하나를 계약에서 어긋나게 바꾸면 CI 잡이 해당 행 실패로 빨개진다.**==
  - 실패 경로: 한 변경이 빠진 채 L2를 돌리면 잡이 그 변경의 행에서 실패한다. 예를 들어 `skills show`가 이름 필수이면 `L2.skills`가 실패한다.
- F4:
  - fixture에 `contractVersion: "1.2"`와 `prepare`, `notRun`, `preview`, `batchFailsAt`, `batchFailsFirst`, `allowedEffects`가 있다. 이 키가 없으면 해당 행이 `unprovided`로 나온다.
  - 실패 경로: `notRun`에 없는 `dashboard` 호출을 fixture에 넣으면 검사기가 멈춘다. `notRun`이 있으면 검사기가 `dashboard`를 실행하지 않는다.
- F5:
  - 증거 목록의 모든 시험이 존재하고 통과한다. 목록의 항목 하나를 지우면 변경 기록의 증거 확인이 실패한다.
  - CHANGELOG의 0.35.0 항목이 위 모든 변경을 싣는다. `just release-check`와 `cargo test --workspace`, 생성 문서 drift 시험이 통과한다.

## Questions and assumptions

- 가정: canon의 `level: "L2"` 선언은 검사기의 L2 행(`canon-0-35-check-l2-*`)과 canon 쪽 변경이 모두 병합된 뒤에만 켠다. 이 변경이 마지막 변경이다.
- 가정: 0.35.0-rc.1은 wave A와 B가 끝난 뒤 이 변경 전에 나갈 수 있다(`canon-0-35-release-rc`). 그 시점에 canon은 1.1과 L1을 선언한다. 0.35.0 정식은 이 변경 뒤이다.
- 가정: 비동기, `--operation`, 스트림, `schema --search`는 canon이 제공하지 않아 L2 행이 `unsupported`이다. 계약상 L2 주장에 지장이 없다.
- 가정: 업그레이드 노트는 각 변경의 brief가 요구한 것을 이 변경이 한 곳에 모은다. 각 변경도 자기 노트를 CHANGELOG에 올릴 수 있다. 중복되지 않게 구현자가 정리한다.
- 정책 질문은 없다. 선언을 올려도 gate가 요구하는 것과 승인자는 바뀌지 않는다.

## Out of scope

- 버전 올리기, 태그, 배포. 릴리스 담당이 한다.
- L2 행을 구현하는 일(`canon-0-35-check-l2-*`)과 canon의 각 동작 변경(`canon-0-35-canon-*`, `canon-0-35-input-order`, `canon-0-35-batch-pointers`, `canon-0-35-plan-jcs`, `canon-0-35-skills-show`, `canon-0-35-read-writes`).
- `schema --search`, 비동기 명령, 스트림 명령을 canon에 더하는 일.
- 0.36 이후의 계약 판(1.3)과 서버 명령의 기계 판독 선언.

Approved: Journey Hwang 2026-11-02T06:53:19Z — "휴가 중 승인 위임(2026-11-01 대화); 0.35 즉시 시작은 사용자 결정(ATIP 세션 경유 2026-11-02)" (relayed by orchestrator orchestrator-main via chat) (brief sha256:ad09608c18a5619dc09e9cb129a2a220a1ea33099fb795468e199d77f1aa0e59)
