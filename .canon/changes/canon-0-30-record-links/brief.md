# 0.30 레코드 연결 보강

<!-- canon:section intent role=context -->
## Intent

canon 레코드를 그래프로 조회하려는 조사에서 끊긴 연결 다섯 곳이 나왔다.

- finding이 자기가 나온 리뷰를 가리키지 않고, 리뷰에는 id가 없다. 어느 리뷰에서 나온 finding인지는 커밋 sha, 리뷰어, 시각으로 추정해야 한다.
- finding의 결함 커밋 칸이 대부분 비어 있다. canon 레저에서 279개 중 215개가 비어 있고, 그중 40개는 blocker다. 그래서 수정이 새 결함을 만든 경우가 연결되지 않는다.
- 시나리오 색인에는 제목만 있고 시나리오 단계가 없다.
- 레코드가 없는 대상을 가리켜도 알려 주는 곳이 없다. canon 레저에서 finding 184개가 가리키는 change에 해당하는 원본이 없다.
- sqlite hot 저장소가 worktree마다 따로라서 hot 레코드가 worktree마다 갈린다. 어디에 있는지도 보여 주지 않는다.
<!-- canon:waive prose.length intent "끊긴 연결 다섯 곳의 목록이고 줄이면 개수와 근거 숫자가 빠진다" -->

0.30 순서: 계획 이전 → 스펙 보강 → 코드 정리 → clippy CI → 레저 이름 변경 → 이 변경. 여섯 brief는 함께 승인받는다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- ==**모든 리뷰 레코드가 본문에 리뷰 id를 담고, 레코드가 옮겨지거나 다시 쓰여도 id는 그대로다.**== `review scenario`나 `review change`를 한 번 실행해 새로 쓴 레코드들은 같은 id를 갖는다.
- 리뷰 중에 나온 finding은 그 리뷰 id를 가리킨다.
- 새 결함 finding은 결함 커밋이나 "모름" 표시 중 하나를 반드시 갖는다.
- 시나리오 색인 레코드에 단계가 원문 순서로 들어 있고, 0.29 색인도 다음 동기화 때 단계를 얻는다.
- 조회와 `status`가 원본이 없는 참조를 표시하고 개수를 보고한다. 같은 커밋이면 어느 머신에서든 결과가 같다.
- hot 저장소 위치가 `canon init --check`와 hot 레코드를 쓰는 명령의 출력에 나온다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 reviewer가 `review scenario`를 실행한다 → canon이 리뷰 레코드들을 쓰거나 이전과 같은 리뷰는 건너뛴다 → 출력에 시나리오별 리뷰 id가 나오고, 그중 하나를 `finding add`에 넘기면 finding이 그 리뷰를 가리킨다.
2. F2 reviewer가 blocker나 should_fix finding을 새로 기록한다 → canon이 결함 커밋 또는 "모름" 표시를 확인한다 → ==**둘 다 없으면 거절하고**==, 하나가 있으면 기록한다.
3. F3 `canon inventory sync`가 돈다 → canon이 시나리오 색인을 쓴다 → 색인 레코드를 조회하면 단계가 나온다.
4. F4 에이전트가 레코드를 조회하거나 `canon status`를 본다 → canon이 참조를 원본과 대조한다 → 원본이 없는 참조가 표시되고 종류별 개수가 보고된다.
5. F5 사용자가 `canon init --check`를 실행하거나 hot 레코드를 쓴다 → canon이 hot 저장소를 연다 → 출력에 hot 저장소 위치가 나온다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 리뷰 id

- ==**리뷰 id는 실행할 때 새로 만드는 ULID이고, 레코드 본문 필드로 저장한다.**== 파일 이름에서 얻지 않는다.
- 이전 리뷰와 같은 내용이라 건너뛴 시나리오는 이전 레코드의 id를 그대로 보여 준다. 같은 내용인지 비교할 때 리뷰 id는 시각처럼 제외한다. 그래서 재시도가 리뷰를 중복으로 쓰지 않는다.
- 출력은 시나리오마다 리뷰 id를 보여 준다. 한 실행에 새로 쓴 것과 건너뛴 것이 섞이면 id가 여럿일 수 있다.
- `finding add`의 새 선택 인자로 리뷰 id를 받는다. 레저와 archive 어디에도 그 id의 리뷰가 없으면 거절한다.
- 리뷰 스키마와 finding 스키마 버전을 올린다. conformance 정규화는 ULID 모양을 가린다.
- `finding close`와 다시 열기는 리뷰 id를 그대로 복사한다.
- 0.30 전 리뷰와 finding은 이 칸이 비어 있고 그대로 유효하다.

### F2 결함 커밋

- 결함 커밋 인자는 지금처럼 기본값이 없다.
- "모름"은 별도 필드에 저장한다. 결함 커밋 칸과 섞지 않는다.
- 이 규칙은 `finding add`가 새 finding을 쓸 때만 적용한다. 이미 있는 finding의 닫기와 다시 열기는 막지 않는다. 두 필드 모두 상태 전이에서 바뀌지 않아야 하는 필드에 넣는다.
- finding 스키마 버전을 올린다.

### F3 시나리오 단계

- 단계마다 키워드(Given/When/Then/And/But)와 문장을 원문 순서로 담는다. Background와 Rule에서 물려받은 단계는 따로 표시한다.
- 단계는 색인의 "변경 없음" 판단에 들어간다. 그래서 0.29 색인 레코드도 다시 쓰인다.
- 시나리오 해시 계산은 바뀌지 않는다.

### F4 원본 없는 참조

- 원본은 커밋되는 것만 센다: `.canon/changes` 아래 변경, 설정된 계획 소스, 스펙 코퍼스, 커밋된 레저 레코드. gitignore된 파생 레코드는 원본이 아니다.
- 확인하는 참조: finding의 change와 리뷰 id, evidence의 task와 scenario, subject의 change.
- 0.23 이전 리뷰 핀은 확인하지 않는다.
- `gate check` 판정은 바뀌지 않는다.

### F5 hot 저장소 위치

- 위치 규칙은 지금과 같다(설정 파일 기준 경로, worktree마다 따로). 이 변경은 위치를 보여 주기만 한다.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: 시나리오 셋을 한 번에 리뷰하면 레코드 셋이 같은 id를 갖고, 그 id로 만든 finding을 조회하면 id가 나온다. 같은 리뷰를 다시 실행하면 새 레코드 없이 이전 id가 나온다. ==**`canon migrate` 뒤에도 id가 같다.**== 실패 경로: 없는 리뷰 id는 거절되고 레저는 그대로다.
- F2: 결함 커밋도 "모름"도 없는 새 blocker는 거절된다. note는 둘 다 없이 기록된다. 0.29에 쓴 결함 커밋 없는 blocker는 그대로 닫힌다. 실패 경로가 곧 이 단계의 거절이다.
- F3: 0.29 색인이 있는 레포에서 동기화하면 모든 시나리오 레코드에 단계가 생긴다. 실패 경로: 단계를 고친 `.feature`를 다시 동기화하면 새 단계로 바뀌고 옛 레코드는 archive로 간다.
- F4: 원본이 없는 change를 가리키는 finding이 있는 fixture에서 조회와 `status`가 그 참조를 표시하고 개수를 보고한다. 같은 커밋을 새로 받은 곳과 기존 작업 트리에서 `status --json`이 같다. `gate check` 출력은 바뀌지 않는다.
- F5: `canon init --check`와 hot 레코드를 쓴 명령의 출력에 hot 저장소 경로가 나온다.

<!-- canon:section questions role=question -->
## Questions and assumptions

- 질문(정책): 새 결함 finding에 결함 커밋이나 "모름"을 요구하는 것은 리뷰어의 기록 방식을 바꾼다. 요구할까, 경고로 둘까? 가정: 요구한다.
- 질문: hot 저장소를 같은 레포의 worktree끼리 공유할까? 공유하면 설정 경로의 의미와 동시 쓰기 잠금이 바뀐다. 가정: 0.30에서는 공유하지 않고 위치만 보여 준다. 시나리오 색인은 worktree의 `.feature`에서 만드는 것이라 worktree마다 다른 게 맞다.
- 질문: 그래프 조회 명령도 0.30에 넣을까? 가정: 0.30은 연결 데이터만 고치고 조회 명령은 별도 변경으로 한다.
- 가정(소유자 결정 2026-10-26): `.feature` 파싱은 표준 Gherkin 파서인 `gherkin` crate를 쓴다. 단계 색인은 그 파서의 결과에서 얻는다. canon의 자체 파서를 대체하는 경우, 코퍼스 전체에서 시나리오 해시가 대체 전과 같아야 한다. 같지 않으면 해시 계산은 자체 파서에 남긴다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 그래프 조회 명령.
- 기존 finding의 빈 칸을 거슬러 채우는 일.
- 원본 없는 옛 참조를 고치거나 지우는 일.
- worktree 사이 hot 저장소 공유.
