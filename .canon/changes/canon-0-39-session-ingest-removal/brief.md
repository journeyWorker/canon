---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "명령 하나를 걷어내며 남는 수집의 같음, 설정·스킬·시나리오·문서의 처분을 한 변경에서 정해야 하고 줄이면 처분이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "제거 대상이 코드, 설정, 스킬, 시나리오, 웹사이트, 생성물에 걸쳐 있어 항목마다 남길 것과 지울 것을 적어야 한다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "흐름 일곱의 실패 경로와 변경 뒤 전체 확인이 각각 한 줄씩이라 줄이면 항목이 빠진다"
  - rule: "prose.length"
    section: "questions"
    reason: "남기는 것(ingest plans, ingest records, Run)과 다음 변경으로 넘기는 것의 경계가 결정 줄마다 다르다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 파일 이름, 코드 이름, 명령 철자이고 호출자가 읽는 철자 자체다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "괄호는 남기는 명령과 코드 이름의 철자다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "명령, 파일 경로, 코드 이름, 시나리오 ID는 지우거나 옮기는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "확인할 명령, 코드, 파일 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "남기는 타입과 코드 이름은 결정의 대상 자체다"
---

# canon ingest sessions leaves canon: its command, adapters, config keys, skill and scenarios

Source: 소유자 결정(2026-11-07, 세션 수집은 canon에서 빼고 워크플로에 집중), 0.38.0 소스 트리의 제거 지도(`canon ingest sessions`의 모든 소비처, 경로와 줄 번호; 세션 전용 코드 8,479줄은 canon-ingest의 어댑터 3,910·정규화 1,208·레지스트리 270·자체 시험 101·어댑터 계약 418과 통합 시험 317, canon-cli의 수집기 2,062와 시험 193), 뒤따르는 변경 canon-0-39-session-kinds-removal

## Intent

canon은 의도에서 증거와 게이트까지 이어지는 워크플로를 다루는 도구다. `canon ingest sessions`는 그 밖의 일을 한다. 에이전트 CLI 넷의 대화 기록 파일을 읽어 세션·실행·이벤트 기록과 비용 표를 만든다. 이 일에 매인 코드가 8,479줄이고, 파서는 남의 제품이 정한 파일 형식을 따라가므로 그 제품이 바뀔 때마다 고쳐진다. 시나리오 16개, 안내문 한 편, 개인정보 페이지 두 편, 환경 변수 둘, 설정 키 셋도 여기에 매여 있다.

## Desired outcome

- `canon ingest sessions`가 없다. 어떤 철자로 불러도 `ingest`의 모르는 하위 명령으로 거절된다.
- 대화 기록 어댑터와 그 시험, 환경 변수 둘, 설정 키 둘, 스킬 한 편, 개인정보 페이지, 평가 과제 셋이 함께 사라진다. 시나리오 16개 중 15개가 사라지고 하나는 남는 명령의 시나리오로 옮겨진다.
- ==**`ingest plans`, `ingest records`, `canon dispatch`의 Run 기록, 계획 커서 파일 이름, 시도의 체제 키는 0.38과 같은 값을 낸다.**==
- 저장된 기록과 커서 파일은 건드리지 않는다.

## User flow

1. F1 에이전트는 `canon ingest sessions`를 돌린다(옵션이 있든 없든) → 종료 값 2로 거절된다 → ==**`ingest`가 아는 하위 명령 `plans`와 `records`를 안내받는다.**==
2. F2 에이전트는 `canon ingest records`와 `canon ingest plans`를 돌린다 → 0.38과 같은 보고서와 같은 기록이 나온다 → 커서와 시도 식별자가 그대로다.
3. F3 에이전트는 `canon dispatch begin`과 `end`를 돌린다 → Run 기록이 전과 같은 계층에 같은 모양으로 저장된다.
4. F4 사용자는 옛 `ingest:` 설정 절이 든 저장소에서 아무 명령이나 돌린다 → 오류 없이 돈다 → 그 절은 읽히지 않는다.
5. F5 사용자는 `canon skills install`과 `check`를 돌린다 → 번들에서 빠진 스킬 파일이 정리되거나 고아로 보고된다 → 안내는 새 번들만 말한다.
6. F6 게이트를 돌리는 사람은 `canon gate check`를 돌린다 → 사라진 시나리오 15개 때문에 위반이 생기지 않고, 옮겨진 하나는 새 증거를 기다린다 → 주제 `ingest-pipelines`의 나머지 시나리오는 그대로 판정된다.
7. F7 사람은 CHANGELOG와 웹사이트를 읽는다 → 명령과 키와 환경 변수가 없어졌다는 말과 옮길 곳(없다는 사실)이 있다.

## Step detail

### F1 명령이 없다

- 도움말, `canon schema`, 생성된 명령 문서(`docs/cli/canon.cli.json`), 쓰기 잠금 표, 보고서 스키마 표, 오류 코드 표, 예시 표, 읽기·쓰기 효과 표에서 `ingest sessions` 줄이 빠진다. 도움말의 `Examples:`도 `canon ingest records`와 `canon ingest plans`만 보인다.
- 거절은 모르는 명령어의 일반 경로를 탄다(코드 `UNKNOWN_COMMAND`, 종료 값 2). 되돌릴 이름이 없으므로 제거된 철자 표에 줄을 더하지 않는다. 그 표의 `--canon-yaml` 줄이 `ingest sessions`를 명령 목록에 갖고 있으므로 그 이름만 뺀다.
- `selftest --suite session-ingest`는 `--suite`의 닫힌 값에서 빠지고, 이름을 주면 닫힌 값 거절이다. 기본 실행(`--suite` 없음)은 그 묶음 없이 돈다.
- `INGEST_FAILED`와 `INGEST_SOURCE_INVALID`는 `ingest records`와 `ingest plans`가 아직 낸다. 코드 표에 남는다.

### F2 남는 수집의 같음

- `content_digest`는 정규 JSON의 SHA-256 앞 6바이트를 12자 16진으로 낸다. 정규화 모듈에 있고 계획 커서 파일 이름(`.canon/ingest/cursors/plan-<방언>-<12자>.json`)과 시도의 `regime_key` 해시가 쓴다. 그 모듈이 사라지므로 이 함수만 살아남는 모듈로 옮긴다.
- ==**옮긴 함수의 출력은 바이트 하나도 달라지면 안 된다.**== 이미 저장된 `regime_key`와 커서 파일이 그 값을 품고 있다. 같은 입력에 같은 12자를 내는 고정 벡터 시험이 옮긴 뒤의 함수를 고정한다. `canon-store`의 8자 `content_digest`는 다른 함수이고 건드리지 않는다.
- `persist_idempotent`(이미 저장된 기록의 중복 쓰기를 오류로 세지 않는 쓰기)는 수집기 파일에 있고 `canon dispatch`가 쓴다. 수집기가 사라지므로 dispatch가 이미 쓰는 계층 모듈로 옮긴다. 이 함수와 짝인 일괄 쓰기 도우미는 수집기만 썼으므로 함께 사라진다.
- 계획 커서 ID의 예약 접두사(`plan-`)와 세션 어댑터 ID 문법 검사는 세션 어댑터의 ID를 검사하던 것이다. 어댑터가 없으니 함께 사라진다. 계획 커서 파일 이름은 그대로다.
- `canon-ingest`는 계획 방언, `brief`, `markdown`, `task_rows`, 기록 어댑터(`ingest records`), `verdict`, `scanner`를 가진 채 남는다. 안 쓰게 된 의존성(대화 기록 파서를 위한 `rusqlite`, `simd-json`, `rayon`, `ulid` 중 이후 쓰이지 않는 것)은 이 변경에서 뺀다. 크레이트의 설명과 문서 주석은 남은 일을 말한다.
- 경로 상수 `INGEST_DIR`와 `INGEST_CURSORS_DIR`, 커서 저장소와 `.gitignore`의 `.canon/ingest` 줄은 계획 커서가 쓰므로 남는다. 커서 저장소 문서의 "세션 수집이 커서를 쓴다"는 말만 고친다.
- `ingest sessions`를 표본 명령으로 쓰던 시험은 `ingest records`로 옮긴다. 정수 옵션의 음수 거절, 쓰기 효과, 쓰기 불가 보고, 환경 변수 제거, 일괄 입력이 그렇다. `ingest records`에도 같은 `--interval-secs`와 `--watch`가 있고 시험이 지키던 규칙은 그대로다. `canon init`이 만든 로컬 hot 저장소가 서비스 없이 도는지 보던 시험은 Run을 쓰는 명령(`canon dispatch begin`)으로 옮긴다.

### F3 Run 기록

- `canon dispatch begin`과 `end`가 쓰는 Run 기록은 계층 이름, 모양, 중복 쓰기 처리가 0.38과 같다. 이 쓰기가 기대던 도우미가 수집기 파일에 있었으므로 F2의 이동으로 닫는다.

### F4 설정 키

- `canon.yaml`의 `ingest:` 절(`sources`, `sessions.privacy`)을 읽는 코드가 사라진다. canon이 모르는 최상위 키는 이미 읽히지 않고 무시되므로 옛 절이 든 canon.yaml은 오류도 경고도 없이 돈다. `migrate`가 그 절을 지우지는 않는다.
- 이 저장소의 canon.yaml과 `canon init`의 뼈대에는 이 절이 없다. 뼈대의 `session`·`event` 라우팅 줄은 다음 변경(canon-0-39-session-kinds-removal)이 다룬다.

### F5 스킬

- `canon/skills/reference/canon-session-ingest.md`가 번들에서 빠진다. 스킬 색인(`SKILL.src.md`의 표 한 줄, `canon/knowledge-index.json`의 `session-reference` 항목과 `ingest-contract`의 검토 기준 문구)과 `canon-artifact-ingest` 안내의 "다른 파이프라인" 문단이 고쳐진다.
- `canon skills install`은 번들에서 빠진 설치 파일이 잠금과 같으면 지우고, 사용자가 고친 것은 `kept:`로 남긴다. 번들을 갱신하지 않은 저장소의 `skills check`는 그 파일을 고아로 읽고 종료 값 1을 낸다. 0.37→0.38에서 스킬이 빠졌을 때와 같은 길이다.
- 역할 안내문(`role-orchestrator`)의 명령 목록은 `canon schema`에서 생성되므로 재생성으로 바뀐다.

### F6 시나리오

- `.canon/specs/kind=feature/area=ingest/sessions.feature`를 지운다. 시나리오 `ingest.sessions.01`부터 `16`이 든 파일이다. canon은 지워진 `.feature` 파일을 스스로 읽고 그 시나리오마다 묘비 기록(`removed: true`)을 덧붙이므로 게이트와 `status`는 그것들을 없는 것으로 본다. 0.36이 검사기 시나리오를 뺄 때와 같은 길이다. 기존 증거 기록은 장부에 남는다.
- `ingest.sessions.15`(세션 수집이 hot 저장소 위치를 말한다)는 지워진 명령의 일이라 그대로 사라진다. 같은 흐름 `canon-0-30-record-links.F5`는 `init.feature`의 둘과 아래 옮기는 하나가 계속 덮는다.
- `ingest.sessions.16`은 세션과 무관하다. 세션 수집이 아닌 쓰기 명령들(`ingest plans`, `ingest records`, `dispatch begin`, `subject`의 쓰기 넷, `change new`)이 hot 저장소 위치를 말하는지 본다. 이 시나리오는 `plans.feature`로 옮겨 `ingest.plans.14`로 다시 쓴다. 본문과 `@flow:canon-0-30-record-links.F5` 태그는 그대로이고, 새 ID이므로 증거는 새로 기록한다.
- 이름만 `ingest sessions`를 든 다른 시나리오는 고친다. `atip.constraints.21`은 `ingest records --interval-secs`로, `atip.constraints.26`은 `ingest records`와 `ingest plans`로 바꾼다. 둘 다 규칙은 그대로이고 문장의 표본 명령만 바뀐다. 그 밖에 이 변경 때문에 문구가 낡는 `.feature` 줄(`ledger/naming.feature`의 "수집이 쓴다" 문구)을 고친다.
- 주제 `ingest-pipelines`는 계획과 기록 수집도 덮으므로 폐기하지 않는다.

### F7 안내

- CHANGELOG의 Breaking에 제거된 명령, 환경 변수 둘, 설정 키 둘이 표로 오른다. 대체가 없다고 적는다. 이미 저장된 세션·실행·이벤트 기록과 `.canon/ingest/cursors/omp.json`, `codex.json`은 canon이 읽지도 지우지도 않으며 사람이 지워도 된다고 적는다.
- 웹사이트에서 `privacy.mdx`(영어, 한국어)는 전체가 세션 수집 이야기이므로 지운다. 사이드바 항목(`astro.config.mjs`)과 이 페이지로 가는 모든 링크도 지운다. `cli.mdx`(생성됨), `getting-started`, `examples`, `concepts/tiered-storage`(영어, 한국어 쌍)와 랜딩 문구에서 세션 수집을 말한 줄을 고친다. `README.md`의 "통합 에이전트 세션 로깅" 소개와 `ARCHITECTURE.md`의 세 곳(수집 크레이트 설명, 세션 어댑터 문단, 계층 라우팅 문단 중 수집을 말한 부분)을 고친다. 계층 라우팅 문단의 `session`·`event` 말은 다음 변경이 고친다.
- 평가 과제 `privacy-secret-redaction-17`, `privacy-pii-boundary-18`, `session-deterministic-normalize-32`(`evals/tasks.json`)를 지운다. 계획과 기록 수집 시험을 가리키는 다른 과제는 남는다.
- 생성물(`docs/cli/canon.cli.json`, ATIP 고정 파일과 역할 표, `conformance` 기대 출력, 번들)은 재생성한다. 열거에 `ingest sessions`가 들어 있는 기대 출력 3개와 `canon schema` 문서 시험이 달라진다. `.canon/ingest/**` `.gitignore` 줄은 기대 출력에 남는다.

## Acceptance

- F1:
  - `canon ingest sessions --json`과 `canon ingest sessions --watch`가 종료 값 2, 코드 `UNKNOWN_COMMAND`이고 `expected`가 `plans`, `records`다.
  - 실패 경로: `canon selftest --suite session-ingest`는 닫힌 값 거절(종료 값 2)이고, 생성 문서 drift 시험이 `ingest sessions` 줄이 남아 있으면 실패한다.
- F2:
  - 같은 입력의 `ingest plans`와 `ingest records`가 0.38과 같은 보고서와 같은 기록, 같은 계획 커서 파일 이름을 낸다.
  - 실패 경로: 고정 벡터 시험이 `content_digest`의 12자 값 하나라도 바뀌면 실패한다.
- F3:
  - `canon dispatch begin`과 `end`의 Run 기록이 0.38과 같은 모양으로 라우팅된 계층에 저장되고 같은 Run을 다시 쓰면 오류가 아니다.
  - 실패 경로: 저장소가 이미 갖고 있는 Run을 다른 내용으로 쓰면 0.38처럼 덮어쓰지 않고 새 버전으로 저장되며, 읽을 때는 새 버전 하나만 보인다. 같은 내용을 다시 쓰면 아무것도 더 저장되지 않는다.
- F4:
  - `ingest:` 절이 든 canon.yaml로 `status`와 `gate check`가 0.38과 같은 답을 낸다.
  - 실패 경로: 깨진 YAML인 canon.yaml은 지금처럼 그 명령의 설정 오류다. 옛 절이 결과를 바꾸지 않는다.
- F5:
  - `canon skills check`와 `doctor`가 새 번들에서 통과하고 `canon-session-ingest`를 어디에서도 말하지 않는다.
  - 실패 경로: 옛 설치본이 든 저장소의 `skills check`는 고아를 이름으로 말하고 종료 값 1이며, `skills install` 뒤 통과한다.
- F6:
  - `sessions.feature`가 없고 `canon gate check`가 위반 0건이다. `canon query --kind scenario`는 `ingest.sessions.*`를 내지 않는다.
  - 실패 경로: 묘비가 없으면 지워진 시나리오가 `uncovered-cell`로 남으므로 게이트 시험이 그것을 잡는다. `ingest.plans.14`에 증거가 없으면 게이트가 그 칸을 짚는다.
- F7:
  - CHANGELOG에 표가 있고, 웹사이트와 `README.md`와 `ARCHITECTURE.md`에 `ingest sessions`가 없다.
  - 실패 경로: 항목이 없으면 릴리스 확인 시험이 실패한다.
- ==**변경 뒤 `cargo build --workspace`가 경고 없이 통과하고, `cargo nextest run --workspace`, 생성 문서 drift 시험, 적합성 시험(`conformance`), `just atip-deep`, `canon skills check`, `canon gate check`가 통과한다.**==

## Questions and assumptions

- 결정: 범위는 `canon ingest sessions`뿐이다. `ingest plans`(워크플로의 핵심: 변경·작업 가져오기와 `task done` 재가져오기)와 `ingest records`(학습의 시도 공급)는 남는다. `canon-ingest` 크레이트는 지우지 않고 줄인다. 이름을 바꾸는 일은 따로 한다.
- 결정: `canon dispatch`가 쓰는 Run 기록, `SessionId`, 행위자의 `session_id`, Run의 `session_id`는 남는다. 리뷰어 독립성 검사가 읽는다. 세션 수집과 이름만 닮았다.
- 결정: 대체 명령이 없으므로 제거된 철자 표에 줄을 더하지 않고 일반 `UNKNOWN_COMMAND`로 거절한다.
- 결정: `ingest:` 설정 절은 경고 없이 무시한다. 읽는 코드가 없고 알 수 없는 최상위 키는 이미 무시되기 때문이다. 라우팅·에이징의 `session`·`event` 키는 다르다(알 수 없는 종류 이름이 지금 설정 오류이므로) 그래서 다음 변경이 경고와 `migrate` 재작성으로 따로 다룬다.
- 결정: 저장된 기록은 이 변경이 건드리지 않는다. 기록의 종류(`session`, `event`)와 그 위에 세운 보고서·표는 다음 변경이 걷어낸다. 이 변경이 들어간 뒤 그 종류의 기록을 새로 만드는 길은 없다.
- 결정: 비용·토큰 보고는 이 변경으로 데이터원이 사라진다. 대체하는 다른 데이터원은 없다.
- 결정: 시나리오는 `.feature` 파일 삭제로 퇴역시킨다. 주제 폐기도, 시나리오 하나씩의 `retired` 표시도 아니다(시나리오의 상태는 묘비 하나뿐이다). 세션과 무관한 시나리오 하나만 남는 명령의 파일로 옮긴다.
- 실패 방식: 이 변경만 들어가고 다음 변경이 아직 없는 동안에는 `RecordKind`에 `Session`과 `Event`가 남아 있고 보고서의 세션 비용 표는 빈 표를 낸다. 같은 릴리스에 둘 다 싣는다.

## Out of scope

- `RecordKind`의 `Session`과 `Event`, 그 구조체와 스키마, `canon.yaml` 라우팅·에이징 키의 허용과 `migrate` 재작성, `purge --kind session|event`, 세션 관련 마트·보고서 패널·대시보드 패널(canon-0-39-session-kinds-removal).
- 크레이트 이름 바꾸기, `ingest` 그룹 이름 바꾸기, `brief`와 `markdown`을 다른 크레이트로 가르는 일.
- 저장된 세션·실행·이벤트 기록과 고아 커서 파일의 삭제.
- 대화 기록을 읽는 새 기능이나 외부 도구로의 안내.
- `docs/research/**`와 `docs/design/cli-commands-0.28.md`의 옛 서술(역사 문서).

Approved: Journey Hwang 2026-11-09T03:52:03Z — "승인: F3 실패 경로를 코드 동작(새 버전 저장)에 맞춰 고친 브리프" (relayed by planner planner-oco via chat) (code tree 95efa39f9e6567eca108728d0627f3b4410a1aec) (brief sha256:2058030074f5c5725aba95df47eca0a9cf8e12c18887b7ab46e9ef095fb8be1f)
