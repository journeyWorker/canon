---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "잰 값과 불변마다 비교할 필드를 열거해야 구현자가 다시 재지 않는다"
  - rule: "prose.length"
    section: "intent"
    reason: "측정표와 원인의 세 곳(보관함 버전, 캐시, 복제)이 근거 수치다"
  - rule: "prose.length"
    section: "invariants"
    reason: "불변마다 비교하는 필드, 남기는 필드, 실패 코드를 열거해야 한다"
  - rule: "prose.length"
    section: "checks"
    reason: "확인마다 질의 집합과 비교 대상이 필요하다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "측정과 원인의 철자(색인 파일, 필드)가 이 변경의 대상이다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "보고서 필드, 판정 코드, 레코드 필드는 비교와 시험이 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "확인은 필드와 파일의 철자를 그대로 말해야 같은 것을 돌린다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 측정값의 출처다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 필드와 코드의 목록이다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 질의와 표의 목록이다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "불변마다 필드와 코드를 열거해야 해서 문장을 나누면 대응이 끊긴다"
  - rule: "prose.sentence-length"
    section: "checks"
    reason: "확인마다 질의와 비교 대상을 열거해야 한다"
  - rule: "prose.paragraph-length"
    section: "invariants"
    reason: "실패 경로와 시험 규칙은 한 불변에 묶여야 대응이 끊기지 않는다"
  - rule: "prose.key-sentence"
    section: "invariants"
    reason: "불변마다 굵은 핵심 문장이 하나씩 있어야 구현자가 찾는다"
  - rule: "prose.key-sentence"
    section: "checks"
    reason: "확인마다 굵은 핵심 문장이 하나씩 있어야 구현자가 찾는다"
---

# canon search와 canon trace는 기록을 한 벌만 들고, 보관함 버전의 본문은 들지 않는다

Source: crates/canon-cli/src/search/inputs.rs, crates/canon-cli/src/search/graph.rs, crates/canon-cli/src/search/index.rs, crates/canon-store/src/git_tier.rs, crates/canon-cli/src/query.rs, canon 0.43.0(`dd12bba71`)의 릴리스 빌드를 canon 자기 저장소(`.canon` 269MB)에서 잰 값(`/usr/bin/time -v`, Linux x86_64, 코어 12, 임시 계측 빌드)

## Intent

`canon search approval --json`은 색인이 최신일 때 최대 상주 메모리가 594MB다. 운영자가 0.41 macOS에서 잰 725MB와 같은 문제다. 같은 저장소에서 `canon status --json`은 113MB, `canon gate check`는 110MB다. 검색은 결과 20건을 내는데 상태 조회의 다섯 배를 쓴다.

| 명령 | 최대 상주 메모리 | 시간 |
| --- | --- | --- |
| `search approval --json`, 색인이 최신, 세 번 | 593.6 · 594.0 · 593.6MB | 4.7초 |
| `search approval --kind brief --json`, 색인이 최신 | 595MB | 3.9초 |
| `search ab --json`, 색인이 최신 | 595MB | |
| `trace 2c5ea809f --json`, 색인이 최신 | 612MB | |
| `search approval --json`, 색인이 없음 | 890MB | 7.7초 |
| `status --json` | 113MB | 12.0초 |

색인이 최신이어도 `--kind`로 좁혀도 값이 같다. 매 실행이 색인을 쓰기 전에 모든 기록을 읽어 지문을 만들고, 그 기록을 끝까지 들고 있기 때문이다. 늘어나는 곳은 한 곳이다.

- 읽는 버전은 46,499개다. 현재 버전이 6,516개(JSON 5.4MB), 보관함에서 대체된 버전이 39,983개(JSON 33.1MB)다. 보관함이 개수와 바이트 모두 86%다. 보관함의 본문은 `canon trace`의 항목표를 만들 때만 쓰이고, 색인이 최신이면 아무도 읽지 않는다. 지문에는 버전마다 종류, 키, 다이제스트만 들어간다.
- 증거 기록을 읽는 동안 상주 메모리가 84MB에서 548MB로 오른다. 파일 41,072개, 합계 40MB이니 파일 하나에 11.3KB다. 모든 기록을 읽은 직후 570MB에 이르고 그 뒤로 거의 늘지 않으며 최대값과 같다. 이 메모리는 읽은 기록을 끝까지 들고 있는 몫이다.
- 본문이 한 벌이 아니다[INFERENCE: 코드를 읽은 것이고 따로 잰 것은 아니다]. 저장소 크레이트의 프로세스 전역 파일 캐시가 읽은 모든 파일의 파싱된 본문을 프로세스가 끝날 때까지 들고, 소유한 본문을 돌려주는 읽기가 그 본문을 복제한다. 검색은 파일을 한 번만 읽으므로 캐시가 얻는 것이 없다. 색인을 새로 만들 때는 복제가 한 번 더 있다. 검토 고리의 항목이 본문을 다시 복사한다. 색인이 없는 실행은 기록을 다 읽은 580MB에서 색인을 연 뒤 859MB(최대 869MB)가 된다. 늘어난 280MB는 이 복사와 SQLite 쓰기가 나눠 갖는다[INFERENCE: 둘을 따로 재지 않았다].

임시 계측 빌드로 확인했다. 커밋하지 않았고 상자에서 지웠다. 보관함의 버전 39,983개를 하나씩 읽어 키, 다이제스트, 시각만 남기고 본문을 버리게만 바꿨다(캐시를 거치지 않는다). 이 한 가지로 `search approval --json`이 594MB에서 118MB로, 4.7초에서 4.1초로 줄었다. 지문이 같아서 0.43 코드가 만든 색인 파일을 그대로 다시 쓴다(`rebuilt`가 false다). 보고서 `data`를 정렬한 JSON의 sha256이 변경 전과 같다(`ccc3d315…`). 남은 118MB는 현재 버전 6,516개가 캐시와 복제로 두 벌 있는 몫이다[INFERENCE].

이 변경 뒤 `search`와 `trace`는 보관함 버전의 본문을 들지 않고, 현재 버전의 본문을 한 벌만 들며, 색인을 새로 만들 때도 항목에 본문을 복사하지 않는다. 결과는 같다.

## Category

refactor

## Invariants

- I1 ==**결과는 바뀌지 않는다.**== 같은 저장소와 같은 색인 상태에서 `search`와 `trace`의 `--json` 보고서 `data` 전체를 정렬해 비교한 값이 변경 전후로 같다. 아래 필드는 예시이고 비교는 이 목록에 한정되지 않는다. `hits`(`id`, `kind`, `rank`, `score`, `title`, `snippet`, `placeholder`), `neighbors`(`id`, `kind`, `hops`, `via`, `from`, `placeholder`, `refs`), `edges`, `counts`, `index`(`nodes`, `edges`, `rebuilt`, `unreadStores`, `inMemory`), `trace`의 `chain` 항목(`id`, `kind`, `role`, `version`, `at`, `sha`, `archived`, `edges`)과 `counts`, `resolvedAs`다. 종료 코드와 경고 코드도 같다. 새 옵션, 새 필드, 새 코드는 없다.
- I2 ==**색인 파일은 서로 호환된다.**== 같은 버전 문자열로 변경 전 코드가 만든 `.canon/index/search.db`를 변경 후 코드가 `rebuilt: false`로 다시 쓰고, 그 반대도 같다. 지문의 입력(`INDEX_FORMAT`, 버전, `canon.yaml`, 버전마다 종류·키·다이제스트·현재 여부, 파일 해시, 매달린 참조)과 순서는 그대로다. 색인을 새로 만들면 `node`, `edge`, `item`, `link`, `meta`의 행이 변경 전후로 같다.
- I3 ==**보관함 버전의 본문은 남지 않는다.**== 색인을 쓰는 실행은 최신이든 새로 만들든 보관함의 버전을 하나씩 읽고 지문과 항목에 필요한 값만 남긴 뒤 본문을 버린다. 남기는 값은 종류, 키, 다이제스트, 시각과 검토 고리가 읽는 필드(`pin`, `review_id`, `evidence_sha`, `reviewed_sha`, `introduced_by`, `resolution_sha`, `disposition`, `change_id`, `round`, `seq`, `scenario_id`, `project_id`, `scenario_ids`)다. 항목은 이 값들만 든다. 이 값들이 연결 규칙(`round_of`, `covers`, `about_same_scenario`)과 항목표의 열이 읽는 전부다.
- I4 ==**현재 버전의 본문은 한 벌이다.**== `search`와 `trace`가 읽는 원장 기록은 프로세스 전역 파일 캐시와 소유한 복제로 두 벌이 되지 않고, 색인을 만들 때 항목마다 다시 복제되지 않는다. 같은 프로세스에서 한 종류를 여러 번 스캔하는 명령(`status`, `gate check`, 쓰기 뒤의 상태)은 지금처럼 캐시를 쓰고, 캐시의 동작과 `GitTier::read`, `read_shared`의 결과는 바뀌지 않는다. 저장소 크레이트가 캐시를 거치지 않고 한 번에 하나씩 방문하는 읽기를 새로 내놓는다. 이름과 모양은 구현이 정한다.
- I5 ==**실패 경로는 그대로다.**== 방문 읽기는 스캔과 같은 규칙으로 파일을 다룬다. 검증을 통과하지 못한 파일(위반)은 읽은 기록에 들지 않고 실행을 실패시키지 않는다. 심볼릭 링크(종류 폴더나 파일)는 따라가지 않고 지금처럼 거절한다. 종료 코드 1, `SEARCH_INDEX_FAILED`이고 건너뛰고 성공하지 않는다. 읽지 못하는 파일이나 폴더(권한, I/O 오류)도 같다. 읽지 못하는 파일을 건너뛰고 성공하는 일은 없다(임시 계측 빌드는 건너뛰었으나 이 변경은 그렇게 하지 않는다). 색인이 깨졌을 때(`SEARCH_INDEX_REBUILT`)와 폴더를 쓸 수 없을 때(`SEARCH_INDEX_IN_MEMORY`)의 동작, 거절하는 `trace`가 색인 파일을 만들거나 고치지 않는 규칙은 그대로다. 색인을 메모리에 만드는 경로도 파일에 만드는 경로보다 본문을 더 들지 않는다.
- I6 ==**메모리 목표.**== 변경이 들어간 릴리스 빌드를 샌드박스에서 `/usr/bin/time -v`로 한 번 잰다. 대상은 canon 자기 저장소다. `search approval --json`은 색인이 최신일 때 150MB 이하(지금 594MB), 색인이 없을 때 250MB 이하(지금 890MB)이고, `trace <sha> --json`은 색인이 최신일 때 150MB 이하(지금 612MB)다. `status`와 `gate check`는 바뀌지 않는다. 잰 값은 이 변경의 증거 메모와 CHANGELOG에 한 줄로 남긴다. 바이트 수를 단언하는 시험은 CI에 두지 않는다.
- I7 새로 쓰거나 고치는 시험은 구조화된 판정만 단언한다. 같은 입력의 두 읽기가 같은 버전 목록(종류, 키, 다이제스트, 시각, 현재 여부를 정렬한 중복 허용 목록)을 내는가, 검색과 추적 보고서의 필드가 기대한 값인가, 읽지 못하는 파일이 종료 코드 1을 내는가를 본다. 오류 문구, 안내 문장, 메시지 조각은 단언하지 않는다. 기존 `search`와 `trace` 시험은 고치지 않고 통과해야 한다.

## Checks

- ==**전후 비교가 같다.**== 변경 전 커밋과 변경 후 커밋을 같은 버전 문자열로 릴리스 빌드해 canon 자기 저장소의 같은 체크아웃에서 질의 집합을 돌리고 `jq -S .data`의 sha256, 종료 코드, 정렬한 경고 코드(봉투의 `meta.warnings`)를 비교한다. 질의는 `search approval`, `search timeout --kind finding`, `search approval --kind brief`, `search ab`(두 글자 경로), `search approval --expand 2 --limit 5`, 아무것도 맞지 않는 `search zzzzqqqq`, `trace <저장소 증거의 evidence_sha 하나>`, `trace <finding 키 하나>`, `trace <review id 하나>`다. 각 질의를 색인이 최신일 때와 `.canon/index`를 지운 뒤에 돌린다. 모두 같아야 하고, 지운 뒤의 `search.db`는 `node`, `edge`, `item`, `link`, `meta` 표의 정렬한 행이 전후로 같다.
- ==**색인이 서로 호환된다.**== 변경 전 빌드가 만든 `search.db`로 변경 후 빌드의 `search approval --json`을 돌리면 `index.rebuilt`가 false이고, 그 반대도 같다.
- ==**메모리 목표를 한 번 쟀다.**== I6의 세 명령을 변경 후 릴리스 빌드에서 한 번씩 돌려 값을 증거 메모에 적는다. 목표를 못 맞추면 어느 단계(캐시, 복제, 항목)가 남았는지 적고 이 변경에서 고친다.
- ==**방문 읽기의 시험.**== 보관함에 대체된 버전이 있는 작은 저장소에서 방문 읽기와 `GitTier::read`가 같은 버전 목록(종류, 키, 다이제스트, 시각, 현재 여부를 정렬한 중복 허용 목록)을 낸다. 위반 파일이 하나 있는 보관함에서도 두 읽기가 같은 목록을 내고 실행이 성공한다. 읽을 수 없는 보관함 파일은 종료 코드 1이다.
- ==**항목 시험.**== 한 finding이 두 번 고쳐진 저장소에서 `trace`의 연결(`re-review`, `fix-reviewed`, `fix-of-fix`, `evidence-at-commit`)이 본문 전체를 든 항목을 쓰던 때와 같다. 기존 시험이 이미 덮으면 그 시험이 통과하는 것으로 갈음한다.
- 같은 커밋에서 `just lint`, `canon-store`와 `canon-cli`의 `search`·`trace` 시험, `canon gate check`가 통과한다. chore라서 시나리오를 더하지 않는다. 바뀐 코드의 시험 묶음을 `canon evidence add --all-scenarios`로 묶고 변경 단위 검토 한 건(`canon review change`)을 남긴다.

## Out of scope

- `status`, `gate check`, `query`의 메모리. 이들은 공유 읽기를 이미 쓰고 113MB다.
- 실행 시간. 색인이 최신이어도 보관함 파일 39,983개를 읽어 다이제스트를 구하므로 4초가 남는다. 파일 이름의 다이제스트 부분이나 파일 시각으로 지문을 바꾸면 이 읽기를 없앨 수 있으나 `INDEX_FORMAT`과 "내용으로만 해시한다"는 설계가 바뀐다. 이 변경은 지문을 그대로 둔다.
- 보관함의 보존과 정리(증거 버전 39,006개, 디스크 162MB). 어떤 버전을 얼마나 두는지는 보존 정책의 일이다.
- 파싱된 JSON의 크기. 현재 버전도 본문 대비 몇 배다. 형식 있는 기록으로 읽는 일은 따로 한다.
- 프로세스 전역 파일 캐시를 없애거나 한도를 두는 일. `status`와 `gate check`가 쓴다.
- macOS에서 다시 재는 일. 이 변경의 값은 Linux 릴리스 빌드에서 잰다. 0.41 macOS의 725MB는 같은 코드 경로의 다른 측정이다[INFERENCE: 0.43 Linux 값이 594MB다].

## Questions and assumptions

- 결정(목표, owner): 합격선은 `search approval --json` 색인 최신 150MB 이하, 색인 없음 250MB 이하, `trace <sha> --json` 색인 최신 150MB 이하다. 임시 계측 빌드가 보관함 변경 하나로 118MB를 냈다(현재 버전 두 벌이 남은 상태). 색인을 새로 만드는 경로는 SQLite의 쓰기 캐시가 더해지므로 넉넉히 잡았다[INFERENCE: 이 경로는 잰 적이 없다].
- 결정(범위, owner): 메모리만 줄인다. 지문 방식과 `INDEX_FORMAT`은 바꾸지 않고 실행 시간 4초는 이 변경 밖이다.
- 가정(구조): 저장소 크레이트에 방문 읽기를 더하는 일은 canon 안 크레이트의 공개 API 추가이고 기존 호출은 그대로다. 임시 계측 빌드는 `read_at`으로 파일을 훑었고, 정식 구현은 스캔과 같은 순회와 검증을 재사용한다.
- 가정(데이터): 잰 값은 canon 자기 저장소 하나다. 보관함이 더 큰 저장소는 더 크게 줄고 보관함이 없는 저장소는 거의 바뀌지 않는다.

Approved: journey 2026-11-18T08:38:13Z — "오너가 채팅에서 'search 수정도 같이하자'로 요청하고 질문 2개를 ask로 답함(2026-11-18). 승인은 오케스트레이터에게 위임. 독립 계획 리뷰 1라운드 approve(P0/P1 없음), P2/P3 반영. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 93bd33395620d15d38198ac8e92c5749c2369b57) (brief sha256:f092bda8e02f5a1c476da7f90e783e0d574b3f8edf20a97a98c6970cd7e6dcd2)
