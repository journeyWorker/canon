# 0.31 검색과 레코드 그래프

<!-- canon:section intent role=context -->
## Intent

canon 레코드는 서로를 가리키지만 그 그래프를 보여 주는 명령이 없고, 텍스트 검색도 없다. `canon query`는 한 번에 한 종류를 구조화된 필터로 나열할 뿐이다. 구현 → 리뷰 → finding → 수정 → 재리뷰 고리는 커밋 sha로만 이어져서, 레코드를 하나씩 읽어야 보인다. 0.30이 리뷰 id, finding의 리뷰 링크, 시나리오 단계, 원본 없는 참조 보고, hot 저장소 위치를 채워서 이제 그 그래프를 만들 수 있다.
<!-- canon:waive prose.length brief "명령 둘의 출력 보고서와 실패 경로를 정하는 변경이라 줄이면 필드와 실패 경로가 빠진다" -->

0.31 순서: CI → 버그 수정 → 승인과 adopt → brief 포맷 → CLI id → 검색. 여섯 brief는 함께 승인받는다.

<!-- canon:section outcome role=normative -->
## Desired outcome

- ==**`canon search "<텍스트>"`는 레코드, brief 섹션, 시나리오 본문을 전문 검색하고, 걸린 노드마다 그래프 이웃을 함께 낸다.**==
- `canon trace <id>`는 리뷰 → finding → 수정 → 재리뷰 → evidence 사슬을 시간순으로 낸다.
- 색인은 gitignore된 파생물이다. 지우면 다음 실행이 같은 내용으로 다시 만든다.
- 한국어 본문도 검색된다.
- 원본 없는 참조는 표시가 붙은 노드로 나오고 오류를 내지 않는다.

<!-- canon:section flow role=normative -->
## User flow

1. F1 사용자가 `canon search`를 처음 실행하거나 레코드를 쓴 뒤 실행한다 → canon이 색인이 없거나 낡았는지 보고 다시 만든다 → ==**결과에 방금 쓴 레코드까지 들어 있다.**==
2. F2 에이전트가 키워드로 `canon search`를 실행한다 → canon이 전문 검색으로 시드 노드를 찾고 이웃을 붙인다 → 순위가 매겨진 hit, 이웃, 엣지가 나온다.
3. F3 리뷰어가 `canon trace`에 finding, 리뷰 id, 커밋 sha 중 하나를 준다 → canon이 같은 고리에 묶인 레코드를 따라간다 → 시간순 사슬이 나온다.
4. F4 에이전트가 원본 없는 change를 가리키는 finding의 이웃을 본다 → canon이 그 change를 자리표시 노드로 둔다 → 표시가 붙어 나오고 오류는 없다.

<!-- canon:section detail role=normative -->
## Step detail

### F1 색인 만들기와 갱신

- 색인은 worktree마다 하나인 파일이고 `.canon/` 아래에 두며 gitignore한다. `canon init`이 관리하는 ignore 줄에 들어간다.
- ==**색인은 원본이 아니다.**== 색인에서 읽어 레코드를 쓰는 일은 없고, 지우고 다시 만들어도 `search`와 `trace`의 결과는 같다.
- 입력은 현재 버전의 레저 레코드와 hot 저장소, brief, `.feature` 파일이다. evidence는 타입 레코드가 버리는 참조, 요약, 명령 결과 원문도 넣는다.
- 입력이 하나라도 바뀌면 낡은 것이고, 읽기 전에 다시 만든다. 동시에 실행해도 색인은 깨지지 않는다.
- 두 명령은 read 효과이고, 효과 설명에 gitignore된 색인 파일을 먼저 갱신한다고 적는다.
- 색인 파일이 데이터베이스가 아니면 지우고 한 번 다시 만들며 경고를 낸다. 다시 만들 수 없으면 원인과 경로를 내고 exit 1이다.

### F2 검색

- 노드 종류는 `canon query`가 읽는 모든 레코드 종류와 brief다. brief는 섹션별 노드이고 flow 단계도 노드다.
- 시나리오 본문은 제목, 태그, 단계, doc string, 표, Examples다. Background와 Rule에서 물려받은 단계도 넣는다.
- 한국어는 세 글자 단위 토큰화로 찾는다. 세 글자 미만 질의는 부분 문자열 일치로 찾는다.
- 질의 문자열은 그대로 글자로 읽는다. 따옴표와 AND 같은 말은 문법이 아니다.
- 옵션: `--kind`(반복, 시드만 거른다), `--expand <N>`(홉 수, 기본 1), `--limit <N>`(시드 수, 기본 20).
- 시드는 전문 검색 점수순이고 동점은 id순이다. 이웃은 홉이 가까운 순이다.
- 엣지는 다음 여섯 가지다.

| rel | 이음 |
|---|---|
| 명시 참조 | 레코드가 담은 참조 필드 전부, 필드 이름이 rel |
| re-review | 수정 커밋이 이후 finding의 검토 커밋과 같다 |
| fix-reviewed | 리뷰 pin이 finding의 수정 커밋과 같다 |
| fix-of-fix | finding의 결함 커밋이 다른 finding의 수정 커밋과 같다 |
| same-commit | evidence의 커밋이 같다 |
| flow | 시나리오의 `@flow:<change>.<F-id>` 태그와 brief flow 단계 |

- 보고 JSON의 `data`에는 다음이 있다.
  - 질의 문자열과 `counts`.
  - `hits[]`: id, kind, rank, score, title, snippet, placeholder.
  - `neighbors[]`: id, kind, hops, via, title, placeholder.
  - `edges[]`: from, to, rel.
  - `index`: 경로, 노드 수, 엣지 수, 이번에 다시 만들었는지.
- 일치가 없으면 exit 0과 빈 `hits`다. 질의가 비었거나 공백뿐이면 exit 2다.

### F3 추적

- 피연산자는 finding(`<change>/<round>/<seq>`), 리뷰 id, 커밋 sha(7자 이상, 앞부분만 가능) 중 하나다.
- 사슬은 대상에서 출발해 finding→리뷰, re-review, fix-reviewed, fix-of-fix, same-commit 엣지만 따라 닿는 레코드다. `at` 오름차순이고 동점은 id순이다.
- 사슬 항목은 시각, id, kind, 사슬 안의 역할, sha, 이은 엣지를 담는다.
- 어떤 레코드도 말하지 않는 id, 모호한 sha 앞부분, 인식할 수 없는 모양은 exit 2다. 메시지에 받는 모양 셋을 적는다.

### F4 자리표시 노드

- 원본 없는 참조는 0.30의 `status`가 세는 것과 같다. 그 id마다 자리표시 노드가 하나 생기고 가리키는 곳의 수를 담는다. 본문이 없어 검색되지 않고 이웃으로만 나온다.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: 색인이 없는 레포에서 `search`가 색인을 만들고 결과를 낸다. 레코드를 쓴 직후 같은 질의에 그 레코드가 나온다. ==**색인 파일을 지우고 다시 실행하면 같은 결과가 나온다.**== 실패 경로: 깨진 색인 파일은 경고와 함께 다시 만들어지고, 쓸 수 없는 디렉터리에서는 exit 1과 경로가 나온다.
- F2: 한국어 문장으로 쓴 finding 요약이 두 글자 단어와 세 글자 이상 단어 양쪽으로 찾힌다. 그 finding의 이웃에 같은 change의 리뷰와 시나리오가 있다. 실패 경로: 빈 질의는 exit 2이고, 일치 없는 질의는 exit 0과 빈 `hits`다.
- F3: 한 번 수정한 finding의 id로 추적하면 리뷰, finding, 수정, 재리뷰 순서로 나온다. 같은 사슬을 그 수정 커밋 sha로 추적해도 같은 항목이 나온다. 실패 경로: 없는 id는 exit 2다.
- F4: 원본 없는 change를 가리키는 finding을 검색하면 그 change가 자리표시로 나온다. 자리표시 id는 `status`가 보고한 원본 없는 참조 id를 모두 덮는다. 실패 경로: 자리표시 id로 `trace`를 실행하면 exit 2다.

<!-- canon:section questions role=question -->
## Questions and assumptions

- 질문: 첫 판에 벡터 검색을 넣을까? 가정: 넣지 않는다. 전문 검색과 그래프 확장만 한다.
- 질문: 코드 기호 엣지(tree-sitter)를 넣을까? 가정: 넣지 않는다.
- 질문: 색인을 worktree마다 둘까, 메인 체크아웃과 합칠까? 가정: hot 저장소처럼 worktree마다 둔다.
- 질문: 한국어 토큰화는? 가정: 세 글자 단위 토큰화이고, 세 글자 미만 질의는 부분 문자열 일치로 보완한다.
- 질문: 색인에 넣을 범위는? 가정: 현재 버전의 레코드, 이 worktree의 hot 저장소, brief, `.feature` 파일이다. archive와 원격 저장소는 뺀다.
- 질문(정책): 읽기 명령이 색인 파일을 쓰는 것을 허용할까? 가정: gitignore된 캐시이고 효과 설명에 적으면 허용한다.
- 질문: 기본값은? 가정: 시드 20개, 확장 1홉, 사슬 길이 제한 없음.
- 질문: 사슬이 닿는 대상을 시나리오와 change로도 넓힐까? 가정: finding, 리뷰, sha 셋만 받는다.
- 가정: 이 변경은 같은 릴리스에서 id 옵션이 명사 이름으로 바뀐다는 전제로 옵션을 `--kind`, `--expand`, `--limit`로 정했다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 벡터와 임베딩, 순위 융합(RRF).
- tree-sitter 코드 기호 엣지.
- 원격(postgres, 객체 저장소) 레코드와 archive 검색.
- 외부 그래프 엔진 내장과 노드·엣지 JSONL 내보내기.
- worktree 사이 색인 공유.
- evidence 저장 방식 변경과 finding·리뷰 레코드 스키마 변경.
- 시나리오 해시 계산 변경.
