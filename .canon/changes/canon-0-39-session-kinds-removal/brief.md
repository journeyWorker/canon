---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "기록 종류 둘의 퇴장이 모델, 저장소, 설정 키, 정리 명령, 조회 거절, 마트, 보고서, 대시보드에 걸쳐 있어 면마다 남길 것과 지울 것을 적어야 한다"
  - rule: "prose.length"
    section: "detail"
    reason: "면마다 호환 처리(허용하는 옛 키, 한 릴리스 남기는 purge, 거절 문구, 남는 열)를 정해야 해서 줄이면 조건이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "흐름 여덟의 실패 경로가 각각 한 줄씩이다"
  - rule: "prose.length"
    section: "questions"
    reason: "소유자 결정 여섯과 이 브리프가 정한 호환 경계 다섯을 각각 적어야 한다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 파일 이름, 열 이름, 명령 철자이고 호출자가 읽는 철자 자체다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "괄호는 남기는 이름과 코드의 철자다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "키, 열, 마트, 경고 코드, 경로는 지우거나 남기는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "확인할 명령, 코드, 열, 경로는 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "남기는 타입과 키는 결정의 대상 자체다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "사용자 저장소의 canon.yaml에 이미 있는 키 이름이 문제의 대상이라 철자 그대로 적어야 한다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "남는 타입과 허용하는 설정 키는 호출자가 읽는 철자 자체다"
---

# canon은 session과 event 기록 종류를 더 두지 않는다

Source: 소유자 결정(2026-11-07, 세션 수집을 빼고 워크플로에 집중), 0.38.0 소스 트리의 제거 지도(`RecordKind` 소비처와 마트·보고서·대시보드), 0.30 `evidence_record` 이름 바꿈(`canon.yaml` 키 허용과 `canon migrate` 재작성의 선례), 앞선 변경 canon-0-39-session-ingest-removal

## Intent

앞선 변경이 `canon ingest sessions`를 걷어내면 `session`과 `event` 기록을 새로 만드는 길이 없다. 그런데 두 종류는 모델의 닫힌 열거(14종), 스키마와 고정 파일, 저장소 라우팅 키, `purge`의 허용 목록, 보고서의 마트와 패널, 대시보드 패널에 그대로 남는다. `canon.yaml`의 `routing.session`, `routing.event`, `aging.event`는 `canon init`이 만든 저장소마다 들어 있다. 이 종류를 열거에서 빼면 알 수 없는 종류 이름은 설정 오류이므로 ==**기존 저장소의 `canon.yaml` 전부가 로드되지 않게 된다.**== 이 변경은 종류를 걷어내면서 그 길을 막는다.

## Desired outcome

- 기록 종류는 12종이다. `session`과 `event`의 구조체, 스키마, 고정 파일이 없다. `Run`과 `SessionId`, 행위자와 Run의 `session_id`는 남는다.
- ==**옛 `canon.yaml`의 `session`·`event` 라우팅·에이징 키는 경고와 함께 읽히고, `canon migrate`가 그 줄을 지운다.**==
- 저장된 session과 event 행은 한 릴리스 동안 `canon purge --kind session|event`로 지울 수 있다.
- `session`·`event` 종류를 말하던 마트, 보고서 패널, 대시보드 패널이 없고 Run 관측 마트는 Run 열만 남는다.

## User flow

1. F1 사용자는 옛 라우팅 키가 든 저장소에서 명령을 돌린다 → 오류 없이 돈다 → ==**경고가 그 키와 `canon migrate`를 말한다.**==
2. F2 사용자는 `canon migrate`를 돌린다 → 미리보기가 지울 줄을 말한다 → `--commit --plan-digest`로 적용하면 경고가 사라진다.
3. F3 사용자는 `canon purge --kind session --before <날짜>`를 돌린다 → 미리보기와 확정이 다른 종류와 같다 → 저장된 행이 지워진다.
4. F4 에이전트는 `canon query --kind session`을 돌린다 → 거절된다 → 이유와 지우는 길을 안내받는다.
5. F5 에이전트는 `canon schema`, `canon context`, 스키마 파일을 읽는다 → 12종만 있다.
6. F6 사람은 보고서와 대시보드를 연다 → 세션 비용 패널이 없고 Run 관측은 Run 열만 있다.
7. F7 git 계층에 `kind=session/`이나 `kind=event/` 디렉터리가 있는 저장소의 사용자는 명령을 돌린다 → 오류가 없다 → 그 디렉터리는 읽히지 않는다.
8. F8 사용자는 새 저장소에 `canon init`을 돌린다 → hot에는 task, handoff, run이 라우팅된다 → `init --check`가 통과한다.
9. F9 사람은 CHANGELOG와 안내문을 읽는다 → 없어진 것과 옮길 길이 적혀 있다 → 저장된 행은 이 릴리스에서 지우라는 말이 있다.

## Step detail

### F1 옛 설정 키 허용

- 허용하는 키는 `routing:`의 `session`, `event`와 `aging:`의 `session`, `event`다. 값은 읽지도 검증하지도 않는다(옛 값이 알 수 없는 계층 이름이어도 오류가 아니다). 다른 알 수 없는 종류 이름은 지금처럼 설정 오류이고 그 메시지의 종류 목록은 12종이다.
- 경고 `RETIRED_KIND_KEYS`가 발견한 키를 `routing.session`처럼 모두 말하고 `canon migrate --json`을 `meta.next` 첫 항목으로 낸다. 0.29 배치를 알리는 `OLD_LAYOUT_FOUND`와 같은 읽기 명령(`status`, `gate check`, `context`)과 `init --check`가 낸다.
- ==**쓰기 명령은 거절하지 않는다.**== 0.29 배치는 쓰기를 `LAYOUT_MIGRATION_REQUIRED`로 거절하지만, 이 키가 든 저장소가 `change new`나 `evidence add`를 못 하게 막을 이유가 없다. 이 키만 있는 저장소는 0.29 저장소로 세지 않는다.
- 이 키는 아무것도 라우팅하지 않는다. `tier age`는 session과 event 행을 늙히지 않는다.

### F2 migrate

- `canon migrate`의 미리보기는 `canon.yaml`에서 지울 줄을 줄 번호로 말한다. 적용은 그 줄만 지운다. 0.30의 `evidence_record` 재작성과 같은 길이다(`rewrite_config`, 안전하지 않은 형태 거절, 결과 문서 동등 검사, 백업과 `--revert`).
- 결과는 옛 문서에서 그 키만 뺀 것과 같아야 한다. 앵커나 흐름 매핑처럼 줄로 지울 수 없는 형태는 거절하고 손으로 지우라고 안내한다. 키를 지운 절이 비면 빈 매핑으로 둔다.
- 키가 없는 저장소의 `migrate`는 이 단계를 건너뛴다. 이미 적용했으면 다음 실행의 계획이 비어 있다. 저장된 행과 파일은 `migrate`가 건드리지 않는다.
- 0.29 배치의 이름 바꾸기는 `kind=session/`과 `kind=event/` 아래 옛 이름 파일을 옮기지도 거절하지도 않고 건너뛴다. 그 종류가 알 수 없는 이름이 되면 겹침 파일 규칙(`__<12자>`)을 요구받아 거절하기 때문이다. hot 행의 옛 키 바꾸기도 두 종류를 건너뛴다.
- 이 저장소의 `canon.yaml`도 같은 길로 고친다. `routing.session`, `routing.event`, `aging.event` 줄이 사라진다. `canon init`의 뼈대는 이 줄을 쓰지 않는다.

### F3 purge 한 릴리스 유지

- `purge --kind session`과 `--kind event`는 미리보기, 계획 다이제스트, 확정, 감사 줄, 부분 삭제 보고가 지금과 같다. `--kind`의 닫힌 값은 `run`, `trajectory`, `session`, `event`이고 도움말은 뒤의 둘이 퇴역했으며 다음 릴리스에 빠진다고 말한다.
- 라우팅 키가 `migrate`로 지워진 뒤에도 지울 수 있어야 한다. 그래서 퇴역 종류의 purge는 라우팅을 읽지 않고 `tiers:`에 설정된 모든 계층을 찾는다. hot의 sqlite와 Postgres, R2 객체, git 계층의 `kind=session/` 디렉터리가 대상이다. 퇴역한 두 이름은 purge만 아는 닫힌 목록이다. `RecordKind`, 스키마, 조회, 내보내기, 검색에는 돌아오지 않는다.
- 다른 종류와 `--before`, `--commit --plan-digest`의 규칙은 그대로다. 허용 밖 종류의 오류 문구는 허용 목록을 퇴역 표시와 함께 말한다. git 계층의 삭제는 작업 트리 파일을 지우며 git 기록에는 남는다.
- 이 구간은 한 릴리스다. 그 뒤에는 `--kind session|event`가 닫힌 값 거절이 된다. 그때 행이 남은 저장소는 지울 길이 직접 DB뿐이므로 CHANGELOG가 이 릴리스에서 지우라고 말한다.

### F4 조회 거절

- `query`, `export`, `search`의 `--kind session|event`는 종료 값 2의 `USAGE`이고 `field`는 `--kind`, `expected`는 12종이다. 메시지는 두 종류가 0.39에서 없어졌다고 말하고 힌트는 이미 저장된 행이 남아 있으며 `canon purge --kind session`으로 지운다고 안내한다. 별칭은 없다.
- 설정의 라우팅 키와 달리 `--kind`의 값은 읽지 못한다. 저장된 행을 보여주는 대체 조회는 없다. `mart_records_by_kind`는 저장된 대로 종류별로 세므로 남은 행이 거기 보이고, 지우면 사라진다.

### F5 모델과 스키마

- 열거 12종, `ALL`, 이름 요소 표, 이름 틀, 고정 파일, 스키마 내보내기, 스키마 레지스트리 시험이 12종을 말한다. `session.schema.json`, `event.schema.json`, `well-formed/session.json`, `well-formed/event.json`이 사라진다. `canon schema`, `canon context`의 종류 목록, 명령 문서의 `--kind` 열거, CEL 종류 값 집합, `JOIN_SPINE.md`가 같은 12종을 말한다.
- `JOIN_SPINE.md`의 `session_id` 행은 남고 설명에서 "어댑터마다 다른 유래"와 "세션 ↔ 비용 ↔ 실행"이 빠진다. `run_id ↔ events`와 `handoff ↔ session`도 고친다.
- `canon search`의 그래프는 session 노드와 event 노드를 만들지 않는다. `session_id`와 `actor.session_id`는 속성으로 남고 그 간선은 생기지 않는다. 다른 종류의 간선은 그대로다.
- 열거를 소비하는 곳은 두 종류를 빼고 같은 답을 낸다. 보고서 다이제스트의 분류, 내보내기의 가림 처리, 쓰기 잠금, 정리와 보존 시험이 그렇다. hot 저장소의 읽기 점검은 `session` 대신 `task`로 읽는다.

### F6 마트, 보고서, 대시보드

- 뷰 `mart_session_costs`와 `mart_session_run_handoff`가 사라진다. `mart_run_observability`는 `sessions`와 `events` 조인을 잃고 열 `event_count`, `token_event_count`, `directive_event_count`, `cost_source_count`, `total_cost`, `total_tokens`, `first_event_at`, `last_event_at`, `event_times_unknown`, `cost_unknown`이 빠진다. `run_id`, `session_id`, `project_key`, `task_id`, `parent_run_id`, `status`, `started_at`, `ended_at`, `latency_ms`, `child_run_count`, `guidance_count`, `provider`, `model`과 `_unknown` 표지(`session_id`, `project_key`, `task_id`, `parent_run_id`, `ended_at`, `provider`, `model`, `guidance`)는 남는다.
- `project_key`는 Run 자신의 값만 읽는다. 세션 기록이 채워 주던 값이 없어지므로 그 값이 없던 옛 Run은 `project_key_unknown`이 된다.
- 보고서 마트는 10개에서 9개로, 스냅샷 파일은 `mart_session_costs.parquet`을 잃고 매니페스트가 같은 집합을 말한다. 보고서에서 `## Session costs` 절과 그 안내문이 사라지고 `## Run observability`의 안내문과 표가 남은 열을 말한다. 0.38이 만든 `REPORT.md`에는 `canon report check`가 종료 값 1을 내고 `canon report build`가 새로 쓴다.
- 대시보드에서 세션 비용 패널(`session-costs.ts`, `index.html`의 절, `main.ts`의 등록, 고정 스냅샷과 그 생성 스크립트, 패널 문구 시험)이 사라진다. 스냅샷에 그 표가 없어도 대시보드는 불러온다.
- 비용과 토큰 보고는 이 변경으로 없다. 대체하는 데이터원은 없다.

### F7 git 계층의 옛 디렉터리

- `kind=session/`과 `kind=event/`는 git 계층이 읽을 때 알 수 없는 이름 공간으로 센다. 읽지도 오류로 세지도 않고 파일은 그대로 있다. `status`, `gate check`, `ledger compact`, `inventory sync`는 그것 때문에 실패하지 않는다.
- 보고서의 원천 다이제스트는 핵심 종류가 아닌 `kind=` 디렉터리를 겹침 입력으로 해시한다. 그 디렉터리를 가진 저장소는 이 변경 뒤 다이제스트가 한 번 달라진다. 그 밖의 저장소는 같다.
- Postgres와 R2의 옛 행, hot sqlite의 옛 행은 읽는 곳이 없다. 지우는 길은 F3뿐이다.

### F8 init

- `canon init`이 쓰는 라우팅은 12종이고 hot에는 `task`, `handoff`, `run`이 간다. 뼈대의 주석("task/handoff/session/run/event")과 `init` 시험이 이를 따른다. 이미 있는 저장소는 `init`이 건드리지 않는다.

### F9 안내

- CHANGELOG의 Breaking에 표가 오른다. 열거 14→12, 설정 키 허용과 `migrate`, 한 릴리스 남는 `purge`, `--kind` 거절을 싣는다. 사라진 마트와 패널, 줄어든 Run 관측 마트, 다이제스트 변화도 싣고 저장된 행은 이 릴리스에서 `purge`로 지우라고 적는다.
- 안내문(`canon-storage`, `canon-report-dashboard`)과 `ARCHITECTURE.md`의 계층 문단, 웹사이트 계층 개념 페이지(영어, 한국어)의 종류 나열이 12종을 말한다. 역할 안내문의 명령 용례는 재생성된다.
- 종류를 말하던 시나리오를 고친다. 규칙은 그대로이고 표본과 목록만 바뀐다.
  - `tier.storage.16`은 `purge --kind run`으로, `tier.storage.17`은 허용 목록을 `run`, `trajectory`와 퇴역한 `session`, `event`로 말한다.
  - `atip.planjcs.09`의 "세션 기록"은 Run 기록이 된다.
  - `ledger.naming.01`의 "열다섯 종류, 핵심 열네 종류"는 "열세, 열둘"이 된다. `ledger.naming.03`의 "이벤트 9와 10"은 "run 순번이 같은 divergence의 round 9와 10"이 된다.
  - `context.capability.39`의 "query, export, purge의 닫힌 `--kind` 값은 모두 그 종류"는 purge가 퇴역 이름 둘을 더 받는다는 말을 담는다.
- 새 시나리오는 이 브리프의 흐름마다 하나 이상 쓴다. 허용 키, `migrate` 재작성, 퇴역 종류의 purge, 조회 거절, 12종, 마트, 옛 git 디렉터리, `init`이다.

## Acceptance

- F1:
  - `routing.session: hot`, `routing.event: hot`, `aging.event`가 든 `canon.yaml`로 `status`, `gate check`, `context`가 오류 없이 돌고 경고 `RETIRED_KIND_KEYS`가 키 셋을 말하며 `meta.next` 첫 항목이 `canon migrate --json`이다.
  - 실패 경로: `routing.sesion` 같은 다른 알 수 없는 종류는 지금처럼 설정 오류이고, 옛 키가 든 저장소에서 `evidence add`와 `change new`는 거절되지 않는다.
- F2:
  - 미리보기가 지울 줄을 말하고 `--commit --plan-digest`가 그 줄만 지운다. 결과는 옛 문서에서 그 키를 뺀 것과 같고 경고가 사라지며 다음 `migrate`의 계획이 비어 있고 `--revert`가 원래 파일을 되살린다. 이 저장소의 `canon.yaml`에 세 줄이 없다.
  - 실패 경로: 앵커나 흐름 매핑 형태의 키는 거절하고 파일을 그대로 두며 손으로 지우라고 안내한다. 0.29 배치의 옛 이름 `kind=session/` 파일이 든 저장소의 `migrate`는 그것 때문에 거절하지 않는다.
- F3:
  - 세션 행 둘(기준일 앞과 뒤)을 가진 저장소에서 `purge --kind session`의 미리보기는 하나를 지울 것으로 말하고 쓰지 않는다. 다이제스트로 확정하면 앞의 것만 사라지고, 라우팅 키를 `migrate`로 지운 저장소에서도 같다.
  - 실패 경로: 낡은 다이제스트는 `PLAN_STALE`이고 `--kind change`는 닫힌 값 거절이며 행은 그대로다.
- F4:
  - `query --kind session`, `export --kind event`, `search --kind session`이 종료 값 2의 `USAGE`이고 `expected`가 12종이며 힌트가 `purge`를 말한다.
  - 실패 경로: `--kind run`과 `--kind evidence`는 0.38과 같은 답을 낸다.
- F5:
  - `canon schema`와 `canon context`의 종류, 스키마 파일 목록, 명령 문서가 12종이고 생성 문서 drift 시험이 통과한다. `search`의 그래프는 session 노드 없이 다른 간선을 0.38과 같게 낸다.
  - 실패 경로: `session.schema.json`이 남아 있으면 스키마 레지스트리 시험이 실패한다.
- F6:
  - 새 보고서에 `## Session costs`가 없고 `## Run observability`가 남은 열만 말하며 스냅샷이 9개 표와 일치하는 매니페스트를 갖는다. `bun test`와 대시보드의 스냅샷 시험이 통과한다.
  - 실패 경로: 0.38이 만든 `REPORT.md`에 `canon report check`는 종료 값 1이고 `canon report build` 뒤 0이다.
- F7:
  - `kind=session/` 파일이 든 git 계층에서 `status`, `gate check`, `ledger compact`가 성공하고 그 파일은 그대로다.
  - 실패 경로: 읽을 수 없는 본문의 파일도 새 오류를 만들지 않는다.
- F8:
  - 새 `canon init`의 `canon.yaml`에 `session`·`event` 줄이 없고 hot 라우팅이 `task`, `handoff`, `run`이다. `init --check`가 통과한다.
  - 실패 경로: 옛 뼈대로 만든 저장소에서 `init --check`는 F1의 경고를 낸다.
- F9:
  - CHANGELOG에 표가 있고 안내문, `ARCHITECTURE.md`, 웹사이트에 `session`·`event` 종류 나열이 없다. 고친 시나리오가 `inventory sync`를 통과한다.
  - 실패 경로: 항목이 없으면 릴리스 확인 시험이 실패한다.
- ==**변경 뒤 `cargo build --workspace`가 경고 없이 통과하고, `cargo nextest run --workspace`, 생성 문서 drift 시험, `conformance`, `just atip-deep`, 대시보드 `bun test`, `canon skills check`, `canon gate check`가 통과한다.**==

## Questions and assumptions

- 결정(소유자): 세션 수집만 뺀다. `ingest plans`, `ingest records`, `canon dispatch`가 쓰는 Run, `SessionId`와 `Run.session_id`, 행위자의 `session_id`는 남는다. 리뷰어 독립성 검사가 읽는다.
- 결정(소유자): 옛 `canon.yaml`의 `session`·`event` 키(에이징 키 포함)는 로더가 경고와 함께 허용하고 `canon migrate`가 지운다. 0.30 `evidence_record` 이름 바꿈이 선례이고 이 한 군데만 "별칭 없음"의 예외다.
- 결정(소유자): `purge --kind session|event`는 한 릴리스 남겨 저장된 행을 지울 수 있게 한다. 그 뒤에는 닫힌 값 거절이다.
- 결정(소유자): 세션 데이터 위에 세운 비용·관측 뷰, 보고서 패널, 대시보드 패널은 없앤다.
- 결정: 이 변경은 앞선 변경(canon-0-39-session-ingest-removal) 뒤에 선다. 같은 릴리스에 둘 다 싣는다. 이 변경만 먼저 들어가면 `ingest sessions`가 존재하지 않는 종류를 쓰게 된다.
- 결정: 옛 키가 든 저장소는 경고만 받고 쓰기는 막지 않는다. 0.29 배치의 쓰기 거절은 옛 이름 파일과 새 이름 파일이 섞이는 위험 때문이고 이 키에는 그런 위험이 없다.
- 결정: 퇴역 종류의 purge는 설정된 모든 계층을 찾고 라우팅을 읽지 않는다. `migrate`가 라우팅 키를 지운 뒤에도 지울 수 있어야 하기 때문이다.
- 결정: `mart_run_observability`는 없애지 않고 Run 열만 남긴다. dispatch가 쓰는 Run의 계보, 보증 주입, 상위 Run은 읽을 곳이 필요하다.
- 결정: `mart_records_by_kind`는 손대지 않는다. 저장된 대로 센다.
- 결정: 옛 git 디렉터리를 위한 별도 처리는 없다. 알 수 없는 이름 공간 규칙이 이미 있고 다이제스트가 한 번 달라지는 것을 받아들인다.
- 실패 방식: 이 키를 지우지 않은 채 다음 릴리스(퇴역 목록이 없어지는 때)로 올라가면 로더의 허용이 끝날 수 있다. 허용을 거두는 일은 퇴역 목록과 함께 다음 변경이 정한다.

## Out of scope

- 저장된 session과 event 행을 `migrate`가 지우는 일(`purge`가 지운다)과 Postgres, R2의 옛 객체 정리 도구.
- `purge`의 퇴역 목록과 옛 키 허용을 거두는 일(다음 릴리스).
- 세션 수집을 대신하는 비용·토큰 보고, 외부 도구로의 안내.
- `canon-ingest` 크레이트 정리(안 쓰는 의존성, 이름 바꾸기)와 `ingest` 그룹 이름 바꾸기.
- `Run`을 지우거나 `Run.session_id`, 행위자의 `session_id`를 바꾸는 일과 `canon dispatch`의 동작 변경.
- `docs/research/**`와 옛 설계 문서의 서술(역사 문서).

Approved: Journey Hwang 2026-11-08T06:11:30Z — "승인 (canon 0.39 브리프 7개와 정책)" (relayed by planner planner-oco via ask) (code tree 7c28ab134d4701f16f343da5e87c7582b9521417) (brief sha256:e2b67535f72b19511a22541b70dcf059de95f88fac74389a3c88b30cad984618)
