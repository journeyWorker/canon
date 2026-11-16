---
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "오류 코드와 이유 이름은 0.41.0이 호출자에게 말하는 거절의 철자 자체다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "명령, 옵션, 기록 종류의 이름은 호출자가 치는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "오류 코드, 이유 이름, 옵션은 시험이 확인하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "시험이 확인하는 오류 코드와 기록 종류의 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "기록 종류와 설정 키는 소유자에게 묻는 대상의 철자 자체다"
  - rule: "prose.length"
    section: "brief"
    reason: "새 명령 셋, 새 기록 종류, 읽는 곳 여럿, 첫 사용을 한 변경에서 정해야 해서 줄이면 거절과 판정이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "닫는 길 넷의 현재 동작과 게이트의 요구를 적어야 모자란 곳이 보인다"
  - rule: "prose.length"
    section: "outcome"
    reason: "결과 여섯 줄이 각각 하나의 닫힘 조건이다"
  - rule: "prose.length"
    section: "flow"
    reason: "아홉 단계마다 호출 하나와 그 결과가 있다"
  - rule: "prose.length"
    section: "detail"
    reason: "단계마다 옵션, 거절 코드, 판정 규칙이 있어야 구현자가 고르지 않는다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "아홉 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정마다 버린 설계의 이유가 있어 줄이면 이유가 빠진다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 측정값의 출처(커밋, 실행, 이슈)와 코드 이름이다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 거절 이유 이름(`SHIPPED`, `CLOSED`, `CANON_YAML`)과 옵션 예시이고 시험이 확인하는 철자이다"
---

# 승인된 변경을 사람이 이유와 근거를 남기고 미루거나 중단할 수 있고, 게이트는 닫힌 변경을 더 요구하지 않는다

Source: `.canon/changes/canon-0-41-ci-nextest/brief.md`(가지 `plan/0.41`, `wip-0.41/ci-nextest`), 커밋 60a4c966f, 0.41 인계 문서 `0.41-owner-handoff.md` §3과 측정 `ci-nextest-measurement.md`(`/Volumes/MoeLive/storage/journey-temp/projects/canon/2026-11-14-0.41-plan-review/`), PR #147, 실행 43498921480, canon-0-32-relay-approval, canon-0-31-loop

## Intent

canon 0.41.0에는 승인된 변경을 계획에서 빼는 명령이 없다. 0.41 릴리스에서 승인된 chore `canon-0-41-ci-nextest`를 내보내지 않기로 했을 때, 그 변경의 브리프와 `gate-trust-spine` 기록의 연결을 릴리스 가지에서 손으로 지웠다(커밋 60a4c966f). 지운 이유는 그 커밋 메시지에만 있어 `canon search`와 `canon query`로 찾을 수 없다.

0.41.0이 닫는 길로 가진 것과 모자란 것이다.

| 길 | 0.41.0이 하는 일 | 모자란 것 |
|---|---|---|
| `subject unadopt` | `Approved:` 줄이 있는 변경은 `SUBJECT_CONFLICT`(이유 `APPROVED_BRIEF`)로, 플로 태그가 붙은 시나리오가 있으면 `FLOW_TAGGED`로 거절한다 | 승인된 변경을 뺄 수 없다 |
| 브리프 수정 | 승인이 `BRIEF_APPROVAL_STALE`이 될 뿐 변경은 그대로 요구된다 | 요구를 없애지 못한다 |
| `divergence defer`, `divergence accept` | 시나리오 하나에 대한 기록이고 미룸은 만료일이 필수이다 | 변경 하나를 담지 못한다 |
| 가져온 계획의 `**DEFERRED to §n**`, `**DROPPED**` | task 행 하나에 붙는 표지이다 | 변경 단위가 아니고 승인과 게이트에 닿지 않는다 |

- 게이트의 요구는 변경이 주제에 붙어 있는 한 이어진다. 승인된 chore는 verifying과 shipped 주제에서 승인 뒤 증거를 다시 기록하라는 요구(`uncovered-cell`)와 변경 리뷰 하나의 요구(`unreviewed-promotion`)를 `gate check`, ship 가드, `status`에 낸다. 정책에서 `block_on_findings`가 켜져 있으면 열린 blocker finding이 주제의 ship을 막고, 승인 없는 브리프는 승인을 요구한다. 0.41 인계 문서도 같은 관측이다("A chore stays unreviewed-promotion forever").
- 사람의 결정을 기록하는 leaf는 `brief approve`, `policy approve`, `divergence accept` 셋이고 모두 에이전트 역할을 `ACTOR_DENIED`로 거절한다. 에이전트는 `--relayed-via <도구>`로 사람의 명시적 답을 옮기고 기록에 옮긴 역할, actor, 도구가 남는다.
- 변경의 `change` 기록은 가져오기가 브리프 파일 수정 시각으로 `at`을 정해 쓴다. 브리프를 고쳐 다시 가져오면 새 기록이 앞의 것을 대체하므로 사람의 결정을 그 기록에 둘 수 없다.
- 플로 태그(`@flow:<change>.<F-id>`)는 `.feature` 파일에만 있고 시나리오 기록에는 없다.
- 기록 종류는 열둘로 닫혀 있고 `canon.yaml`의 `routing:`이 종류마다 한 줄을 요구한다. 0.41.0에서 `finding` 줄을 지우면 `canon query --kind finding`이 `QUERY_FAILED`이고, `change` 줄을 지우면 `change new`가 `CONFIG_INVALID`(이유 `TIERS`)이다. 두 경우 모두 `status`와 `gate check`는 그 종류를 읽지 않고 지나간다. 이 저장소에 추적되는 `canon.yaml`은 90개이고 74개가 `routing:`을 가지며 16개는 없다(conformance와 픽스처 포함). `canon migrate`에는 `routing:`에 줄을 더하는 단계가 없어 새 종류가 생기면 기존 저장소가 줄을 손으로 더해야 한다.
- main(d7518d159)에는 `canon-0-41-ci-nextest`가 없다. `.canon/changes/`에 디렉터리가 없고 `gate-trust-spine` 기록이 그 변경을 나열하지 않는다. 승인된 브리프와 코드는 `plan/0.41`과 `wip-0.41/ci-nextest`에 있다. 그 변경의 정지 규칙이 발동했다: 오프라인 잡이 33분 32초로 기준 30분 1초보다 길다(실행 43498921480, PR #147).

## Desired outcome

- 사람이 승인된 변경을 `canon change defer`로 미루거나 `canon change drop`으로 접는다. 이유는 `--reason`, 근거는 하나 이상의 `--ref`, 승인자는 `--approver`로 남긴다. 에이전트는 사람의 답을 `--relayed-via`로 옮긴다.
- ==**닫힌 변경은 `gate check`, ship과 reopen 가드, `status`가 더 요구하지 않는다.**== 그 변경에만 속한 시나리오도 치워지고 다른 변경과 나눠 쓰는 시나리오와 태그 없는 시나리오는 그대로이다.
- 결정은 `change_state` 기록으로 남아 `canon query --kind change_state`와 `canon search`로 찾는다. 증거, 리뷰, finding은 지워지지 않는다.
- 미룬 변경은 `canon change reopen`으로 되살리고 브리프 승인을 새로 받는다. 중단은 끝이고 되살리려면 새 변경을 쓴다.
- 이미 나간 변경은 닫을 수 없다.
- 0.41에서 뺀 `canon-0-41-ci-nextest`를 이 길로 처음 미룬다.
- 새 기록 종류의 줄이 없는 기존 저장소는 `canon migrate`가 `change_state: local` 한 줄을 더한다. `--commit --plan-digest`와 `--revert`를 쓰는 기존 migrate의 길이다.

## User flow

1. F1 사람이 승인된 변경을 이번에는 내지 않기로 한다 → 사람이(또는 사람의 답을 옮긴 에이전트가) `canon change defer <slug> --reason … --ref … --approver …`를 부른다 → 변경이 deferred로 기록된다 → ==**승인된 변경이 이유와 근거와 함께 미뤄진다.**==
2. F2 사람이 변경을 접기로 한다 → `canon change drop <slug> …`를 부른다 → 변경이 dropped로 기록된다 → 접은 변경이 이유와 함께 남고 되살리지 않는다.
3. F3 에이전트가 `gate check`, `status`, ship을 본다 → 닫힌 변경과 그 변경에만 속한 시나리오가 요구에서 빠지고 빠진 것이 보인다 → 요구가 없는 일을 하지 않는다.
4. F4 나중에 읽는 사람이 왜 빠졌는지 묻는다 → `canon query --kind change_state`나 `canon search <단어>`가 이유, 근거, 승인자, 시각을 준다 → 결정을 다시 만들지 않는다.
5. F5 사람이 미룬 변경을 되살린다 → `canon change reopen <slug> …`를 부른다 → 변경이 주제로 돌아오고 앞의 승인은 무효이다 → 승인을 새로 받고 다시 일한다.
6. F6 에이전트가 닫힌 변경에 승인이나 태그를 쓰려 하거나 승인된 변경을 `subject unadopt`하려 한다 → 닫힌 변경에는 쓰기를 거절하고 `unadopt`의 거절 힌트가 `change defer`와 `change drop`을 말한다 → 올바른 길로 간다.
7. F7 릴리스 담당이 0.41에서 뺀 `canon-0-41-ci-nextest`를 처리한다 → 측정을 근거로 미룸을 기록한다 → 게이트가 그 변경을 요구하지 않고 코드는 `wip-0.41/ci-nextest`에 남는다.
8. F8 안내를 읽는 에이전트 → 닫는 법, 사람 전용인 것, 닫힌 변경이 어떻게 보이는지를 안다 → 사람에게 올바른 한 줄을 내민다.
9. F9 줄이 없는 옛 저장소의 사용자 → 닫는 명령이 설정 오류로 거절하고 힌트가 `canon migrate`를 말한다 → `canon migrate`의 미리보기를 보고 `--commit --plan-digest`로 한 줄을 더하면 닫는 명령이 돈다.

## Step detail

### F1 미루기

- 명령은 `canon change defer <CHANGE_ID> --reason <TEXT> --ref <TEXT>… --approver <NAME> [--relayed-via <NAME> --actor <ACTOR_ID>] [--dry-run] [--input]`이다. `--reason`은 사람이 준 이유를 그 말대로 적는다. `--ref`는 읽는 사람이 열어 볼 수 있는 근거이고(링크, 커밋, 파일, 대화의 날짜) 값마다 옵션을 한 번씩 쓴다(쉼표 목록 없음). 세 값은 비어 있지 않은 한 줄이다.
- 사람 전용이고 `brief approve`와 같은 길이다. 에이전트 역할은 `ACTOR_DENIED`이고 사람의 명시적 답을 `--relayed-via`로 옮길 때만 통과한다. 옮기면 기록에 옮긴 역할, actor, 도구가 남는다. `canon.yaml`의 `approval: { relay: deny }`는 옮김을 거절한다.
- 인자의 거절이다. 옮김에 `--actor`(또는 `CANON_ACTOR`)나 `--approver`가 없으면 `USAGE`이고 역할 없는 `--relayed-via`도 `USAGE`이다. 승인자가 비면 `APPROVAL_INVALID`, 이유나 근거가 비면 `CHANGE_ARGUMENT_INVALID`이다. 그 코드의 설명과 힌트는 `change new`만 말하던 것에서 네 leaf(`new`, `defer`, `drop`, `reopen`)를 말하게 고친다.
- 기록 한 건(`change_state`)에 변경, 상태(`deferred`, `dropped`, `reopened`), 이유, 근거 목록, 승인자, 옮겼다면 역할·actor·도구, 시각(UTC, 초)이 있다. 변경마다 가장 늦은 기록이 현재 상태이고 앞의 기록은 이력으로 남는다. 변경이 여러 주제에 속하면 모든 주제에서 닫힌다.
- 닫을 수 있는 것은 기록이 있는 변경이다. 승인된 변경, 승인 전 초안, chore 모두이다. 없는 변경은 `CHANGE_NOT_FOUND`이다. 승인 전 초안을 닫아도 `subject unadopt`는 연결만 푸는 명령으로 남는다.
- 이미 나간 변경은 `CHANGE_CONFLICT`(이유 `SHIPPED`)로 거절하고 아무것도 쓰지 않는다. 나감은 주제의 이동 이력에서 읽는다(`.canon/archive/kind=subject`, `canon query --history`). 나간 변경의 조건은 셋이다.
  - 주제의 어떤 버전이 shipped로 옮겨졌다. 그 앞 버전은 shipped가 아니다.
  - 그 버전의 변경 목록이 그 변경을 담았다.
  - 그때 그 변경은 승인 전 초안이 아니었다.
- 현재 `Approved:` 줄의 시각은 판정에 쓰지 않는다. 다시 승인하면 줄이 대체되기 때문이다. 승인 시각은 브리프의 git 이력에서 첫 `Approved:` 줄을 읽는다.
- 이미 shipped인 주제에 `subject adopt`가 쓴 버전은 shipped로 옮긴 것이 아니다. 주제를 다시 열었거나 변경을 다시 승인했어도 나간 변경은 그대로 나간 변경이다.
- 이동 이력이나 승인 이력을 읽을 수 없으면 같은 이유로 거절한다(fail closed). 힌트는 그 동작을 없애는 새 변경을 쓰라고 말한다.
- 이미 deferred인 변경에 같은 이유, 근거, 승인자로 다시 부르면 쓰지 않고 `unchanged`로 exit 0이다. 그 밖의 호출은 `CHANGE_CONFLICT`(이유 `CLOSED`)이다. 미룸은 만료되지 않는다.
- 변경이나 그 시나리오의 증거, 리뷰, finding 기록이 있어도 거절하지 않는다. 경고 `CHANGE_RECORDS_KEPT`가 종류별 개수를 적고 기록은 그대로 두어 조회된다.
- 보고서는 변경, 상태, 치워지는 시나리오 id, 요구가 빠지는 주제, 남은 기록 개수를 말하고 `meta.next`는 `canon status`와 `canon gate check`를 가리킨다.
- 쓰기이고 지우는 것이 없어 파괴적이지 않다. `--commit`과 `--plan-digest` 쌍이 없다. `--dry-run`은 같은 거절을 하고 쓰지 않는다.

### F2 접기

- `canon change drop <CHANGE_ID>`는 F1의 옵션, 사람 전용 규칙, 거절, 경고, 보고서를 그대로 쓰고 상태가 `dropped`인 기록을 쓴다.
- 미룬 변경을 접는 것은 허용한다. 접은 변경은 어느 상태로도 바꾸지 못하고 다시 부르면 같은 값일 때만 `unchanged`이며 그 밖에는 `CHANGE_CONFLICT`(이유 `CLOSED`)이다. 되살리려면 새 변경을 쓴다.

### F3 요구에서 빠짐

- 닫힌 변경은 주제의 변경 목록에 없는 것처럼 읽힌다. ==**읽는 곳이 모두 같은 판정을 쓴다.**== 읽는 곳은 다음이다.
  - `gate check`: chore의 승인 뒤 증거 요구와 변경 리뷰 요구.
  - `gate check`의 낡은 증거 검사: 치워진 시나리오의 오래된 증거는 `stale-evidence`와 낡음을 풀 수 없는 경우의 셈에 들지 않는다.
  - 증거 판정과 열린 divergence 검사: 치워진 시나리오의 갈라진 판정은 `gate check`를 빨갛게 하지 않는다.
  - 리뷰, 감사, CI 실행 요구.
  - ship 가드와 reopen 가드: 브리프 승인, 구조 blocker, chore.
  - 열린 blocker finding.
  - `evidence add`의 브리프 범위: 닫힌 변경의 승인이 태그 없는 시나리오를 얼리지 않는다.
  - `status`의 `briefs`와 다음 단계, 대시보드의 브리프 목록.
- 치우는 시나리오는 닫힌 변경이 만든 것이다. 조건은 둘이다.
  - 플로 태그가 하나 이상이고 모두 닫힌 변경을 가리킨다.
  - 생성 시각(`.feature`의 `# canon: {"at":…}` 줄, `scenario new`가 쓴다)이 태그가 가리키는 닫힌 변경 하나의 창 안에 든다. 창은 그 변경의 브리프 `.canon/changes/<slug>/brief.md`를 처음 추가한 커밋의 시각부터 가장 늦은 닫는 기록의 시각까지이다. 시작은 커밋된 git 이력에서 읽고 가져오기가 만드는 변경 기록의 시각은 쓰지 않는다. 그 기록은 커밋되지 않고 새 클론에서 브리프 수정 시각으로 다시 만들어지기 때문이다.
  - 생성 시각이 없거나 읽을 수 없으면 치우지 않는다(fail closed). git이 시작을 답하지 못해도(얕은 클론) 치우지 않는다.
- 다른 시나리오는 닫힌 변경의 태그를 무시하고 태그 없는 시나리오처럼 요구받는다. 변경보다 앞서 있던 시나리오에 나중에 붙은 태그와 닫은 뒤에 손으로 붙인 태그가 그렇다. 열린 변경을 하나라도 가리키는 시나리오도 그대로 요구받는다. 무시한 태그는 advisory `closed_change`가 시나리오 id와 이유(`no_creation_time`, `outside_window`, `history_unavailable`)와 함께 적는다.
- 이유가 `history_unavailable`이면 `status`도 같은 줄을 말하고 얕은 클론이라 git 이력이 모자란다고 알린다. 조용히 지나가지 않는다. 변경 단위 요구(chore의 승인 뒤 증거와 변경 리뷰)는 `change_state` 기록에서 오므로 git 이력과 무관하게 빠진다.
- 치워진 시나리오는 증거, 케이스, 리뷰, 감사, CI 실행 요구와 ship의 시나리오 집합, `evidence add --all-scenarios`의 대상에서 빠진다. `.feature` 파일, 시나리오 기록, 증거는 그대로이다.
- 치움은 읽을 때 계산하고 목록을 저장하지 않는다. 그래서 reopen이 지울 것이 없다. 플로 태그가 `.feature`에만 있으므로 그 정보를 게이트 쪽에 주는 길은 구현자가 고른다. 판정은 `gate check`, `status`, `subject move`에서 같아야 한다.
- `status --json`은 주제마다 `setAsideChanges`(변경, 상태, 이유, 근거, 승인자, 시각)와 `setAsideScenarios`를 비어 있으면 생략하고 더한다. `statusVersion`은 4 그대로이다. 사람용 출력은 한 줄씩 말하고 미룬 변경에는 되살리는 명령을 붙인다. `gate check`는 advisory `closed_change`로 같은 것을 적고 종료 값은 그대로이다.
- 시나리오가 모두 치워진 주제는 시나리오가 없는 주제와 같이 ship이 거절된다.

### F4 찾기

- `canon query --kind change_state`는 `--change`, `--status <deferred|dropped|reopened>`, `--history`로 거른다. 현재 상태는 변경마다 가장 늦은 기록이다.
- `canon search <단어>`는 이유와 근거의 글에서 찾고 `--kind change_state`로 좁힌다. 맞은 기록의 이웃에 변경과 주제가 붙는다.

### F5 되살리기

- 명령은 `canon change reopen <CHANGE_ID> --reason <TEXT> --approver <NAME> [--ref <TEXT>…] [--relayed-via <NAME> --actor <ACTOR_ID>] [--dry-run] [--input]`이다. 사람 전용이고 옮김의 규칙은 F1과 같다. deferred인 변경만 되살린다. dropped는 `CHANGE_CONFLICT`(이유 `DROPPED`)이고 힌트는 `canon change new`로 새 변경을 쓰고 그 `Source:`에 접은 변경을 적으라고 말한다. deferred가 아닌 변경은 이유 `NOT_DEFERRED`이다.
- 돌아온 변경은 닫히기 전 자리로 간다: 주제의 변경 목록과 치워졌던 시나리오가 돌아온다. 닫기 전에 승인된 chore는 승인 전 chore가 되고 승인 전 초안은 초안으로 돌아온다. 돌아온 변경은 다시 미루거나 접을 수 있다.
- `Approved:` 줄의 시각이 reopen 기록보다 이르면 그 줄은 승인으로 치지 않는다. `status`는 이를 승인이 낡았다고 말한다. `brief approve`는 앞의 승인과 이름, 답, 코드 트리가 같아도 새 줄을 쓴다(지금은 같으면 쓰지 않고 `unchanged`로 끝난다).

### F6 닫힌 변경에 쓰기

- 닫힌 변경에 `brief approve`는 `CHANGE_CONFLICT`(이유 `CLOSED`)로 거절한다. 여러 id 중 하나가 닫혔으면 지금의 규칙대로 전부 거절하고 아무것도 쓰지 않는다. 대시보드의 승인 요청도 같다. `canon scenario new --flow <F-id> --change <닫힌 변경>`도 같은 거절이다. `.feature` 파일에 닫힌 변경의 태그를 손으로 붙이는 것은 막지 못하지만 F3의 규칙이 그 시나리오를 치우지 않는다.
- `subject unadopt`의 거절(`APPROVED_BRIEF`, `FLOW_TAGGED`)은 그대로이다. `APPROVED_BRIEF`의 힌트와 코드 레지스트리의 설명은 승인된 변경을 빼려면 사람의 결정으로 `canon change defer`나 `canon change drop`을 쓰라고 바뀐다.

### F7 첫 사용, `canon-0-41-ci-nextest`

- 지금 main에서 `canon change defer canon-0-41-ci-nextest …`는 `CHANGE_NOT_FOUND`이다. 디렉터리와 기록이 없다. 힌트는 변경을 먼저 들여오라고 말한다.
- 변경이 트리에 오는 순서는 이렇다.
  - `plan/0.41`이나 `wip-0.41/ci-nextest`에서 승인된 브리프 `.canon/changes/canon-0-41-ci-nextest/brief.md`를 가져온다(승인 줄은 커밋 29b2a90d0). 그 브리프는 두 가지에 있고 main에는 없다.
  - `canon ingest plans`로 기록을 만들고 0.41 계획대로 `canon subject adopt gate-trust-spine --change canon-0-41-ci-nextest`로 잇는다. 이 순간 shipped 주제 `gate-trust-spine`이 승인된 chore의 변경 리뷰를 요구해 `gate check`가 빨개진다.
  - 같은 작업 트리에서 소유자의 명시적 답을 옮겨 부른다: `canon change defer canon-0-41-ci-nextest --reason "<소유자의 말>" --ref <PR #147의 URL> --ref <실행 43498921480의 URL> --ref /Volumes/MoeLive/storage/journey-temp/projects/canon/2026-11-14-0.41-plan-review/ci-nextest-measurement.md --approver <소유자> --relayed-via <도구> --actor <id>`.
  - 브리프, 연결, `change_state` 기록을 한 커밋으로 올리고 코드(CI 파일과 `.config/nextest.toml`)는 올리지 않는다. 코드는 `wip-0.41/ci-nextest`에 남는다. 이 저장소의 `.gitignore`는 `.canon/ledger/*`를 무시하고 종류마다 다시 포함하므로 이 변경이 `!.canon/ledger/kind=change_state/`를 더한다(`_staging`은 그대로 무시). 이 줄이 없으면 기록이 경고 없이 커밋에서 빠지고 CI와 다른 클론은 그 변경을 계속 요구한다.
- 측정 문서는 임시 NAS 경로(`journey-temp`)에 있어 지워질 수 있다. 같은 측정을 말하는 PR #147과 실행의 URL이 그 뒤에도 찾을 수 있는 근거이다.
- 그 뒤 `gate check`는 그 변경 때문에 빨개지지 않고 `status`가 deferred와 이유를 보여 준다. 소유자가 세 갈래 중 하나를 고르면 `canon change reopen`으로 되살리고 브리프를 새로 승인받는다. 세 갈래는 범위를 줄여 출하, nextest를 비슷한 시간으로 출하, 시험의 CPU를 먼저 줄임이다. 쓰지 않기로 하면 `canon change drop`이다. 이 일은 이 변경이 구현된 뒤에 하고 이 브리프의 승인은 소유자의 그 답을 대신하지 않는다.
- 시험은 같은 모양을 임시 저장소로 만든다: shipped 주제, 승인된 chore, 변경 리뷰 없음 → `gate check`가 `unreviewed-promotion`으로 빨강 → 미룸 뒤 초록 → reopen 뒤 다시 빨강.
- 이 저장소의 CI에서 `gate check`는 `test.yml`의 `offline` 잡 한 곳에서만 돈다. 그 잡의 체크아웃은 `fetch-depth: 0`(전체 이력)이라 창의 시작과 이 변경의 나감 판정(F1)이 읽힌다. 다른 잡은 얕게 받지만 `gate check`를 돌리지 않는다. 그래서 CI 파일은 이 변경이 고치지 않는다. 그 잡의 `fetch-depth`를 줄이면 위의 `history_unavailable`이 CI에 나타난다.
- `canon-0-41-ci-nextest`는 시나리오가 없는 chore라 이 변경의 첫 사용은 치움의 창에 기대지 않는다.

### F8 안내와 변경 기록

- 안내가 사람 전용 목록(`brief approve`, `policy approve`, `divergence accept`과 `change drop`, `change defer`, `change reopen`), 닫는 절차(사람의 답을 묻고 `--relayed-via`로 옮김), `status`의 `setAside*`를 말한다. 대상은 `canon skills show`의 `role-planner`, `role-orchestrator`, `canon-subject`, `--help`, 생성 명세 `docs/cli/canon.cli.json`, conformance 기대 출력이다.
- `canon query`와 `canon search`의 `--kind` 값에 `change_state`가 더해지고 생성 명세가 다시 만들어진다. `canon init`이 쓰는 `canon.yaml` 뼈대에 `change_state: local` 줄이 들어가고(`init`이 열셋 종류를 모두 한 줄씩 쓴다) 옛 저장소에는 `canon migrate`가 더한다(F9). 사람 전용 leaf 셋을 말하는 시나리오(`skills.install.21`)와 시험은 여섯으로 고친다.
- CHANGELOG의 Added에 새 leaf 셋, 기록 종류, `CHANGE_CONFLICT`, `CHANGE_RECORDS_KEPT`, advisory `closed_change`와 `canon migrate`의 새 단계를 싣는다. Changed에는 `canon.yaml`의 `routing:`이 한 줄을 더 요구하고 `canon migrate`가 `change_state: local`을 더한다는 것, `APPROVED_BRIEF` 힌트, `CHANGE_ARGUMENT_INVALID`의 설명과 힌트가 네 leaf를 말한다는 것을 싣는다.
- 호출자 노트:
  - 닫힌 변경은 요구에서 빠진다.
  - 줄이 없는 저장소는 닫는 명령 전에 `canon migrate`를 돌린다.
  - `canon migrate`나 0.42의 `canon init`이 `change_state`를 더한 저장소를 canon 0.41 이하는 `CONFIG_INVALID`로 거절한다. 되돌리려면 `routing:` 아래 `change_state:` 줄을 손으로 지운다. `canon migrate --revert`는 그 줄을 더한 `canon migrate --commit` 바로 뒤에만 파일을 되돌린다(0.42 `canon init`이 만든 저장소에는 백업이 없고, 그 뒤 저장소가 바뀌면 되돌리기는 거절된다). 0.41은 닫힘을 읽지 않으므로 닫힌 변경이 다시 요구된다.
  - `.gitignore`에 `.canon/ledger/*`를 직접 쓴 저장소는 `!.canon/ledger/kind=change_state/`를 손으로 더한다. `canon init`이 쓰는 `.gitignore`는 원장을 통째로 무시하지 않아 그대로 커밋된다.
- 이 저장소의 `canon.yaml` 74개 수정은 0.42 바이너리를 설치하는 것과 같은 시점에 들어간다. 0.41.0이 PATH에 있는 세션은 그 줄 때문에 `gate check`, `evidence add`, `review add`가 거절된다.

### F9 migrate

- `canon migrate`의 미리보기는 `routing:`에 `change_state` 줄이 없는 `canon.yaml`에 더할 줄 `change_state: local`과 자리를 줄 번호로 말한다. 자리는 `routing:` 절의 마지막 항목 뒤이고 들여쓰기는 앞 항목과 같다. `--commit --plan-digest`가 그 줄만 더한다. 백업과 `--revert`, 결과 문서 동등 검사(옛 문서에 그 키만 더한 것과 같다)는 0.39가 `session`·`event` 키를 지운 단계와 같은 길이다.
- 이 단계의 확정은 0.39 단계처럼 모든 역할에 열려 있고 새 사람 전용 규칙을 더하지 않는다. 같은 실행에서 다른 단계가 `Approved:` 줄이나 리뷰 고정을 다시 묶으면 그 확정은 지금처럼 사람의 몫이다.
- 값은 `local`이다. 변경 기록은 커밋되고 diff로 읽히며 게이트가 읽는 종류이기 때문이다. 다른 값을 원하는 사용자는 적용 뒤 손으로 고친다.
- 줄이 이미 있으면(값이 무엇이든) 이 단계는 건너뛰고 값을 바꾸지 않는다. 적용했으면 다음 `migrate`의 계획이 비어 있다. 저장된 행과 파일은 건드리지 않는다.
- `routing:` 절이 없는 `canon.yaml`은 이 단계를 건너뛰고 아무것도 더하지 않는다. 어떤 종류도 라우팅하지 않는 저장소라 줄이 채울 것이 없다. 닫는 명령은 `CONFIG_INVALID`(`TIERS`)와 줄을 더하라는 힌트로 거절한다.
- 줄로 더할 수 없는 형태(앵커, 흐름 매핑)만 파일을 그대로 두고 `CONFIG_INVALID`(이유 `CANON_YAML`)로 거절한다. 메시지가 손으로 더할 한 줄을 말하고 힌트가 고친 뒤 `canon migrate`를 다시 돌리라고 말한다. 0.39 단계가 안전하지 않은 형태에 내는 거절과 같은 코드와 이유이다. 이 거절은 그 실행의 다른 migrate 단계도 막는다.
- 닫는 명령(`change drop|defer|reopen`)의 `CONFIG_INVALID`(`TIERS`) 거절 힌트는 줄을 더하는 `canon migrate`를 말한다. 읽기 명령에는 경고를 더하지 않는다.
- 새 종류가 생길 때마다 한 단계를 두는 길을 고른다. `routing.default`도 코드의 종류별 기본도 두지 않는다(질문 절의 결정 참조).

## Acceptance

- F1:
  - 승인된 chore에 사람이 `canon change defer`를 부르면 `change_state` 기록 한 건이 이유, 근거, 승인자, 시각과 함께 남고 `unchanged`가 아니다. 에이전트가 `--relayed-via ask --actor a --approver 이름`으로 부르면 기록에 옮긴 역할, actor, 도구가 남는다. 증거나 리뷰가 있는 변경은 경고 `CHANGE_RECORDS_KEPT`와 함께 성공하고 그 기록이 그대로이다. 같은 호출을 다시 하면 쓰지 않는다.
  - 실패 경로: 에이전트 역할이 옮김 없이 부르면 `ACTOR_DENIED`이고 아무것도 쓰지 않는다. `approval: { relay: deny }`, `--actor` 없는 옮김, 역할 없는 `--relayed-via`, 빈 승인자, 빈 이유, `--ref` 없음이 각각 거절되고 쓰지 않는다. 없는 변경은 `CHANGE_NOT_FOUND`, 이미 나간 변경은 `CHANGE_CONFLICT`(`SHIPPED`), 다른 이유로 다시 부른 닫힌 변경은 `CHANGE_CONFLICT`(`CLOSED`)이고 모두 쓰지 않는다.
  - 실패 경로, 나감의 판정: 주제가 shipped로 옮겨졌다가 다시 열린 뒤 그 변경을 다시 승인한다. 그 변경에 defer나 drop을 부르면 `CHANGE_CONFLICT`(`SHIPPED`)이고 쓰지 않는다. `subject adopt`로 shipped 주제에 들인 변경과 shipped일 때 승인 전 초안이던 변경은 나간 변경이 아니어서 닫힌다. 이동 이력을 읽을 수 없으면 거절한다.
- F2:
  - `canon change drop`이 dropped를 기록하고, deferred였던 변경도 drop이 된다. 승인 전 초안도 닫힌다.
  - 실패 경로: dropped 변경에 defer나 drop을 다시 다른 값으로 부르면 `CHANGE_CONFLICT`(`CLOSED`)이다. 사람 전용 거절은 F1과 같고 `--dry-run`도 같은 코드로 거절하며 쓰지 않는다.
- F3:
  - shipped 주제의 승인된 chore가 변경 리뷰 없이 `gate check`를 빨갛게 하고 있다가 미룸 뒤 초록이 된다. ship 가드, reopen 가드, `status`의 다음 단계가 그 변경을 더 요구하지 않는다.
  - 닫힌 변경이 만든 시나리오(생성 시각이 창 안이고 플로 태그가 모두 닫힌 변경을 가리킴)는 치워진다. `status --json`의 `setAsideChanges`와 `setAsideScenarios`가 이를 말한다. `gate check`의 advisory `closed_change`도 같다. 열린 blocker finding은 기록으로 남고 막지 않는다.
  - 치워진 시나리오의 증거가 `max_commits_behind`를 넘어도 `gate check`는 빨갛지 않다. 그 시나리오의 열린 divergence나 갈라진 판정도 같다.
  - 실패 경로: 같은 주제의 다른 변경이 내는 요구가 닫힌 변경 때문에 사라지면 시험이 실패한다. 닫힌 변경과 열린 변경의 태그를 함께 가진 시나리오와 태그 없는 시나리오는 계속 요구받는다. 시나리오가 모두 치워진 주제는 ship이 거절된다. `status`, `gate check`, `subject move`가 같은 시나리오를 치웠는지 한 시험이 대조한다.
  - 실패 경로, 태그의 무시: 변경보다 앞서 있다가 나중에 닫힌 변경의 태그만 받은 시나리오는 계속 요구받는다. 닫은 뒤에 만들어 손으로 그 태그를 붙인 시나리오도 같다. 생성 시각이 없는 시나리오도 같다. `closed_change`가 무시한 태그를 적는다.
  - 닫힌 변경이 만든 시나리오가 있는 변경을 미룬 뒤 새 클론(전체 이력)에서 `inventory sync`와 `gate check`를 돌리면 초록이고 `setAsideScenarios`가 원래 작업 트리와 같은 집합을 낸다.
  - 실패 경로, 얕은 클론: 같은 저장소를 `--depth 1`로 받으면 아무것도 치우지 않고 `closed_change`와 `status`가 이유 `history_unavailable`을 말한다. 그 시나리오의 요구는 그대로이고 변경 단위 요구는 빠져 있다.
- F4:
  - `canon query --kind change_state --change <slug>`가 현재 상태를, `--history`가 reopen 앞의 기록까지 준다. `canon search <이유의 단어> --kind change_state`가 그 기록과 이웃 변경을 준다.
  - 실패 경로: 알 수 없는 `--status` 값은 `USAGE`이고 `expected`가 상태 셋이다. 닫힌 적 없는 변경은 빈 목록이고 오류가 아니다. `canon.yaml`에 `change_state` 줄이 없으면 `change drop|defer|reopen`이 `CONFIG_INVALID`(`TIERS`)로, `canon query --kind change_state`가 `QUERY_FAILED`로 거절하고 메시지가 그 종류를 말하며 `CONFIG_INVALID`의 힌트가 `canon migrate`를 말한다(F9).
- F5:
  - `canon change reopen`이 reopened를 기록하고 변경과 치워졌던 시나리오가 요구로 돌아온다. 앞의 승인 줄은 승인으로 읽히지 않고 같은 이름과 답의 `brief approve`가 새 줄을 쓴다.
  - 실패 경로: dropped 변경은 `CHANGE_CONFLICT`(`DROPPED`), 닫히지 않은 변경은 `NOT_DEFERRED`이다. 에이전트 역할의 옮김 없는 reopen은 `ACTOR_DENIED`이고 `approval: { relay: deny }`인 저장소의 옮김, `--actor` 없는 옮김, 역할 없는 `--relayed-via`도 거절되며 모두 쓰지 않는다. reopen 뒤 새 승인 없이 ship하면 승인 요구로 거절된다.
- F6:
  - `unadopt`의 `APPROVED_BRIEF`와 `FLOW_TAGGED` 거절이 그대로이고 힌트가 `change defer`와 `change drop`을 말한다.
  - 실패 경로: 닫힌 변경에 `brief approve`(여러 id 중 하나만 닫혀도), 대시보드 승인, `scenario new --flow --change`가 `CHANGE_CONFLICT`(`CLOSED`)이고 쓰지 않는다.
- F7:
  - 시험이 F7의 모양으로 임시 저장소를 만든다. 변경이 없는 트리에서는 `CHANGE_NOT_FOUND`, 들여온 뒤 미룸 전에는 `gate check`가 빨강, 미룸 뒤 초록, reopen 뒤 빨강이다. 미룸 뒤 새 클론에서도 `gate check`가 초록이고 `canon query --kind change_state --history`가 기록을 준다. 이 저장소의 `.gitignore`에 대해 `git check-ignore`가 `change_state` 기록 경로와 맞지 않고 `canon init`이 만든 저장소도 같다.
  - 실패 경로: 변경이 없는 트리의 호출은 `CHANGE_NOT_FOUND`이고 아무것도 쓰지 않는다. 사람의 답 없이 에이전트가 부르면 `ACTOR_DENIED`이다. `.canon/ledger/*`를 무시하되 `change_state`를 다시 포함하지 않는 `.gitignore`에서는 새 클론이 그 변경을 다시 요구하므로 호출자 노트의 한 줄이 그 이유이다.
- F8:
  - 새 leaf 셋의 안내, 도움말, 생성 명세, conformance가 새 모양을 말하고 사람 전용 시나리오가 여섯 leaf를 말한다. 안내의 명령 줄은 스키마에 맞게 풀린다. `canon init`의 뼈대가 `change_state: local`을 쓴다.
  - 실패 경로: 안내의 명령 줄이 스키마에 맞지 않거나 사람 전용 leaf 목록이 여섯이 아니면 시험이 실패한다. CHANGELOG에 호출자 노트가 없으면 릴리스 확인 시험이 실패한다.
- F9:
  - 줄이 없는 `canon.yaml`로 `change defer`가 `CONFIG_INVALID`(`TIERS`)이고 힌트가 `canon migrate`를 말한다. `canon migrate`의 미리보기가 줄 번호와 `change_state: local`을 말하고 `--commit --plan-digest`가 그 줄만 더한다. 결과는 옛 문서에 그 키만 더한 것과 같고 `change defer`가 돌며 다음 `migrate`의 계획이 비어 있고 `--revert`가 원래 파일을 되살린다. 이미 줄이 있는 저장소(값이 `cold`여도)는 계획이 비어 있고 값이 그대로이다. 역할을 선언한 에이전트도 사람의 답 없이 이 확정을 돌릴 수 있다.
  - 실패 경로: 앵커나 흐름 매핑 형태의 `routing:`이면 `CONFIG_INVALID`(`CANON_YAML`)로 거절하고 파일을 그대로 두며 메시지가 손으로 더할 줄을 말한다. `routing:` 절이 없는 `canon.yaml`(기존 시험 `authoring.migrate.02`)은 파일이 바이트까지 그대로이고 다음 `migrate`가 할 일이 없다고 말한다. 미리보기 뒤 `canon.yaml`이 바뀐 채 낡은 다이제스트로 확정하면 `PLAN_STALE`이고 파일이 그대로이다.
  - 이 저장소의 추적되는 `canon.yaml` 90개 가운데 `routing:`이 없는 16개는 단계가 건너뛴다. `routing:`이 있는 74개는 이 변경이 줄을 더해 두므로 계획이 비어 있다. 부분 라우팅을 인라인으로 쓰는 시험의 `canon.yaml`은 새 단계가 생기므로 그 시험의 기대 출력을 이 변경이 고친다.
- 변경 뒤 새 leaf는 `--input`, `--dry-run`, 쓰기 불가, 잠금, 반복 쓰기에 걸린 일반 시험을 지난다.
- ==**변경 뒤 `cargo test --workspace`, `just atip-deep`, `canon gate check`가 통과한다.**==

## Questions and assumptions

- 결정: 이 변경은 feature이다. 새 명령, 새 기록 종류, 새 오류 코드와 경고, `status`와 `gate check` 출력이 호출자가 읽는 출력이다.
- 결정, 소유자의 지시: 닫는 길은 `canon change drop|defer <slug> --reason … --ref … --approver …`이고 사람 전용이며 `brief approve`처럼 옮긴다. 미룸은 `change reopen`으로 되살리고 새 승인이 필요하며, 중단은 끝이고 되살리려면 새 변경이다. 이미 나간 변경은 거절, 증거나 리뷰가 있으면 경고하고 남기며, 사람이 아닌 역할은 거절한다. 첫 사용은 `canon-0-41-ci-nextest`의 미룸이다(2026-11-14). 이 브리프의 승인은 그와 별개로 받는다.
- 결정: 결정을 기존 `change` 기록이나 브리프의 한 줄에 두지 않고 기록 종류 `change_state`를 새로 둔다. `change` 기록은 가져오기가 브리프 수정 시각으로 덮어쓴다. 브리프의 `Approved:`처럼 줄로 쓰면 `canon query`, 이력, 검색의 이웃 그래프가 없다. 기록 종류가 열둘에서 열셋이 되면 `canon.yaml`에 한 줄이 필요하고 아래 결정이 그 줄을 다룬다.
- 결정: 닫는 leaf는 파괴적이지 않다. 기록을 덧붙일 뿐 아무것도 지우지 않아 미리보기와 확정의 쌍을 두지 않는다. 접은 변경을 되돌리지 못하는 것은 정책이고 데이터 삭제가 아니다.
- 결정: 치움은 읽을 때 계산한다. 목록을 저장하면 태그나 상태가 바뀔 때 어긋난다. 한 변경이 닫히면 그 변경의 승인, 증거, 리뷰는 그대로 있고 판정에서만 빠진다.
- 결정: `--ref`는 하나 이상 필수이다. 나중에 찾을 수 있게 한다는 것이 이 길의 이유이고 근거 없는 결정은 다시 물어야 한다. 근거로 링크가 없으면 대화의 날짜를 쓴다. reopen의 `--ref`는 선택이다.
- 결정: reopen이라는 이름은 주제를 다시 여는 `subject move --to verifying`과 같은 말이다. 소유자가 고른 이름이고 도움말이 대상(변경)을 말해 구분한다.
- 결정: 닫힌 변경의 코드는 이 길이 건드리지 않는다. 가지와 파일은 사람이 정리한다.
- 가정: 읽는 곳이 모두 따르도록 치움은 한 번 계산해 `gate check`, `status`, `subject move`와 낡은 증거, 판정, divergence 검사에 같은 집합을 넘긴다. 이 목록에 없는 시나리오별 읽는 곳이 있으면 F3의 목록에 더해 같은 시험을 건다.
- 가정: 창의 시작은 브리프를 처음 추가한 커밋의 시각이다. 시나리오는 승인 뒤에 쓰므로 그 커밋보다 늦다. 브리프가 아직 커밋되지 않은 변경은 닫아도 치우는 시나리오가 없다. 창 안에서 만든 시나리오에 손으로 닫힌 변경의 태그를 붙이는 것까지는 막지 못한다.
- 가정: 이 저장소 밖의 사용자는 `canon migrate`를 한 번 돌려 `canon.yaml`에 한 줄을 더한다. 줄이 있는 저장소는 canon 0.41 이하가 거절하므로 내리려면 그 줄을 손으로 지운다(`--revert`는 migrate 직후에만 된다).
- 결정(소유자): 라우팅은 종류마다 명시한다. `routing.default`도 코드의 기본값도 두지 않는다. `canon init`이 `change_state: local`을 쓰고(`init`이 모든 종류를 돈다) 줄이 없는 옛 저장소는 `canon migrate`가 더한다(F9). 이유: 새 종류는 드물다(`finding`이 v0.1 이후 유일하다). `routing.default`도 기존 저장소마다 `migrate`가 한 번은 필요하다(저장소 기록 저장 검토 `storage-review.md` §4, `/Volumes/MoeLive/storage/journey-temp/projects/canon/2026-11-14-0.41-plan-review/`). 줄이 없는 저장소에서 `status`와 `gate check`는 닫힌 변경을 읽지 못한 채 지나가지만 그 저장소는 닫는 기록을 쓸 수도 없고 거절 힌트가 `canon migrate`를 말한다. 이 저장소의 `routing:`이 있는 `canon.yaml` 74개는 이 변경에서 줄을 더한다.
- 결정: `canon migrate`의 이 단계는 0.39 단계처럼 모든 역할에 열려 있다. 한 줄을 더하고 백업과 `--revert`가 있어 파괴적이지 않다.
- 결정: `canon migrate`는 `.gitignore`의 `.canon/ledger/*`와 다시 포함 줄을 고치지 않고 `canon init`의 `.gitignore` 줄에도 `change_state`를 더하지 않는다. `canon init`은 원장을 통째로 무시하지 않아 `kind=change_state`가 그대로 커밋된다. 원장을 직접 무시한 저장소는 호출자 노트가 말하는 한 줄을 손으로 더한다.
- 결정(소유자): `change reopen`도 사람 전용이다. 미룸이 사람의 결정이므로 에이전트가 풀 수 없다. 옮김의 규칙은 `defer`, `drop`과 같고 사람이 아닌 역할은 `ACTOR_DENIED`이다(F5의 실패 경로). 에이전트가 풀 수 있게 하면 승인이 새로 필요하다는 보호가 유일한 방어가 된다.

## Out of scope

- 닫힌 변경의 코드, 가지, 파일을 되돌리거나 지우는 일. `canon-0-41-ci-nextest`의 코드와 세 갈래 중 무엇을 고를지는 소유자의 몫이다.
- 이미 나간 변경을 닫는 길(그 동작을 없애는 새 변경을 쓴다).
- 시나리오 하나를 따로 치우는 길(`divergence`가 있다)과 주제를 통째로 접는 길(`subject move --to retired`가 있다).
- 미룸의 만료일, 대시보드에서 닫는 화면, 플로 태그를 시나리오 기록으로 옮기는 일.
- `status`의 `statusVersion`을 올리는 일과 보고서(`report`)의 새 패널.

Approved: journey 2026-11-16T12:01:26Z — "오너가 채팅에서 오케스트레이터에게 승인을 위임함 (2026-11-16, '일단 한번에 가자 기본안대로'). 구현 리뷰 1라운드 finding 1_3(0.41로 내리는 안내가 실제로 안 되는 경로를 적음)에 따라 F8 호출자 노트와 가정 한 줄을 실제 동작(줄을 손으로 지움, --revert는 migrate 직후만)으로 고침. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 7a8f9d3a4f70c7ad7e85c8a4a675e0bd4275d444) (brief sha256:2e7da23123988a24656d359232de53f3b575a6658c10fbb62478b0d13457ff60)
