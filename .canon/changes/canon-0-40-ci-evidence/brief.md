---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "필드 이름과 옵션 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "정책 키 이름은 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "정책 키와 명령줄은 사용자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "옵션, 환경변수, 필드, 정책 키, 오류 코드, 이유 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "오류 코드와 이유 이름은 시험이 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "결정과 버린 설계가 정책 키와 옵션 이름을 가리킨다"
  - rule: "prose.length"
    section: "brief"
    reason: "기록 모양, 거절, 정책 판정 다섯 이유, 호환을 한 변경으로 묶어 그 사실이 모두 필요하다"
  - rule: "prose.length"
    section: "intent"
    reason: "소비 저장소의 사건과 이 저장소에서 잰 값이 둘 다 필요하다"
  - rule: "prose.length"
    section: "detail"
    reason: "환경변수 표, 필드 목록, 다섯 이유의 철자를 빼면 구현자가 규칙을 다시 추측해야 한다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "흐름마다 긍정과 실패 경로를 요구한다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정과 버린 설계를 빼면 다음 사람이 명령 모양과 정책 자리를 다시 논의한다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 필드 이름과 변수 이름의 풀이다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "괄호는 버린 설계의 이유다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 측정 조건이다"
---

# CI가 증거를 남기면 실행 신원과 보고서 수가 기록에 있고, 정책으로 현재 코드 트리의 완전한 CI 실행만 받을 수 있다

Source: canon 이슈 #119, canon-0-28의 코드 트리 기록(`codeTree`)과 `evidence add --all-scenarios`, 이 저장소 증거 장부 1,966건을 0.39.0 소스와 파일로 잰 값

## Intent

배치 `canon evidence add --all-scenarios --report junit:<경로>`는 보고서를 묶고 코드 트리를 기록하고 판정을 케이스에서 낸다. 빠진 것은 그 보고서가 어디서 어떻게 나왔는지다. 소비 저장소(테스트 약 13,000개, 여러 에이전트가 한 브랜치)에서 에이전트들이 부하가 큰 기계에서 스위트를 돌려 통과한 실행의 보고서만 남기고, 실패한 파일은 따로 다시 돌렸다. 묶인 보고서는 모두 초록이었는데 같은 커밋의 뒤 전체 실행은 32건이 실패했고 `gate check`는 내내 clean이었다. 보고서와 "설정한 스위트의 완전한 한 번의 실행"을 잇는 것이 장부에 없다.

0.39.0으로 잰 값은 이렇다. 기록에는 실행 신원 필드가 없다. 보고서의 digest는 시나리오가 맞은 케이스마다 붙는 첨부로만 남아서, 한 시나리오의 기록은 그 보고서의 케이스가 몇 개였는지 모르고 맞은 케이스가 없는 보고서는 어느 기록에도 digest가 없다. 어떤 명령이 만들었는지는 `--ref`가 말하는데 기본값이 보고서 경로라서 명령이 아니다. 정책에는 증거의 출처를 요구하는 줄이 없고, 배포 가드(`subject move --to shipped`)는 판정과 케이스와 리뷰를 보지만 증거가 어디서 만들어졌는지는 보지 않는다.

이 저장소 장부의 증거 기록 1,966건 가운데 보고서를 묶은 것이 1,635건이다. 보고서가 `target/` 아래(gitignore됨)인 1,604건은 전부 `worktree_dirty`가 거짓이고, 저장소 루트의 `junit.xml`을 묶은 31건은 ==**31건 모두 `worktree_dirty`가 참이다.**== 추적되지 않은 보고서 파일이 작업 트리의 코드 트리를 바꾼다. CI가 남긴 코드 트리가 나중에 같은 커밋에서 계산한 값과 같으려면 이 경우를 막아야 한다.

## Desired outcome

- CI 작업은 `--from-ci`로 배치를 기록한다. 기록마다 ==**공급자, 실행 번호와 시도, 실행 주소, 저장소, CI가 말하는 커밋, 보고서 digest와 케이스 수**==가 남는다. 값은 CI 환경변수에서 읽는다.
- 사람과 에이전트가 지금 쓰는 길은 그대로다. `--from-ci`가 없으면 기록은 0.39와 바이트까지 같다.
- 배치는 어느 보고서에서나 케이스가 몇 개 돌았는지 출력한다. 케이스 한 파일만 다시 돌린 부분 실행은 수가 작아서 눈에 보인다.
- 정책 `spec_coverage.require_ci_run`을 켜면 배포 가드와 `gate check`와 `status`는 범위 안의 시나리오마다 세 가지를 본다. 가장 새 증거가 CI 기록이고, 코드 트리가 지금의 것과 같고, 돈 케이스 수가 선언한 최솟값 이상이어야 한다. 정책이 없으면 아무것도 달라지지 않는다.
- 스킬에 GitHub Actions 조리법이 있다.

## User flow

1. F1 CI 작업은 테스트를 JUnit으로 돌리고 `canon evidence add --all-scenarios --from-ci --ref "<스위트 명령>" --report junit:<경로>`를 돌린다 → 시나리오마다 기록이 쓰이고 기록마다 `ci` 블록이 있다 → ==**같은 실행의 기록은 모두 같은 `ci`를 말한다.**==
2. F2 CI가 아닌 곳이나 정직한 실행이 아닌 상태에서 `--from-ci`를 준다 → 거절되고 아무것도 쓰이지 않는다 → 이유와 고칠 방법이 나온다.
3. F3 사람이나 에이전트는 배치의 출력을 읽는다 → 보고서마다 케이스 수가 있다 → 부분 실행이 부분으로 보인다.
4. F4 정책이 `require_ci_run`을 켠 저장소에서 주제를 배포하거나 `gate check`를 돈다 → CI가 현재 코드 트리에서 완전하게 남긴 증거만 통과한다 → 아니면 시나리오마다 이유와 고칠 명령이 나온다.
5. F5 에이전트는 `status`와 `context`를 읽는다 → 같은 요구와 같은 틈을 본다.
6. F6 정책을 켜지 않은 저장소와 0.39까지의 기록은 그대로 읽힌다 → 판정이 같다.
7. F7 사람은 스킬과 CHANGELOG를 읽는다 → 조리법과 새 필드와 새 정책이 있다.

## Step detail

### F1 `--from-ci`와 `ci` 블록

- 새 옵션은 `--from-ci` 하나다. `--from-ci`는 `--all-scenarios`와 명시적인 `--ref`(스위트를 돌린 명령줄)와 함께만 받는다. 단일 시나리오 기록은 완전한 실행이 아니고, `--ref`의 기본값인 보고서 경로는 어떤 스위트인지 말해 주지 않는다. 별도 명령 `evidence ci`는 만들지 않는다. 배치의 인수와 묶는 일을 전부 되풀이하게 된다.
- `--from-ci`는 CI 환경변수를 명시적으로 읽는다. 자동으로 CI를 알아보지 않는다. 지금의 기록은 CI 안에서 사람이 돌려도 바뀌지 않는다.
- 지원 공급자는 GitHub Actions 하나다. 읽는 변수는 `GITHUB_ACTIONS`(`true`여야 한다), `GITHUB_RUN_ID`, `GITHUB_RUN_ATTEMPT`, `GITHUB_SERVER_URL`, `GITHUB_REPOSITORY`, `GITHUB_SHA`이고 있으면 `GITHUB_WORKFLOW`와 `GITHUB_JOB`도 읽는다. 다른 공급자는 변수 표 한 줄을 더하는 일이고 이번 변경 밖이다.
- 기록의 새 선택 필드 `ci`는 다음을 담는다.
  - `provider`(문자열, 지금은 `github-actions`), `run`(실행 번호), `attempt`(정수), `url`(`<서버>/<저장소>/actions/runs/<실행>/attempts/<시도>`), `repository`.
  - `sha`(`GITHUB_SHA` 그대로), `workflow`와 `job`(있을 때만).
  - `reports`: 묶은 보고서마다 `path`, `sha256`, `format`, `passed`, `failed`, `skipped`.
- `provider`는 문자열이다. 닫힌 열거로 두면 뒤 버전이 공급자를 더한 기록을 이 버전이 읽지 못한다(기록의 잘못된 값은 기록 전체를 읽지 못하게 한다). 코드 트리와 `evidence_sha`는 이미 있는 필드가 말하고 `ci`가 복사하지 않는다.
- `sha`가 `evidence_sha`(기록 때의 HEAD)와 다를 수 있다. `pull_request` 실행에서 PR 머리를 체크아웃하는 작업이 그렇다. 둘을 모두 기록하고 거절하지 않는다.
- 같은 배치의 기록은 모두 같은 `ci` 값을 갖는다. `reports`는 보고서 파일마다 한 줄이고 시나리오가 맞은 케이스가 없는 보고서도 든다.
- 돈 케이스 수는 `passed + failed`다. 건너뛴 케이스는 돌지 않았다. 세는 단위는 보고서 형식의 케이스다. JUnit의 testcase, Cucumber의 시나리오, Swift Testing의 테스트를 세고 시나리오 id가 없는 케이스도 센다.
- 같은 `ci` 값으로 다시 부른 배치(`GITHUB_RUN_ATTEMPT`가 같고 보고서가 같다)는 지금의 재시도 안전 규칙대로 기록을 다시 쓰지 않는다(`unchanged`). 시도가 다르면 새 기록이다.
- 리뷰와 수락된 divergence는 `ci`를 보지 않는다. 리뷰가 붙잡는 것은 판정과 검사의 정체(보고서 형식과 케이스 이름)이고, 같은 스위트를 CI가 다시 기록해도 리뷰는 그대로 유효하다.

### F2 거절

- `GITHUB_ACTIONS`가 `true`가 아니거나 위 변수 가운데 하나가 비었으면 `CI_ENV_REQUIRED`(종료 2)이 빠진 변수를 말한다. 아무것도 쓰이지 않는다. 코드 이름은 atip/1 7.2.1의 열다섯 조건(`REQUIRED`, `CONFLICT` 등)에서 고른다. `MISSING`과 `DIRTY`는 그 조건이 아니다.
- ==**`--from-ci` 기록은 작업 트리가 HEAD와 같을 때만 쓴다.**== 작업 트리의 코드 트리가 HEAD의 것과 다르면(`worktree_dirty`) `CI_WORKTREE_CONFLICT`(종료 2)가 거절한다. 흔한 원인은 gitignore되지 않은 곳의 보고서 파일이고, 안내는 보고서를 gitignore된 디렉터리나 `.canon/` 아래에 쓰거나 `code.exclude`에 넣으라고 한다.
- `--from-ci`는 `--rev`와 `--verdict`와 같이 못 쓴다. 기록의 코드는 테스트가 돈 작업 트리여야 하고 판정은 보고서의 케이스에서만 나와야 한다. 둘 다 `EVIDENCE_ARGUMENT_CONFLICT`다.
- `--from-ci` 없이는 위 어떤 것도 달라지지 않는다. 특히 더티한 작업 트리는 지금처럼 기록되고 거절되지 않는다.

### F3 케이스 수

- 모든 `--report` 배치(`--from-ci`가 없어도)가 보고서마다 `<경로>: <passed> passed, <failed> failed, <skipped> skipped`를 한 줄씩 출력한다. `--json`의 `data.reports[]`는 같은 값을 `path`, `sha256`, `format`, `passed`, `failed`, `skipped`로 낸다(추가 필드, 기존 필드는 그대로).
- 수는 기록에 들어가지 않는다(`--from-ci`가 아니면). 출력만 느는 변경이다.
- 부분 실행의 판정은 F4의 `min_cases`가 한다. canon은 스위트에 케이스가 몇 개여야 하는지 모른다.

### F4 정책 `spec_coverage.require_ci_run`

- 새 하위 절이다(`require_review`, `require_audit`와 같은 자리). `scope`(배포 가드와 `gate check`가 보는 주제 상태, 기본 `[shipped]`)와 `min_cases`(1 이상의 정수, 선택)를 갖는다. 모르는 키, `null`, 0, 음수, 정수가 아닌 값은 `spec_coverage` 절 전체를 잘못된 것으로 만든다(조용히 꺼지지 않는다). 빈 매핑 `require_ci_run: {}`은 기본값이다.
- 판정 대상은 범위 안의 주제가 가진 시나리오(`exclude_lanes`에 든 레인 제외)마다 가장 새 선언 증거 기록 하나다. 시각이 가장 늦은 기록이고 감사 기록은 아니다. 증거가 아예 없는 시나리오는 지금의 증거 요구가 따로 말한다.
- 가장 새 기록이 아래 하나라도 해당하면 `gate check`의 위반이다. 분류는 `uncovered-cell`, 코드 트리가 어긋나면 `stale-evidence`다. 이유 이름은 세부에 들어간다.
  - `no_ci_run`: `ci`가 없다. 기록한 행위자와 고칠 명령을 말한다.
  - `other_code_tree`: 기록의 `code_tree`가 지금의 것과 같지 않다(기록에 없으면 같지 않다). 기록의 트리와 지금의 트리를 말한다.
  - `dirty_worktree`: 기록의 `worktree_dirty`가 참이다.
  - `short_run`: 기록의 `ci.reports`에서 돈 수의 합이 `min_cases`보다 작다. 돈 수와 최솟값을 말한다.
  - `code_tree_unknown`: 지금의 코드 트리를 git에서 얻지 못했다. 켠 정책은 모르는 채로 통과시키지 않는다.
- 가장 새 기록이 사람이나 에이전트의 것이면 그 뒤의 CI 기록이 나올 때까지 위반이다. 이것이 의도다. 모은 보고서로 낸 증거가 CI의 증거를 대신하지 못한다.
- 배포 가드(`subject move --to shipped`)는 `scope`가 `shipped`를 덮을 때 같은 판정을 한다. 한 함수가 `gate check`와 가드와 `status`의 빨강을 모두 낸다. 기록 쪽 코드 트리는 `gate check`가 이미 계산한 현재 트리를, 가드는 그 자리의 메모를 쓴다. `--override-reason`은 이 위반을 면제하지 않는다(리뷰 점검만 면제된다).
- 비용은 시나리오 수에 비례한다. 기존 증거 색인으로 찾고 장부를 다시 훑지 않는다.

### F5 status와 context

- `status`와 `context`는 `require_ci_run` 한 줄을 `require_audit`처럼 말하고, `status`의 정책 수준 판정은 `custom`이 된다. 정책이 없으면 새 줄이 없다.

### F6 호환

- 새 필드는 모두 선택이고 비었을 때 쓰이지 않는다. `ci`가 없는 기록의 JSON은 0.39와 같다. 0.39 바이너리는 `ci`가 든 기록의 모르는 키를 무시한다.
- `SpecCoverage::Active`의 구조 분해(31곳)에 새 필드가 더해진다. 필드 없는 정책의 해석은 같다.
- 정책 키는 `spec_coverage` 안의 하위 키이고 `KNOWN_TOP_LEVEL_KEYS`를 늘리지 않는다.

### F7 안내

- 스킬 `canon-gate.md`의 "One report, many scenarios" 뒤에 GitHub Actions 조리법을 싣는다. 순서는 이렇다.
  - ① 테스트를 JUnit으로 gitignore된 경로에 쓴다.
  - ② `if: always()`로 `canon inventory sync`와 `canon evidence add --project <루트> --all-scenarios --from-ci --ref "<명령>" --report junit:<경로> --role implementer --promote`를 돈다. 실패한 실행도 기록해야 divergent가 남는다.
  - ③ 기록을 올린다. `.canon/ledger`와 `.canon/artifacts`를 `actions/upload-artifact`로 올리거나, `permissions: contents: write`로 커밋해 푸시한다. `GITHUB_TOKEN`으로 한 푸시는 새 워크플로 실행을 시작하지 않는다.
  - 체크아웃은 `fetch-depth: 0`이다.
- `canon-policy.md`는 새 키를, `canon-subject.md`는 가드의 새 점검을 말한다. 생성 문서(`schemas/evidence.schema.json`, `docs/cli/canon.cli.json`, 웹 `cli.mdx`, 스킬의 명령 블록)는 다시 만든다.
- CHANGELOG의 Added에 `--from-ci`, `ci` 블록, `data.reports[]`, 정책 키, 새 오류 코드 둘을 싣는다. `ci`는 환경변수가 말하는 주장이다. 큐레이션한 보고서는 막고 위조는 막지 못한다고 적는다.

## Acceptance

- F1:
  - 여러 시나리오를 가진 보고서와 CI 환경변수를 준 배치가 기록 하나당 `ci`를 쓰고, `provider`, `run`, `attempt`, `url`, `repository`, `sha`, `reports[].path/sha256/format/passed/failed/skipped`가 환경과 보고서의 값과 같다. 같은 배치의 기록은 `ci`가 모두 같다.
  - 시나리오가 맞은 케이스가 없는 보고서도 `reports`에 든다.
  - 같은 `GITHUB_RUN_ATTEMPT`로 같은 배치를 다시 부르면 `unchanged`이고 다른 시도로 부르면 새 기록이다.
  - 같은 스위트를 CI가 다시 기록해도 기존 리뷰는 유효하다.
  - 실패 경로: `--from-ci`를 `--all-scenarios` 없이, 또는 `--ref` 없이 주면 종료 2이고 아무것도 쓰이지 않는다.
- F2:
  - `GITHUB_ACTIONS`가 없거나 `GITHUB_RUN_ID`가 빈 환경은 `CI_ENV_REQUIRED`이고 빠진 변수를 말한다.
  - 추적되지 않은 `junit.xml`이 있는 작업 트리는 `CI_WORKTREE_CONFLICT`이고 gitignore된 경로의 보고서는 통과한다.
  - 실패 경로: `--from-ci --rev <커밋>`, `--from-ci --verdict faithful`은 `EVIDENCE_ARGUMENT_CONFLICT`이고 아무것도 쓰이지 않는다. `--from-ci` 없는 같은 배치는 더티한 작업 트리에서도 0.39처럼 기록된다.
- F3:
  - 보고서 둘의 배치가 보고서마다 한 줄을 출력하고 `data.reports[]`가 같은 수를 낸다. `--from-ci`가 없어도 낸다.
  - 실패 경로: 파싱할 수 없는 보고서는 지금처럼 거절되고 수를 말하지 않으며 아무것도 쓰이지 않는다.
- F4:
  - 정책이 `require_ci_run: {}`인 저장소에서 CI 기록(현재 코드 트리, 완전한 실행)이 최신인 시나리오는 `gate check`와 가드를 통과한다.
  - `no_ci_run`, `other_code_tree`(`stale-evidence`), `dirty_worktree`, `short_run`, `code_tree_unknown`이 각각 시나리오 하나와 이유 이름으로 나오고, `status`의 빨강과 가드의 거절이 같은 이유를 낸다. 사람의 기록이 CI 기록보다 새로우면 `no_ci_run`이다.
  - `min_cases: 100`일 때 돈 수 합이 99면 `short_run`, 100이면 통과한다.
  - 실패 경로: ==**`min_cases: 0`, `scope: [done]`, 모르는 키, `require_ci_run:`(null)은 `spec_coverage`를 잘못된 절로 만들어 `gate check`가 위반을 낸다.**== `--override-reason`은 이 위반을 면제하지 않는다.
- F5:
  - `context`가 `require_ci_run` 한 줄을 말하고 `status`가 정책 수준을 `custom`으로 읽는다.
  - 실패 경로: 정책이 없으면 둘 다 새 줄이 없다.
- F6:
  - 정책이 없는 저장소의 `gate check`와 가드의 답이 0.39와 같다. `ci`가 없는 기록의 JSON은 0.39와 같다.
  - 실패 경로: 형식이 틀린 `ci`(예: `attempt`가 문자열)가 든 기록은 읽히지 않고 기존의 잘못된 기록 규칙대로 보고된다. 모르는 키만 더 든 `ci`는 읽힌다.
- F7:
  - 스킬 조리법과 CHANGELOG 항목이 있다.
  - 실패 경로: 항목이나 생성 문서가 어긋나면 릴리스 확인과 생성 문서 drift 시험이 실패한다.
- 새 오류 코드 `CI_ENV_REQUIRED`와 `CI_WORKTREE_CONFLICT`는 레지스트리에 등록되고 시험 모음이 내보낸다(T9 witness). `cargo test --workspace`, `just atip-deep`, `canon gate check`가 통과한다.

## Questions and assumptions

- 결정: 이슈의 `--from-ci`를 따른다(별도 `evidence ci`를 버렸다. 배치의 인수와 묶는 규칙을 되풀이한다). CI 환경을 자동으로 알아보는 것도 버렸다. 지금의 기록이 CI 안에서 사람이 돌릴 때 말없이 달라지고, 에이전트가 CI 같은 환경에서 돌릴 때 그 기록이 CI의 것으로 읽힌다.
- 결정: 정책은 이슈의 `evidence.require: ci-run` 대신 `spec_coverage.require_ci_run`이다. `evidence` 최상위 키는 `KNOWN_TOP_LEVEL_KEYS`를 늘리고 `scope`와 `exclude_lanes`와 `require_evidence`가 도는 자리에서 떨어진다. `experimental.evidence_binding`에 `strength: ci-run`을 더하는 길도 버렸다. 그 절은 실험이고 `gate check`만 읽으며 배포 가드는 읽지 않는다.
- 결정: 완전한 실행은 선언한 최솟값(`min_cases`)과 기록한 돈 수의 비교다. 스위트 목록(매니페스트)과의 비교는 canon이 스위트를 알아야 하므로 버렸다. 명령 이름 일치도 버렸다. 문자열은 쓴 사람이 정하고, 부분 실행은 수로 드러난다. 선택이므로 켜지 않으면 신원과 코드 트리만 본다.
- 결정: 현재 코드 트리와 같은지는 기록의 `code_tree`를 `gate check`의 현재 코드 트리와 비교한다(0.28 이후 낡음 검사가 쓰는 같은 값). 커밋 sha는 비교하지 않는다. 기록 커밋은 `.canon/`만 바꾸므로 코드 트리가 같다.
- 결정: 더티한 작업 트리는 `--from-ci`에서 거절한다(기록하지 않는다). 기록만 하고 정책이 거절하게 두면 CI가 아닌 몇 주 뒤 배포 가드가 이유를 모르는 채 막힌다. 이 저장소에서 31/31이 루트 `junit.xml` 때문이었다.
- 결정: `ci`는 환경변수가 말하는 주장이다. 어떤 서명도 덮지 않는다(승인 서명은 `--all-scenarios`와 같이 못 쓰고, 그 페이로드는 주제, 산출물 digest, 표면, 행위자, 시각을 본다). 목적은 에이전트가 모은 보고서를 막는 것이고 공격자가 `GITHUB_ACTIONS=true`를 쓰는 것은 막지 않는다. 실행 주소로 사람이 확인할 수 있다.
- 결정: 위반은 면제할 수 없다(`--override-reason`이 아니다). 정책을 켠 저장소가 정책을 우회하려면 정책을 고치고 사람이 승인한다.
- 결정: 기본 `scope`는 `[shipped]`다. 이미 배포된 주제도 `gate check`에서 코드가 바뀌면 다음 CI 기록까지 빨강이다. CI가 main에서 매 푸시에 기록을 남기는 저장소가 전제다.
- 실패 방식: PR을 스쿼시 머지하면 병합 결과의 코드 트리가 PR 머리의 것과 다를 수 있다(main이 움직였을 때). 그때 main의 CI가 다시 기록할 때까지 `other_code_tree`다. 포크의 PR은 쓰기 토큰이 없어 기록을 올리지 못한다(업로드 아티팩트 길만 있다). `GITHUB_RUN_ATTEMPT`가 오르는 재실행은 새 기록을 쓴다.
- 질문: `require_ci_run`의 기본 `scope`를 `[shipped]`(배포된 주제도 계속 CI 증거를 요구)로 둘지, `[verifying, shipped]`나 가드만의 더 약한 형태로 둘지?
- 질문: canon 자신의 `test.yml`(오프라인 작업)이 이 기능을 이번 릴리스에서 쓰게 할지(nextest JUnit 출력과 기록 올리기, 정책 켜기), 아니면 조리법만 싣고 쓰는 일은 다음으로 미룰지? 쓰려면 CI가 main에 기록을 푸시하는 워크플로 권한이 필요하다.
- 질문: 정책이 기록의 `ci.repository`가 지정한 저장소와 같을 것을 요구하는 선택 키를 이번에 더할지? 위조는 막지 못하지만 다른 저장소의 실행에서 온 기록을 거른다. 기본은 더하지 않는다.

## Out of scope

- GitHub Actions 외의 CI 공급자(변수 표 한 줄씩 더하는 일).
- 실행 신원의 인증(서명, 공급자가 서명한 증명(attestation), OIDC). `ci`는 주장이다.
- 스위트 매니페스트와 그에 대한 완전성 판정, `--ref` 문자열 일치.
- 단일 시나리오 기록(`--scenario`, `--task`)의 `--from-ci`.
- `status`나 `gate check`가 CI 기록을 새로 쓰거나 CI를 시작하는 일(canon은 아무것도 돌리지 않는다).
- canon 자신의 `test.yml`과 `.canon/policy.yaml`의 수정(질문 2가 답해진 뒤 따로).
- 보고서 첨부가 있는 리뷰 핀이나 승인 페이로드에 `ci`를 넣는 일.

Approved: Journey Hwang 2026-11-09T14:24:16Z — "재승인: 리뷰 지적에 따라 고친 브리프 3개" (relayed by planner planner-oco via ask) (code tree b9fe5e9bd8ef5436b3e8990ed06245172b3a003c) (brief sha256:29925e27804cd2a7ab009f83d89f6fae8533276ae91aed7d831ac77c8c25f855)
