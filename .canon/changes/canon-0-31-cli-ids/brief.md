# 0.31 CLI id 플래그를 명사로

Source: skill cli-interface-design "Identifier options" (소유자 기본 규칙 2026-10-26); docs/design/cli-commands-0.28.md; docs/design/cli-vocabulary.tsv; docs/cli/canon.cli.json

<!-- canon:section intent role=context -->
## Intent

canon은 레코드나 설정 항목의 id를 받는 옵션 14개를 `--<명사>-id`로 쓴다. 24개 명령에 모두 67곳이다. `--actor-id` 16, `--session-id` 15, `--project-id` 11, `--change-id` 6, `--run-id` 4, `--task-id`·`--subject-id`·`--scenario-id` 각 3, 나머지 여섯 옵션 각 1이다. 옵션 이름이 대상을 가리키고 값이 그 대상의 id인데 이름에 `-id`가 또 붙는다. `--input` 문서의 키에도 `Id`가 붙는다. 소유자의 기본 규칙은 접미사 없는 명사다. 이 변경이 14개를 모두 그 규칙에 맞춘다.

0.31 순서: CI → 승인과 adopt → 버그 수정 → brief 포맷 → CLI id → 검색. 여섯 brief는 함께 승인받는다. 승인한 brief를 고치면 그 brief를 adopt한 모든 subject가 재승인 전까지 막힌다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- ==**id를 받는 옵션은 접미사 없는 명사이고, 값은 그 대상의 정확한 id다.**== 예를 들어 `--change add-login`, `--scenario auth.login.01`, `--actor reviewer-1`이다.
- 14개가 모두 바뀐다. 목록은 Step detail F1의 표에 있다.
- 옛 `-id` 철자는 별칭 없이 사용 오류가 되고, 오류의 힌트가 같은 줄을 새 철자로 고쳐 다시 보여 준다.
- `--input` 문서 키는 옵션 이름을 따라 `Id`가 빠진다. 저장된 레코드, 보고서 JSON, `--fields` 값은 id가 붙은 키 이름 그대로다.
- canon이 출력하는 명령 줄, 스킬, 웹사이트, README는 새 철자만 쓴다.
- 새 이름이 기존 옵션 이름과 겹치지 않는지 기계가 검사한다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 에이전트나 사람이 id를 가리키는 명령을 쓴다 → 옵션을 접미사 없는 명사로 쓴다 → canon이 이전과 같은 일을 하고 같은 레코드를 쓴다.
2. F2 옛 `-id` 철자가 든 줄이 실행된다 → canon이 거절한다 → ==**아무것도 쓰기 전에 종료 2로 거절하고**== 새 철자를 알려 주며, 힌트의 명령 줄은 그대로 실행된다.
3. F3 에이전트가 `--input` 문서를 쓴다 → 키가 옵션 이름을 따른다 → 옛 키는 거절되고 새 키를 알려 준다.
4. F4 canon이 오류, 힌트, 다음 단계를 낸다 → 필드와 명령 줄이 새 철자다 → 에이전트가 출력된 줄을 그대로 실행한다.
5. F5 에이전트가 스킬이나 `canon schema`를 연다 → 명령표가 새 철자만 담는다 → 흔한 흐름의 명령을 추측 없이 만든다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 새 철자

| 옛 철자 | 새 철자 | 명령 수 |
|---|---|---|
| `--actor-id` | `--actor` | 16 |
| `--session-id` | `--session` | 15 |
| `--project-id` | `--project` | 11 |
| `--change-id` | `--change` | 6 |
| `--run-id` | `--run` | 4 |
| `--task-id` | `--task` | 3 |
| `--subject-id` | `--subject` | 3 |
| `--scenario-id` | `--scenario` | 3 |
| `--skill-id` | `--skill` | 1 |
| `--parent-run-id` | `--parent-run` | 1 |
| `--evaluator-id` | `--evaluator` | 1 |
| `--review-id` | `--review` | 1 |
| `--plugin-id` | `--plugin` | 1 |
| `--contradicting-trajectory-id` | `--contradicting-trajectory` | 1 |

- 값 이름, 값의 형식, 반복 여부, 기본값, 필수 조건은 그대로다. 환경 변수 이름도 그대로다.
- ==**옵션 이름만 바뀌고 명령, 피연산자, 효과, 보고서와 레코드의 모양, 종료 코드는 바뀌지 않는다.**==
- 이름 충돌 검사: 14개 명사는 지금 어느 명령의 옵션 이름이나 전역 옵션과도 같지 않다. `--input`, `--json`, `--repo`, `--as`와도 다르다. 겹치는 명사는 더 구체적인 명사를 쓴다는 규칙은 검사로 남긴다.
- `--project`는 설정에 적힌 스펙 루트의 id이고, 저장소 디렉터리는 `--repo`다. `--spec-root`는 디렉터리를 받는 옵션이 이미 쓰고 있어 대안이 못 된다.
- 한 명령에서 같은 개념이 두 철자로 나뉘지 않는다. `-id`로 끝나는 옵션은 하나도 남지 않는다.
- `evidence audit`는 `--evaluator`와 `--evaluator-version`, `--evaluator-digest`를 함께 갖는다. 뒤의 둘은 평가자의 속성이고 같은 줄기라서 충돌이 아니다.

### F2 옛 철자

- 별칭도, 경고를 내고 실행하는 유예도 없다. 이유와 대안은 Questions and assumptions에 있다.
- 사용 오류는 종료 2, 코드 USAGE다. 오류의 `field`는 입력한 옛 철자이고 `expected`는 그 명령의 유효한 옵션이다.
- 메시지는 줄에서 처음 만난 옛 철자를 말한다("`--scenario-id` was removed in 0.31.0"). 힌트는 입력한 줄에서 옛 `-id` 철자를 모두 새 철자로 바꾼 명령 줄이다.
- 지금은 처음 만난 철자 하나만 알려 주고 나머지 토큰은 입력 그대로 둔다. 힌트를 줄 전체로 고쳐 쓰는 것은 이 변경이 새로 정하는 동작이다.
- `--옛철자=값`과 값이 빠진 `--옛철자`도 같게 거절한다. `--` 뒤 토큰과 다른 옵션의 값은 옛 철자로 읽지 않는다.
- 옛 철자가 있던 명령에서만 이 오류를 낸다. 가진 적 없는 명령은 일반 알 수 없는 인자 오류다.
- 67곳이 모두 제거 표의 행이다. 표는 코드의 한 데이터 표이고 스펙의 제거 섹션과 README의 제거 목록이 거기서 생성된다.
- 옛 행 중 옛 철자가 다시 유효해지는 것은 표에서 뺀다. `--task`, `--plugin`, `--parent-run`(0.28 제거)과 `--change`, `--project`, `--subject`, `--actor`, `--run`(0.24 개명)은 해당 명령에서 다시 실행된다.
- 바꿀 철자가 `-id`인 옛 행(`--agent-id`, `--reviewer`)과 설명문에 `-id` 철자가 든 행(`subject adopt`의 `--change-id` 안내)은 새 철자로 고친다.
- 값이 피연산자로 옮겨진 옛 행(`--scenario-id`, `--run-id` 등)은 이미 입력한 줄을 고쳐 보여 준다. 이 행도 위 규칙에 따라 줄의 다른 옛 철자를 모두 고친다.
- 호출자 조사: 레포 안의 호출자는 같은 변경에서 모두 옮긴다. 스킬 문서와 가이드, 웹사이트(영어와 한국어), README, 예제가 있다. 벤치 스크립트, 테스트, 에빌 trace 분류기, conformance 케이스(106개 중 63개), 스펙 코퍼스도 있다.
- 대시보드의 안내 문구는 0.28이 없앤 `ingest artifacts`를 가리키고 있다. 이 변경에서 `ingest records --run`으로 고친다.
- 설치되는 훅은 이 옵션을 쓰지 않는다. 훅이 부르는 명령은 `canon task done`, `canon gate check`, `canon retrieve --for-role --regime`뿐이다. 그래서 `canon migrate`가 훅을 다시 쓸 일이 없다.
- 레포 밖의 호출자는 조사할 수 없다. 그 스크립트와 CI는 첫 실행에서 종료 2와 바꿀 철자를 받고, 조용히 어긋나지 않는다.

### F3 `--input` 문서 키

- 키는 옵션 이름의 camelCase이고 반복 옵션도 단수다. `scenarioId`는 `scenario`가 되고, 나머지 키도 `Id`를 뗀다. 쓰기 명령 20개의 입력 문서가 바뀐다.
- 옛 키는 INPUT_UNKNOWN_FIELD, 종료 2로 거절하고 아무것도 쓰지 않는다. 오류의 `field`는 문서 안 위치이고, 배치면 그 원소의 키까지 가리킨다.
- `expected`는 유효한 키이고 힌트는 새 키다.
- 플래그와 문서가 같은 옵션에 다른 값을 주면 거절하는 기존 규칙은 그대로고, 플래그는 새 철자만 받는다.
- 저장된 레코드의 키, 보고서와 오류 JSON의 키, 조회 `--fields`의 값은 id가 붙은 이름 그대로다. 이 키들은 레코드 필드를 가리키므로 바꾸지 않는다. 보고서의 스키마 버전도 그대로다.
- 레저, archive, 대시보드의 HTTP 요청 키도 바뀌지 않는다. 마이그레이션은 없다.
- 되돌리기: 릴리스를 되돌린다. 레코드와 레저가 바뀌지 않았으므로 되돌릴 마이그레이션이 없다.

### F4 오류와 생성된 명령

- 오류의 `field`는 새 옵션 철자나 새 키의 위치다. 오류 코드 목록(`canon schema --errors`)의 설명과 해결 문구도 새 철자를 쓴다.
- 생성된 명령 줄은 다음 단계 목록의 명령과 argv, 오류 힌트, `status`의 제안, gate 위반의 시정 명령이다. 스키마 예제와 도움말 예제도 있다. 모두 새 철자를 쓰고 스펙에 대해 파싱된다.
- 이 변경이 바꾸는 것은 철자뿐이다. 생략 부호와 자리표시에 관한 기존 규칙은 그대로다.
- 여러 스펙 루트에서 `--project`를 빠뜨렸을 때의 거절은 자리표시가 든 힌트를 그대로 두고 철자만 바꾼다. 줄이 그대로 실행되는 힌트는 루트가 하나일 때와 옛 철자 거절의 힌트다.
- 이 변경 전에 들어가는 0.31 변경이 새로 만든 `-id` 철자와 메시지도 이 단계에서 옮긴다. 구현 시점에 코드 전체를 다시 찾아 확인한다.

### F5 스킬, 문서, 검사

- 스킬 가이드와 역할 스킬의 명령 블록은 새 철자를 쓴다. 가이드는 규칙을 한 번 말한다. id를 받는 옵션은 접미사 없는 명사이고, JSON 키는 `Id`를 뗀 이름이다.
- 지금 가이드의 "`--<noun>-id` everywhere" 문장은 이 규칙으로 바뀐다.
- 웹사이트 CLI 레퍼런스(영어와 한국어)는 스펙에서 다시 생성한다. README의 제거 목록 생성기는 지금 0.28 행만 싣는다. 0.31 행을 싣는 표를 더한다. 문법 문서와 옵션 어휘 목록의 옵션 열도 새 이름이다.
- 스펙 코퍼스: 옛 철자를 말하는 시나리오 33개(13개 `.feature` 파일, 8개 subject)의 단계 글을 새 철자로 고친다. 단계 글이 바뀌면 시나리오 다이제스트가 바뀌므로 각 시나리오는 새 evidence와 다른 세션의 리뷰가 필요하다.
- 앞선 0.31 변경이 옛 철자로 새로 쓴 시나리오도 같은 방식으로 옮긴다. 입력 문서 키를 말하는 시나리오 하나도 새 키로 고친다.
- 뒤집히는 기존 검사는 넷이다. 옛 철자 거절과 도움말을 말하는 시나리오 둘, 한 이름 검사기의 시나리오와 테스트가 있다. 옛 철자 거절 conformance 케이스와 입력 키 거절 시나리오도 뒤집힌다.
- 한 이름 검사기를 뒤집는다. `-id`로 끝나는 옵션과, 한 개념이 두 철자로 나뉜 경우를 보고한다.
- 아래 흐름은 문법만 맞으면 되는 예시이고 실행하지 않는다. 새 표만으로 이 줄들을 만들 수 있어야 하고, 추측한 곳은 Questions and assumptions에 적는다.

```text
canon change new add-search --subject payments --title "Add search"
canon scenario new payments.search.01 --title "Finds a note" --subject payments --change add-search --flow F1 --actor planner-1 --session s-1
canon subject adopt payments --change add-search
canon dispatch begin --regime implementer/canon/search/abcdef --task add-search#1 --actor impl-1 --session s-2 --skill canon
canon evidence add --scenario payments.search.01 --project root --class test-run --ref "npm test" --role implementer --actor impl-1 --session s-2
canon ingest records --run 01ARZ3NDEKTSV4RRFFQ69G5FAV
canon review scenario payments.search.01 --project root --actor rev-1 --session s-3 --role reviewer --original-spec-ref search.feature
canon finding add --change add-search --round 1 --seq 1 --severity blocker --actor rev-1 --summary "No rate limit" --scenario payments.search.01 --review 01ARZ3NDEKTSV4RRFFQ69G5FAV
canon finding close add-search --round 1 --seq 1 --disposition fixed --actor impl-1
canon query --kind task --change add-search
canon retrieve --for-role reviewer --domain dev --subject payments
```

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: 14개 옵션을 새 철자로 쓴 줄이 실행된다. 0.30 바이너리가 같은 fixture에 옛 철자로 쓴 레코드와 비교한다. 시각, ULID, 다이제스트를 가리는 conformance 정규화 뒤에 바이트까지 같다. 비교 fixture에는 brief가 없다. 실패 경로: 옛 `--change-id`로 같은 줄을 실행하면 종료 2이고 레저는 그대로다.
- F2: 제거 표의 행마다 해당 명령에서 USAGE 종료 2가 나온다. `field`가 옛 철자이고 힌트가 새 철자를 말한다. `--옛철자=값`도 같다.
- F2: `--` 뒤 토큰은 거절하지 않는다. 옛 철자가 없던 명령은 일반 오류다. `dispatch begin --task add-search#1`은 다시 실행된다.
- F2: ==**옛 철자가 여럿 든 줄은 오류의 힌트 줄을 그대로 실행하면 성공한다.**== 실패 경로가 곧 이 단계의 거절이다.
- F3: 새 키 문서와 같은 뜻의 플래그가 같은 결과를 낸다. 시각, ULID, 다이제스트를 가리는 conformance 정규화 뒤에 바이트까지 같다. 같은 fixture의 보고서 JSON과 저장 레코드는 키가 0.30과 같다.
- F3: 실패 경로: 두 번째 원소가 옛 `scenarioId` 키를 쓴 배치는 INPUT_UNKNOWN_FIELD 종료 2다. `field`가 그 원소의 키를 가리키고 첫 원소도 쓰이지 않는다.
- F4: 스펙 루트가 둘인 레포에서 `--project` 없이 `feature new`를 실행하면 SPEC_ROOT_UNRESOLVED 종료 2다. 메시지와 힌트의 철자만 `--project`로 바뀌고 자리표시는 그대로다.
- F4: 없는 spec root id를 준 `evidence add --project`는 UNKNOWN_SPEC_ROOT 종료 2다. 루트가 하나인 레포에서 `field`가 `--project`이고 힌트가 그 루트를 말하는 `--project` 줄이다. 없는 id를 준 `finding add --review`는 `--review`를 가리키며 거절된다. 모든 생성 명령 줄이 스펙에 대해 파싱되고 옛 철자는 하나도 없다.
- F5: `canon schema <명령> --json`의 옵션에 `-id` 철자가 없다. 스킬과 웹사이트의 명령 줄이 모두 파싱된다.
- F5: 실패 경로: `--change`와 `--change-id`가 함께 있는 명령 트리를 검사기에 주면 두 철자와 명령 목록이 보고된다. 스킬 문서에 `--actor-id`가 남으면 생성 명령 파싱 검사가 실패한다.
- F5: 스펙 코퍼스에 옛 `-id` 철자가 없고 `canon format`이 통과한다. 옛 철자를 말하던 시나리오 33개가 새 evidence와 다른 세션의 리뷰를 받은 뒤 `canon gate check`가 clean이다. 실패 경로: 리뷰를 받지 않은 시나리오는 `gate check`가 낡은 리뷰로 보고한다.

<!-- canon:section questions role=question -->
## Questions and assumptions

- 질문(정책): 옛 철자를 한 릴리스 동안 경고와 함께 실행할까, 바로 제거할까? 스킬은 가이드를 읽는 호출자에만 즉시 제거를 허용한다. 레포 안 호출자와 설치된 훅은 전부 확인했으나 레포 밖 스크립트는 알 수 없다. 가정: 즉시 제거한다. 유예 장치는 0.25에서 코드에서 없앴으므로 되살려야 하고, 옛 철자는 첫 실행에서 종료 2와 바꿀 철자로 바로 드러난다.
- 질문(정책): 0.24가 맨 명사를 접미사로 바꾸고 0.28이 이를 유지했다. 이 변경은 방향을 되돌린다. 에이전트가 배운 접미사 철자는 F2의 힌트가 고친다. 가정: 소유자의 기본 규칙이 우선한다. 에빌 trace의 철자 실패 수를 릴리스 전후로 기록하되 게이트로 쓰지 않는다.
- 질문: `--project`를 스펙 루트의 id로 두면 저장소를 가리키는 말로 오해할 수 있다. 가정: 이름을 그대로 두고 도움말에 "스펙 루트 id"라고 쓴다.
- 질문: `--input` 문서 스키마에는 자체 버전이 없다. 가정: 보고서 스키마 버전과 스펙 버전은 올리지 않고, 릴리스 노트가 옛 키 → 새 키 목록을 담는다.
- 가정: 이 변경은 CI, 버그 수정, 승인과 adopt, brief 포맷 뒤에 들어간다. 그 변경들이 쓰거나 만든 `-id` 철자는 이 변경이 옮긴다. 검색 변경은 이미 새 이름을 전제한다.
- 추측한 곳: `finding add --scenario`는 반복할 수 있고 `evidence add --scenario`는 한 번만 받는다. 기존 동작이므로 가정: 바꾸지 않는다.
- 질문(정책): 옛 철자를 쓰는 사람의 스크립트에도 유예가 없다. `divergence accept`와 `strategy rollback`은 사람이 쓰는 명령이고 스킬은 사람과 스크립트에 한 릴리스 유예를 권한다. 가정: 이 둘도 유예 없이 제거한다. 이유는 위 첫 질문과 같고, 힌트가 첫 실행에서 고칠 줄을 보여 준다.
- 가정: 스펙 코퍼스의 재작성은 시나리오의 `@subject` 태그를 따라 adopt한다. 옛 철자를 말하는 시나리오 33개는 8개 subject에 있다. 레저 이름 파일에서 옵션 철자를 말하는 시나리오는 하나다. evidence 기록 결과 줄의 시나리오로 `evidence-attestation`에 속하고 이미 adopt했다. `tier-storage`의 시나리오(tier age, export, purge의 id 규칙)는 옵션 철자를 말하지 않아 adopt하지 않는다. 파일 하나에 여러 subject의 시나리오가 있으므로 파일 단위로 adopt하지 않는다.
- 가정: 승인한 이 brief를 고치면 이 brief를 adopt한 8개 subject가 재승인 전까지 reopen, ship, evidence 기록이 막힌다. 승인 전에 brief를 확정하고, 승인 뒤에는 고치지 않는다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 명령 이름 변경, 피연산자로 옮기는 일, `-id`가 아닌 옵션의 이름 변경.
- 보고서, 저장 레코드, 오류, `--fields` 값의 키 변경과 레코드 마이그레이션.
- 값 이름, 환경 변수 `CANON_ACTOR`, 대시보드의 HTTP 요청 키.
- 별칭, 경고 후 실행하는 유예 릴리스.
- 레포 밖 스크립트의 마이그레이션 도구.
- docs/research의 옛 분석 문서와, 레저와 archive의 과거 레코드 안에 남은 명령 문자열.
- 에빌 러너 자신의 `--task-id` 옵션(canon 옵션이 아니다).

<!-- canon:waive prose.length brief "옵션 14개 표, 철자 거절 규칙, 입력 문서 키, 생성 명령, 호출자 조사를 한 변경에서 정해야 해서 줄이면 사실이 빠진다" -->
<!-- canon:waive prose.length intent "14개 옵션의 개수와 명령 수를 근거 숫자로 적어서 줄이면 현재 동작이 빠진다" -->
<!-- canon:waive prose.length detail "단계마다 규칙과 호출자 조사 결과를 담아야 하는 변경이라 줄이면 사실이 빠진다" -->
<!-- canon:waive prose.length questions "정책 질문 셋과 가정이 서로 다른 결정이라 함께 적어야 한다" -->
<!-- canon:waive prose.length acceptance "여섯 단계마다 실패 경로를 요구하는 규칙과 스펙 코퍼스 재리뷰 조건 때문에 줄일 수 없다" -->
<!-- canon:waive prose.code-citation detail "CANON_ACTOR는 사용자가 설정하는 환경 변수 이름이다" match="CANON_ACTOR" -->
<!-- canon:waive prose.code-citation detail "scenarioId는 사용자가 쓰는 JSON 키 철자 자체다" match="scenarioId" -->
<!-- canon:waive prose.code-citation acceptance "scenarioId는 사용자가 쓰는 JSON 키 철자 자체다" match="scenarioId" -->
<!-- canon:waive prose.code-citation acceptance "INPUT_UNKNOWN_FIELD는 사용자가 받는 오류 코드다" match="INPUT_UNKNOWN_FIELD" -->
<!-- canon:waive prose.code-citation acceptance "SPEC_ROOT_UNRESOLVED는 사용자가 받는 오류 코드다" match="SPEC_ROOT_UNRESOLVED" -->
<!-- canon:waive prose.code-citation detail "INPUT_UNKNOWN_FIELD는 사용자가 받는 오류 코드다" match="INPUT_UNKNOWN_FIELD" -->

<!-- The human records their answer, and only that answer, with: canon brief approve --approver <who> --note "<their words>" -->

Approved: Journey Hwang 2026-10-27T10:13:52Z — "레츠고" (brief sha256:c4698decd6c6beb20ce37425dd66661bbca615358e364d8e5b1ddeb6cac716c7)
