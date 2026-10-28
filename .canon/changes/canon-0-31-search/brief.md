---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "명령 둘의 출력 보고서와 실패 경로를 정하는 변경이라 줄이면 필드와 실패 경로가 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "명령 둘의 입력 범위, 엣지 규칙, 사슬 구성, 실패 경로를 정하는 단계라 줄이면 규칙이 빠진다"
---

# 0.31 검색과 레코드 그래프

## Intent

canon 레코드는 서로를 가리키지만 그 그래프를 보여 주는 명령이 없고, 텍스트 검색도 없다. `canon query`는 한 번에 한 종류를 구조화된 필터로 나열할 뿐이다. 구현 → 리뷰 → finding → 수정 → 재리뷰 고리는 커밋 sha로만 이어져서, 레코드를 하나씩 읽어야 보인다. 0.30이 리뷰 id, finding의 리뷰 링크, 시나리오 단계, 원본 없는 참조 보고, hot 저장소 위치를 채워서 이제 그 그래프를 만들 수 있다.

0.31 순서: CI → 승인과 adopt → 버그 수정 → brief 포맷 → CLI id → 검색. 여섯 brief는 함께 승인받는다. 승인한 brief를 고치면 그 brief를 adopt한 모든 subject가 재승인 전까지 막힌다.

## Desired outcome

- ==**`canon search "<텍스트>"`는 레코드, brief 섹션, 시나리오 본문을 전문 검색하고, 걸린 노드마다 그래프 이웃을 함께 낸다.**==
- `canon trace <id>`는 리뷰 → finding → 수정 → 재리뷰 → evidence 사슬을 시간순으로 낸다.
- 색인은 gitignore된 파생물이다. 지우면 다음 실행이 같은 내용으로 다시 만든다.
- 한국어 본문도 검색된다.
- 원본 없는 참조는 표시가 붙은 노드로 나오고 오류를 내지 않는다.

## User flow

1. F1 사용자가 `canon search`를 처음 실행하거나 레코드를 쓴 뒤 실행한다 → canon이 색인이 없거나 낡았는지 보고 다시 만든다 → ==**결과에 방금 승격한 레코드까지 들어 있다.**==
2. F2 에이전트가 키워드로 `canon search`를 실행한다 → canon이 전문 검색으로 시드 노드를 찾고 이웃을 붙인다 → 순위가 매겨진 hit, 이웃, 엣지가 나온다.
3. F3 리뷰어가 `canon trace`에 finding, 리뷰 id, 커밋 sha 중 하나를 준다 → canon이 같은 고리에 묶인 레코드를 따라간다 → 시간순 사슬이 나온다.
4. F4 에이전트가 원본 없는 change를 가리키는 finding의 이웃을 본다 → canon이 그 change를 자리표시 노드로 둔다 → 표시가 붙어 나오고 오류는 없다.

## Step detail

### F1 색인 만들기와 갱신

- 색인은 worktree마다 하나인 파일이고 `.canon/index/` 아래에 두며 -wal, -shm 파일까지 gitignore한다. `canon init`이 그 줄을 넣는다. 첫 빌드는 `.canon/index/`에 `*`만 담은 ignore 파일도 써서, init을 다시 실행하기 전에도 색인은 커밋되지 않는다. 이미 init한 레포는 init을 다시 실행해 받고, 그 전에는 `search`와 `trace`가 경고를 낸다.
- ==**색인은 원본이 아니다.**== 색인에서 읽어 레코드를 쓰는 일은 없고, 지우고 다시 만들어도 `search`와 `trace`의 결과는 같다.
- 입력은 승격된 현재 버전의 레저 레코드, 이 worktree의 sqlite hot 저장소, brief, `.feature` 파일이다. 레저 archive의 finding, 리뷰, evidence 옛 버전도 읽는다(F3). evidence는 타입 레코드가 버리는 참조, 요약, 명령 결과 원문도 넣는다.
- 승격 전에 staged인 레코드와 postgres, 객체 저장소의 레코드는 읽지 않는다. 읽지 않은 저장소 이름이 `index`에 나온다.
- 입력이 하나라도 바뀌면 낡은 것이고, 읽기 전에 전부 다시 만든다. 동시에 실행해도 색인은 깨지지 않는다.
- 두 명령은 read 효과이고, 효과 설명에 gitignore된 색인 파일과 그 ignore 파일을 먼저 갱신한다고 적는다.
- 색인 파일이 데이터베이스가 아니면 지우고 한 번 다시 만들며 경고를 낸다. 쓸 수 없는 디렉터리면 그 실행 동안만 메모리에 만들고 경고에 경로를 적는다. 입력 레코드를 읽을 수 없으면 원인을 내고 exit 1이다.

### F2 검색

- 노드 id는 종류 접두어와 `canon query`가 보이는 자연 키다. 종류가 다르면 같은 id 문법도 겹치지 않는다.
- 노드 종류는 `canon query`가 읽는 모든 레코드 종류와 brief다. brief는 `brief check`가 읽는 섹션별 노드이고 flow 단계도 노드다.
- 시나리오 본문은 `.feature` 파일에서 읽은 제목, 태그, 단계, doc string, 표, Examples다. 물려받은 Background와 Rule 단계도 넣는다.
- 한국어는 세 글자 단위 토큰화로 찾는다. 세 글자 미만 질의는 노드 본문의 부분 문자열 일치로 찾고 id순으로 낸다.
- 질의 문자열은 글자 그대로다. 따옴표와 AND 같은 말은 문법이 아니다.
- 옵션: `--kind`(반복, 시드만 거른다), `--expand <N>`(홉 수, 기본 1), `--limit <N>`(hit 수, 기본 20, 페이지 없음).
- 시드는 전문 검색 점수순, 동점은 id순이다. 이웃은 홉이 가까운 순이다. hit는 현재 버전만 보인다.
- 엣지는 다음과 같다.

| rel | 이음 |
|---|---|
| 명시 참조 | 레코드의 참조 필드 전부. 필드 이름이 rel이다. finding의 리뷰 id는 그 id를 담은 모든 리뷰로 간다 |
| re-review | finding의 수정 커밋 = 같은 change에서 (round, seq)가 더 큰 finding의 검토 커밋 |
| fix-reviewed | 리뷰의 pin = finding의 수정 커밋이고, 리뷰가 그 finding의 change나 시나리오를 다룬다 |
| fix-of-fix | finding의 결함 커밋 = 같은 change에서 (round, seq)가 더 작은 finding의 수정 커밋 |
| evidence-at-commit | evidence의 커밋 = finding의 검토·수정 커밋 또는 리뷰의 pin이고, 그 finding이나 리뷰가 evidence의 시나리오를 가리킨다 |
| flow | 시나리오의 `@flow:<change>.<F-id>` 태그 → brief flow 단계 |

- 보고 JSON의 `data`에는 다음이 있다.
  - `hits[]`: id, kind, rank, score, title, snippet, placeholder.
  - `neighbors[]`: id, kind, hops, via, title, placeholder.
  - `edges[]`: from, to, rel.
  - `counts`와 `index`(경로, 노드 수, 엣지 수, 이번에 다시 만들었는지, 읽지 않은 저장소).
- 일치가 없으면 exit 0과 빈 `hits`다. 질의가 비었거나 공백뿐이면 exit 2다.

### F3 추적

- 피연산자는 셋 중 하나다. 26자 ULID는 리뷰 id이고, 그 id를 가진 리뷰가 없으면 sha 앞부분으로 다시 본다. 7~40자 16진수는 커밋 sha이고 앞부분만 줘도 된다. 이어 쓴 finding 키는 `canon query`가 보이는 change, 라운드, seq를 밑줄로 이은 것이다.
- 리뷰 id는 한 실행이 쓴 리뷰 전부를 가리킨다. 시작점은 그 리뷰들이다. 시작점이 sha이면 그 sha를 담은 레코드 전부다.
- 사슬은 시작점에서 finding→리뷰, re-review, fix-reviewed, fix-of-fix, evidence-at-commit 엣지만 따라 닿는 레코드다. finding에서 리뷰로 갈 때는 그 finding의 change나 시나리오를 다룬 리뷰만 남기고, 없으면 그 실행의 리뷰 전부를 남긴다.
- 사슬은 archive의 옛 버전을 포함한다. 항목 하나는 한 버전이다. finding은 처음 기록한 버전(역할 finding)과 fixed로 닫은 버전(역할 fix)이 따로 나오고, 시각은 각 버전의 시각이다.
- evidence 엣지는 finding이나 리뷰가 시나리오를 가리킬 때만 생긴다. 시나리오 없이 기록한 finding의 사슬에는 evidence가 없다.
- 항목은 시각 오름차순이고 동점은 id순이다. 항목은 시각, id, kind, 역할, sha, 이은 엣지를 담는다.
- 어떤 레코드도 말하지 않는 id, 모호한 sha 앞부분, 인식할 수 없는 모양은 exit 2다. 메시지에 받는 모양 셋을 적는다.

### F4 자리표시 노드

- 원본 없는 참조는 `canon query`가 원본 없는 참조로 보고하는 것과 같다(`status`는 개수만 센다). 그 id마다 자리표시 노드가 하나 생기고 가리키는 곳의 수를 담는다. 본문이 없어 검색되지 않고 이웃으로만 나온다.

## Acceptance

- F1: 색인이 없는 레포에서 `search`가 색인을 만들고, 색인 디렉터리에 `*`만 담은 ignore 파일이 생기고, 결과가 나온다. finding을 승격한 직후 같은 질의에 그 finding이 나온다. ==**색인 파일을 지우고 다시 실행하면 다시 만들었는지 나타내는 필드만 빼고 같은 결과가 나온다.**== 레코드 2,300건짜리 픽스처에서 낡은 색인의 전체 재구성은 5초 안에 끝난다. 실패 경로: 깨진 색인 파일은 경고와 함께 다시 만들어지고, 쓸 수 없는 디렉터리에서는 경고와 함께 결과가 나온다.
- F2: 한국어 문장으로 쓴 finding 요약이 두 글자 단어와 세 글자 이상 단어 양쪽으로 찾힌다. 그 finding의 이웃에 같은 change의 리뷰와 시나리오가 있다. 실패 경로: 빈 질의는 exit 2이고, 일치 없는 질의는 exit 0과 빈 `hits`다.
- F3: 픽스처에서 리뷰 R1이 시나리오를 가리키는 finding을 기록하고 수정 커밋 S2가 닫는다.
  - 리뷰 R2가 S2에 고정되어 R1을 archive로 밀어낸다.
  - 그 시나리오의 evidence가 S2에서 S3으로 다시 기록되어 S2 것이 archive로 간다.
  - finding을 추적하면 R1, finding, fix, R2, evidence가 이 시간순으로 나온다. R1과 S2의 evidence는 archive 버전이다.
  - S2로 추적해도 같은 항목이 나온다.
  - 실패 경로: 없는 id는 exit 2다.
- F4: 원본 없는 change를 가리키는 finding을 검색하면 그 change가 자리표시로 나온다. 자리표시 id 집합은 `canon query`가 보고하는 원본 없는 참조의 서로 다른 id 집합과 같다. 실패 경로: 자리표시 id로 `trace`를 실행하면 exit 2다.

## Questions and assumptions

- 질문: 첫 판에 벡터 검색과 tree-sitter 코드 기호 엣지를 넣을까? 가정: 둘 다 넣지 않는다. 전문 검색과 그래프 확장만 한다.
- 질문: 색인을 worktree마다 둘까, 메인 체크아웃과 합칠까? 가정: hot 저장소처럼 worktree마다 둔다.
- 질문: 한국어 토큰화는? 가정: 세 글자 단위 토큰화이고, 세 글자 미만 질의는 부분 문자열 일치로 보완한다.
- 질문: 이슈의 같은 커밋 evidence 엣지를 그대로 둘까? 가정: evidence끼리는 잇지 않는다. 한 커밋의 evidence 수십 건이 사슬을 덮기 때문이다. finding이나 리뷰가 가리키는 시나리오의 evidence만 잇는다.
- 질문: archive는 어디까지 읽을까? 가정: finding, 리뷰, evidence의 옛 버전만, `trace`에만 쓴다.
- 질문: 이 레포의 hot 저장소는 postgres다. 거기 있는 레코드와 승격 전 staged 레코드를 색인할까? 가정: 첫 판은 읽지 않고, 저장소는 `index`에 읽지 않은 저장소로 표시한다.
- 질문(정책): 읽기 명령이 gitignore된 색인 파일을 쓰는 것을 허용할까? 가정: 효과 설명에 적으면 허용한다.
- 질문: 낡음 판정과 비용은? 가정: 입력 전체의 지문으로 판정하고 전체를 다시 만든다. 쓰기 뒤 첫 검색이 비용을 내고, 5초 상한은 F1 acceptance가 잰다.
- 가정: id 옵션이 같은 릴리스에서 명사 이름으로 바뀌므로 옵션은 `--kind`, `--expand`, `--limit`다. `--limit`은 페이지 없는 hit 수 상한이다.

## Out of scope

- 벡터와 임베딩, 순위 융합(RRF).
- tree-sitter 코드 기호 엣지.
- postgres와 객체 저장소 레코드, staged 레코드, finding, 리뷰, evidence 밖의 archive 검색.
- 외부 그래프 엔진 내장과 노드·엣지 JSONL 내보내기.
- worktree 사이 색인 공유.
- evidence 저장 방식 변경과 finding·리뷰 레코드 스키마 변경.
- 시나리오 해시 계산 변경.

Approved: Journey Hwang 2026-10-27T10:13:53Z — "레츠고" (brief sha256:2eef6217002b1563753763320f80fcd026bc20d649900c61f2cb751e8673566c)
