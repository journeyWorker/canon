---
waive:
  - rule: "prose.length"
    section: "intent"
    reason: "서로 다른 문제 셋을 하나씩 적은 목록이라 줄이면 현재 동작이 빠진다"
  - rule: "prose.length"
    section: "brief"
    reason: "서로 다른 세 문제의 네 흐름을 각각 실패 경로와 함께 다루는 한 변경이라 합치면 현재 동작과 거절 조건이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "세 문제의 네 흐름 각각의 규칙과 거절 조건을 적은 목록이라 줄이면 판정 규칙이 빠진다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "네 흐름 각각의 통과 경로와 실패 경로를 한 줄씩 적은 목록이라 줄이면 거절 조건이 빠진다"
---

# 0.31 승인과 adopt 흐름 개선

## Intent

canon으로 canon 0.30을 만들며 세 가지가 걸렸다.

- `canon brief approve`는 change 하나만 받는다. brief 여섯 개를 승인하려고 사람이 같은 명령을 여섯 번 쳤고 셸 반복문은 깨졌다. 대시보드 briefs 페이지도 brief마다 이름과 답을 따로 적는다.
- reopen과 ship은 adopt된 change를 가장 새 것부터 거슬러 올라가다 처음 만나는, 승인 줄이 있거나 시나리오가 태그한 change 하나의 brief로만 판정한다. 그보다 오래된 change는 승인되지 않았거나 고쳐졌어도 보지 않는다. 같은 subject의 판정이 adopt 순서와 승인 순서에 따라 달라진다.
- hot 경로에 디렉터리가 있으면 `canon report`는 hot 레코드 없이 exit 0으로 끝난다. `canon query`는 같은 경우를 거절한다. 깨진 파일은 report도 이미 거절한다.

Source: GitHub 이슈 #114와 0.30 계획 리뷰.

0.31 순서: CI → 승인과 adopt → 버그 수정 → brief 포맷 → CLI id → 검색. 여섯 brief는 함께 승인받는다. 승인한 brief를 고치면 그 brief를 adopt한 모든 subject가 재승인 전까지 막힌다.

## Desired outcome

- ==**`canon brief approve` 한 번이 change 여럿을 같은 이름과 답으로 승인한다.**== 하나라도 거절되면 아무것도 쓰지 않는다.
- 대시보드 briefs 페이지에서 brief를 여럿 골라 한 번에 승인한다.
- subject의 reopen, ship, status는 관할 change를 모두 판정한다. 관할 change는 시나리오가 태그한 change, 승인 줄이 있는 change, chore다. 승인된 적 없고 태그되지 않은 feature change는 막지 않는다.
- 시나리오의 증거 기록은 태그된 시나리오이면 그 태그한 change, 태그 없는 시나리오이면 승인 줄이 있는 관할 change를 따른다.
- hot 경로에 디렉터리 같은 SQLite가 아닌 것이 있으면 report도 거절한다.

## User flow

1. F1 사람이 승인 대기 중인 brief 여럿을 본다 → `canon brief approve`에 change id를 모두 적어 한 번 실행한다 → ==**하나라도 거절되면 하나도 승인되지 않는다.**==
2. F2 사람이 briefs 페이지에서 brief를 여럿 고르고 이름과 답을 한 번 적는다 → 승인 버튼을 한 번 누른다 → F1과 같은 규칙으로 brief마다 결과가 나온다.
3. F3 에이전트가 change를 여럿 adopt한 subject를 reopen, ship하거나 증거를 기록하고 status를 본다 → canon이 관할 change마다 brief를 판정한다 → 막는 brief가 모두 이름 붙고 대기 초안은 막지 않는다.
4. F4 사용자가 hot 경로에 디렉터리가 있는 repo에서 report를 만들거나 대시보드를 연다 → canon이 hot 저장소를 열려 한다 → report가 hot 경로를 이름 붙여 거절한다.

## Step detail

### F1 여러 brief의 한 번 승인

- change id를 하나 이상 받는다. 생략하면 지금처럼 live change 하나를 읽고, 같은 id를 여러 번 적으면 한 번으로 센다.
- 승인자 이름과 답은 하나이고 모든 brief와 정책 승인에 같이 적힌다. 한 호출은 시각을 하나만 쓴다.
- 에이전트 role은 id가 몇 개든 사람 전용 거절(HUMAN_ONLY)을 받는다. 플래그로 풀 수 없고 아무것도 쓰지 않는다. hint는 모든 id를 담은 호출 한 줄을 사람에게 알려 준다.
- 쓰기 전에 모든 id를 검사한다. brief가 있고 읽히는가, blocker가 없는가, 이름과 답이 한 줄인가. 본문이 열린 코드 펜스나 주석으로 끝나지 않아 승인 줄을 붙일 수 있는가. 하나라도 어기면 exit 2의 BRIEF_APPROVE_REFUSED 한 오류로 거절하고 거절된 id와 이유를 모두 이름 붙인다. 항목마다 자기 코드를 담고 아무것도 쓰지 않는다.
- 통과하면 입력 순서대로 쓰며 내용은 id 하나를 승인할 때와 같다. 쓰는 중 디스크 오류가 나면 exit 1이다. 이미 쓴 id는 승인된 채 남고 나머지는 건너뛰므로 전부 아니면 전무는 거절에만 해당한다. 같은 호출을 다시 실행하면 승인된 id는 변경 없음이다.
- id가 하나면 출력은 지금과 같다. 여럿이면 JSON은 id마다 항목(written, unchanged, failed, skipped)을 담는다. `--input` 배열 실행은 지금처럼 첫 실패에서 멈추고 앞선 승인은 남는다. 원소마다 이름과 답이 다를 수 있어서 둘을 합치지 않는다.

### F2 대시보드에서 여러 brief 승인

- 승인할 수 있는 brief마다 선택 칸이 있고 blocker가 있는 brief는 고를 수 없다. 고른 개수가 보인다. 이름과 답, 버튼은 하나다.
- 요청은 이름과 답을 한 번, brief마다 change id와 페이지가 읽은 파일의 sha256을 담는다. brief 하나만 담은 지금 요청도 계속 받는다.
- 고른 brief 하나라도 그 뒤 바뀌었거나 blocker가 생겼으면 F1처럼 모두 거절하고 아무것도 쓰지 않는다. 페이지는 거절된 brief를 이름 붙이고 다시 읽으라고 안내한다.
- 결과 기록, 정책 승인, 목록에서 빠지는 동작은 단일 승인과 같다.

### F3 관할 change로 판정

- 관할 change는 subject가 adopt한 change 가운데 셋 중 하나다: 시나리오가 `@flow:<change>.*`로 태그한 것, brief에 승인 줄이 있는 것(승인 뒤 고친 것 포함), chore. chore는 시나리오가 없어 태그할 수 없으므로 adopt하는 순간부터 관할이다.
- 한 번도 승인되지 않았고 태그되지 않은 feature change는 대기 초안이고 관할이 아니다. 태그를 지워도 승인 줄이 있으면 계속 관할이다.
- ==**reopen과 ship은 subject의 관할 change가 모두 현재 승인 상태일 때만 통과한다.**== 미승인과 승인 뒤 고침은 다른 이유로 이름이 나온다. 판정은 adopt 순서와 승인 순서에 따르지 않는다. ship의 brief 구조 blocker 검사도 관할 change 모두에 돈다.
- 증거 기록은 시나리오에 따라 다르다. 태그된 시나리오는 자기가 태그한 change에만 답한다. 태그 없는 시나리오는 승인 줄이 있는 관할 change(chore 포함) 모두에 답한다. 미승인 chore는 reopen과 ship만 막고 증거는 막지 않는다.
- 승인 줄이 있는 관할 change가 없으면 태그 없는 시나리오는 지금 규칙을 따른다. 가장 새 adopt feature change의 승인을 기다리고, 첫 adopt 전에 증거가 있던 시나리오는 계속 재검증한다. 관할 change가 하나도 없는 subject의 reopen과 ship도 같다.
- status JSON은 지금의 brief 필드 하나 대신 briefs 목록을 낸다. 항목은 관할 change마다 하나다. 항목은 change, brief 경로, 상태, 관할 이유, 검사 결과 개수를 담는다. 상태는 approved, unapproved, changed이고 이유는 tagged, approved, chore다. pendingDrafts는 대기 초안만 담고 flow는 태그된 관할 change의 단계를 모두 담는다.
- 막는 brief가 여럿이면 hint는 모든 id를 담은 `canon brief approve <id>…` 한 줄을 알려 준다. reopen 거절은 `--override-reason`으로 넘길 수 없고 ship의 brief 거절은 넘길 수 있다. chore의 승인 뒤 검사도 그대로다.
- 새 규칙으로 고칠 기존 시나리오는 다음 동작을 말하는 것들이다.
  - 대기 초안이 reopen, 증거, 리뷰, ship을 얼리지 않는다.
  - 태그된 미승인 초안은 더 새 초안 아래서도 막는다.
  - ship과 reopen이 가장 새 adopt change의 승인 줄을 본다.
  - status가 가장 새 change의 brief를 읽는다.
  - 미승인 brief가 증거를 거절하고 승인하면 풀린다.
  - 한 change를 승인하는 호출 하나가 brief와 정책을 함께 승인한다.

### F4 hot 경로 거절

- 경로가 없으면 hot 레코드가 없는 것으로 읽고 파일을 만들지 않는다. 지금과 같다.
- 경로에 무언가 있는데 SQLite 파일로 열리지 않으면 report는 exit 1로 거절하고 오류가 그 경로를 이름 붙인다. report를 만드는 build, check, snapshot 모두에 적용한다.
- 대시보드는 같은 repo에서 뜨되 snapshot을 쓸 수 없음으로 표시하고 그 사유가 경로를 이름 붙인다. briefs 페이지는 동작한다.

## Acceptance

- F1: ==**세 change를 한 번에 승인하면 세 brief와 정책에 같은 승인자와 답이 적히고, 하나씩 승인했을 때와 내용이 같다.**== 같은 호출을 다시 실행하면 변경 없음이다. 실패 경로: blocker가 있는 id, 없는 id, 열린 코드 펜스로 끝나는 brief가 섞인다. exit 2이고 오류가 셋을 모두 이름 붙이며 어느 brief도 바뀌지 않는다. planner role의 호출은 사람 전용 거절이고 hint가 모든 id를 담는다.
- F2: brief 셋을 고르고 승인하면 셋이 목록에서 빠지고 CLI로 승인한 복사본과 내용이 같다. 실패 경로: 고른 brief 하나를 페이지를 읽은 뒤 고치면 409로 거절된다. 셋 다 그대로이고 그 brief가 이름 붙는다.
- F3: 태그된 change가 둘이고 하나만 승인된 subject는 reopen과 ship이 미승인 change를 이름 붙여 거절한다. 둘을 어느 순서로 승인해도 통과한다. 실패 경로 셋:
  - 승인된 둘 중 하나를 고치면 그 change를 이름 붙여 거절한다. 태그를 지워도 같다.
  - 시나리오가 태그하지 않은 승인된 feature brief를 고쳐도 ship을 막고 그 change를 이름 붙인다.
  - 승인 뒤 고친 chore와 미승인 chore는 reopen과 ship을 막는다.
- F3 통과 경로: 승인된 적 없고 태그되지 않은 feature brief는 reopen, ship, 증거 기록을 막지 않는다. status는 그것을 pendingDrafts에 올린다. status JSON의 briefs가 관할 change마다 상태와 이유를 낸다. 관할 change 둘이 다 미승인이면 거절이 둘을 모두 이름 붙이고 hint는 id 둘을 담은 승인 호출 한 줄이다.
- F3 증거: 태그된 시나리오의 증거는 자기가 태그한 change의 승인만 기다리고, 다른 change의 승인 뒤 고침은 막지 않는다. 태그를 지운 시나리오는 태그 없는 것으로 읽는다. 태그 없는 시나리오의 증거는 승인 줄이 있는 change가 고쳐졌으면 거절된다(BRIEF_CHANGED_SINCE_APPROVAL). 미승인 chore는 막지 않는다.
- F3 문서: status 보고서의 스키마 버전이 올라간다. README의 Upgrading에 brief 필드가 briefs 목록으로 바뀐 것이 적힌다. 같은 설명이 든 스킬, README, status 도움말, 스키마 문서가 새 규칙과 맞는다.
- F4: hot 경로에 디렉터리가 있는 fixture에서 report build, check, snapshot이 exit 1이다. 오류는 그 경로를 이름 붙이고 깨진 파일도 같다. 대시보드는 snapshot 불가 사유에 그 경로를 담아 뜬다. 경로가 없으면 성공하고 파일을 만들지 않는다. canon 자기 repo의 hot은 postgres라서 fixture로만 보인다.

## Questions and assumptions

- 거절은 전부 아니면 전무이고 쓰는 중 디스크 오류에만 일부가 남는다고 가정한다. id마다 따로 판정하는 쪽이 낫다면 알려 달라.
- 한 번의 답을 모든 brief에 같이 적는다고 가정한다. 다른 답이 필요한 brief는 따로 실행한다.
- 승인 줄이 있는 change는 태그가 없어도 그 change를 adopt한 모든 subject의 reopen과 ship을 막는다고 가정한다. 고친 brief는 다시 승인하는 것으로 풀리고 F1이 그 비용을 줄인다.
- chore는 adopt하는 순간부터 관할이라고 가정한다. 승인 전에는 그 subject를 reopen하거나 ship하지 못하고 증거 기록은 막지 않는다.
- 지금의 brief 필드는 briefs 목록으로 바뀌고 status 스키마 버전을 올린다고 가정한다. status JSON을 읽는 도구는 고쳐야 한다.
- status의 사람 단계는 subject마다 하나이고 `--all` 옵션은 만들지 않는다고 가정한다.
- 승인한 brief를 고치면 그 brief를 adopt한 모든 subject가 다시 승인할 때까지 막힌다. 이 규칙은 이 변경 뒤에도 그대로다.

## Out of scope

- 승인 정책, 승인자 검증, `brief approve`의 사람 전용 규칙 변경.
- brief 승인 취소와 대시보드의 일괄 거절.
- postgres hot 백엔드 동작과 `canon query`의 거절 문구.
- 증거와 리뷰 레코드의 형식.

Approved: Journey Hwang 2026-10-27T10:13:49Z — "레츠고" (brief sha256:206c88064ca0253715f268cba7c0c29cceeffa6d0c3be913444306186fdfeef6)
