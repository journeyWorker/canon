# 0.31 승인과 adopt 흐름 개선

<!-- canon:section intent role=context -->
## Intent

canon으로 canon 0.30을 만들며 세 가지가 걸렸다.

- `canon brief approve`는 change 하나만 받는다. brief 여섯 개를 승인하려고 사람이 같은 명령을 여섯 번 쳤고 셸 반복문은 깨졌다. 대시보드 briefs 페이지도 brief마다 이름과 답을 따로 적는다.
- 여러 change를 adopt한 subject는 가장 새로 승인된 change 하나의 brief로 reopen과 ship이 판정된다. 그 brief를 고치면 시나리오가 태그하지 않은 subject의 reopen, ship, 증거 기록까지 막힌다. 시나리오가 태그한 다른 change의 미승인은 reopen과 ship 판정에 들어가지 않는다.
- hot 경로에 디렉터리가 있으면 `canon report`는 hot 레코드 없이 exit 0으로 끝난다. `canon query`는 같은 경우를 거절한다. 깨진 파일은 report도 이미 거절한다.
<!-- canon:waive prose.length intent "서로 다른 문제 셋을 하나씩 적은 목록이라 줄이면 현재 동작이 빠진다" -->
<!-- canon:waive prose.length brief "서로 다른 세 문제의 네 흐름을 각각 실패 경로와 함께 다루는 한 변경이라 합치면 현재 동작과 거절 조건이 빠진다" -->

Source: GitHub 이슈 #114와 0.30 계획 리뷰.

0.31 순서: CI → 버그 수정 → 승인과 adopt → brief 포맷 → CLI id → 검색. 여섯 brief는 함께 승인받는다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- ==**`canon brief approve` 한 번이 change 여럿을 같은 이름과 답으로 승인한다.**== 하나라도 거절되면 아무것도 쓰지 않는다.
- 대시보드 briefs 페이지에서 brief를 여럿 골라 한 번에 승인한다.
- subject의 reopen, ship, 증거 기록, status는 그 subject의 시나리오가 태그한 change의 brief로 판정한다. 태그하지 않은 change의 brief는 초안이든 승인 뒤 고친 것이든 막지 않는다.
- hot 경로에 디렉터리 같은 SQLite가 아닌 것이 있으면 report도 거절한다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 사람이 승인 대기 중인 brief 여럿을 본다 → `canon brief approve`에 change id를 모두 적어 한 번 실행한다 → ==**모든 brief가 승인되거나 하나도 승인되지 않는다.**==
2. F2 사람이 briefs 페이지에서 brief를 여럿 고르고 이름과 답을 한 번 적는다 → 승인 버튼을 한 번 누른다 → F1과 같은 규칙으로 brief마다 결과가 나온다.
3. F3 에이전트가 change를 여럿 adopt한 subject를 reopen, ship하거나 증거를 기록하고 status를 본다 → canon이 시나리오가 태그한 change마다 brief를 판정한다 → 막는 brief가 모두 이름 붙고 태그되지 않은 change는 막지 않는다.
4. F4 사용자가 hot 경로에 디렉터리가 있는 repo에서 report를 만든다 → canon이 hot 저장소를 열려 한다 → report가 hot 경로를 이름 붙여 거절한다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 여러 brief의 한 번 승인

- change id를 하나 이상 받는다. 생략하면 지금처럼 live change 하나를 읽고, 같은 id를 여러 번 적으면 한 번으로 센다.
- 승인자 이름과 답은 하나이고 모든 brief와 정책 승인에 같이 적힌다.
- 에이전트 role은 id가 몇 개든 사람 전용 거절(HUMAN_ONLY)을 받는다. 플래그로 풀 수 없고 아무것도 쓰지 않는다. hint는 모든 id를 담은 호출 한 줄을 사람에게 알려 준다.
- 쓰기 전에 모든 id를 검사한다: brief가 있다, blocker가 없다, 이름과 답이 한 줄이다. 하나라도 어기면 exit 2로 거절하고 거절된 id와 이유를 모두 이름 붙인다. 아무것도 쓰지 않는다.
- 통과하면 입력 순서대로 쓰며 내용은 id 하나를 승인할 때와 같다. 쓰는 중 디스크 오류가 나면 exit 1이다. 이미 쓴 id는 승인된 채 남고 나머지는 건너뛴다. 같은 호출을 다시 실행하면 승인된 id는 변경 없음이다.
- id가 하나면 출력은 지금과 같다. 여럿이면 JSON은 id마다 항목(written, unchanged, failed, skipped)을 담는다. `--input` 배열 실행은 그대로다.

### F2 대시보드에서 여러 brief 승인

- 승인할 수 있는 brief마다 선택 칸이 있고 blocker가 있는 brief는 고를 수 없다. 고른 개수가 보인다. 이름과 답, 버튼은 하나다.
- 각 brief는 페이지가 읽은 파일의 sha256에 묶인다. 고른 brief 하나라도 그 뒤 바뀌었거나 blocker가 생겼으면 F1처럼 모두 거절하고 아무것도 쓰지 않는다. 페이지는 거절된 brief를 이름 붙이고 다시 읽으라고 안내한다.
- 결과 기록, 정책 승인, 목록에서 빠지는 동작은 단일 승인과 같다.

### F3 관할 change로 판정

- 관할 change는 subject가 adopt한 change 가운데 그 subject의 시나리오가 `@flow:<change>.*`로 태그한 것이다.
- ==**reopen과 ship은 subject의 관할 change가 모두 현재 승인 상태일 때만 통과한다.**== 미승인과 승인 뒤 고침은 다른 이유로 이름이 나온다. 판정은 adopt 순서와 승인 순서에 따르지 않는다.
- 관할 밖 change의 brief는 초안이든 승인 뒤 고친 것이든 subject를 막지 않는다. 승인 뒤 고친 brief는 초안으로 읽는다. 거절 출력과 status는 관할 밖 change를 한 줄로 나열한다.
- 태그 없는 시나리오의 증거 기록은 현재 승인 상태인 adopt change 가운데 가장 새 것을 따른다. 관할 change가 없는 subject의 승인 범위도 같다. 그런 change가 없으면 지금 규칙이다. 첫 adopt 전에 증거가 있던 시나리오는 재검증하고, 나머지는 가장 새 adopt change의 승인을 기다린다.
- 태그된 시나리오의 증거 기록은 태그된 change의 승인을 기다린다. 막는 brief가 여럿이면 hint는 모든 id를 담은 `canon brief approve <id>…` 한 줄을 알려 준다.
- reopen 거절은 `--override-reason`으로 넘길 수 없고 ship의 brief 거절은 넘길 수 있다. chore의 승인 뒤 검사도 그대로다.

### F4 hot 경로 거절

- 경로가 없으면 hot 레코드가 없는 것으로 읽고 파일을 만들지 않는다. 지금과 같다.
- 경로에 무언가 있는데 SQLite 파일로 열리지 않으면 report는 exit 1로 거절하고 오류가 그 경로를 이름 붙인다. report를 만드는 build, check, snapshot 모두에 적용한다.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: ==**세 change를 한 번에 승인하면 세 brief와 정책에 같은 승인자와 답이 적히고, 하나씩 승인했을 때와 내용이 같다.**== 같은 호출을 다시 실행하면 변경 없음이다. 실패 경로: 하나에 blocker가 있거나 없는 id가 끼면 exit 2이고 거절된 id가 모두 이름 붙는다. 어느 brief도 바뀌지 않는다. planner role의 호출은 사람 전용 거절이고 hint가 모든 id를 담는다.
- F2: brief 셋을 고르고 승인하면 셋이 목록에서 빠지고 CLI로 승인한 복사본과 승인 내용이 같다. 실패 경로: 고른 brief 하나를 페이지를 읽은 뒤 고치면 409로 거절되고 셋 다 그대로이며 그 brief가 이름 붙는다.
- F3: 태그된 change가 둘이고 하나만 승인된 subject는 reopen과 ship이 미승인 change를 이름 붙여 거절한다. 둘을 어느 순서로 승인해도 통과하고, 둘 중 하나를 고치면 그 change를 이름 붙여 거절한다. 태그되지 않은 change의 brief를 고치거나 초안으로 두면 reopen, ship, 증거 기록이 통과한다. 출력은 그 change를 관할 밖으로 나열한다. 실패 경로: 관할 change가 둘 다 미승인이면 거절이 둘을 모두 이름 붙이고 hint는 id 둘을 담은 승인 호출 한 줄이다.
- F4: hot 경로에 디렉터리가 있는 fixture에서 report build, check, snapshot이 exit 1이다. 오류는 그 경로를 이름 붙인다. 깨진 파일도 같다. 경로가 없으면 성공하고 파일을 만들지 않는다.

<!-- canon:section questions role=question -->
## Questions and assumptions

- 여러 brief 승인의 거절은 전부 아니면 전무라고 가정한다. 쓰는 중 디스크 오류에만 일부가 남는다. id마다 따로 판정하는 쪽이 낫다면 알려 달라.
- 한 번의 답을 모든 brief에 같이 적는다고 가정한다. 다른 답이 필요한 brief는 따로 실행한다.
- 승인 뒤 고친 brief는 초안으로 읽어 태그되지 않았으면 막지 않는다고 가정한다. 태그된 시나리오가 있으면 막는다.
- status의 사람 단계는 지금처럼 subject마다 하나이고, 모든 brief를 승인하는 `--all` 옵션은 만들지 않는다고 가정한다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 승인 정책, 승인자 검증, `brief approve`의 사람 전용 규칙 변경.
- brief 승인 취소와 대시보드의 일괄 거절.
- postgres hot 백엔드 동작과 `canon query`의 거절 문구.
- 증거와 리뷰 레코드의 형식.
