---
waive:
  - rule: "prose.code-citation"
    section: "flow"
    reason: "흐름의 명령 이름과 설정 키는 사용자가 치는 철자 자체다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "git 설정 키와 경로 이름은 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "git 명령과 옵션, 색인 표지, 함수 이름은 구현자와 시험이 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "git 설정과 함수 이름은 시험이 확인하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "속성 이름과 pathspec 모양은 철자 자체다"
  - rule: "prose.contrast"
    section: "detail"
    reason: "버린 설계(복사만 하는 길)와 고른 설계의 차이가 실험 결과의 핵심이다"
  - rule: "prose.key-sentence"
    section: "detail"
    reason: "하위 절이 여섯이고 절마다 핵심이 다르다. 이어받는 길의 핵심 한 문장만 표시한다"
  - rule: "prose.length"
    section: "brief"
    reason: "이어받는 길, 돌아가는 조건 표, 제외 pathspec, 실험 결과를 한 길로 묶는 변경이라 합이 4000자를 넘는다"
  - rule: "prose.length"
    section: "intent"
    reason: "잰 시간 두 줄이 근거다"
  - rule: "prose.length"
    section: "detail"
    reason: "돌아가는 상태 열넷과 그 실험 결과를 한 곳에 적어야 구현자가 상태를 다시 찾지 않는다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "상태 행렬이 시험의 입력이라 철자를 모두 적는다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정마다 버린 설계와 실험 근거를 달았다"
  - rule: "prose.paragraph-length"
    section: "detail"
    reason: "⑭의 이유 하나를 이어서 말한다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 잰 저장소의 크기와 값의 출처다"
  - rule: "prose.parenthetical"
    section: "outcome"
    reason: "괄호는 설정 키와 제외 경로의 예다"
  - rule: "prose.parenthetical"
    section: "flow"
    reason: "괄호는 돌아가는 상태의 예다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 소스 줄 번호와 잰 값이다"
  - rule: "prose.parenthetical"
    section: "acceptance"
    reason: "괄호는 상태 번호와 소스 줄 번호다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "괄호는 잰 id와 설정 키, 버린 설계의 재현이다"
  - rule: "prose.sentence-length"
    section: "flow"
    reason: "돌아가는 상태의 열거가 한 문장이다"
  - rule: "prose.sentence-length"
    section: "detail"
    reason: "git 명령과 상태의 열거가 한 문장이라 80자를 넘는다"
  - rule: "prose.sentence-length"
    section: "acceptance"
    reason: "시험 상태의 열거가 한 문장이라 80자를 넘는다"
---

# canon의 코드 트리 계산은 저장소의 색인을 이어받아 바뀐 파일만 해시한다

Source: Buzz에서 동료 세션이 전한 관찰(`brief approve`, `status`, `evidence add`가 부하 아래서 한 번에 2~3분 `.canon/repo-write.lock`을 쥔 채 기다렸다, 명령 다섯 개가 줄을 섰다), 0.40.0 소스(`crates/canon-cli/src/git_rev.rs:236-275`, `:294-312`, `crates/canon-store/src/git.rs:189-221`, `:284-298`), git 2.56.0으로 잰 값과 실험(`/Volumes/External/tmp/canon-0.41/exp`, 임시 저장소 수십 개)

## Intent

코드 트리를 계산하는 모든 쓰기 명령은 임시 색인에 `git read-tree HEAD`를 한 뒤 `git add -A -- .`를 돈다. `read-tree`가 만든 색인에는 파일의 stat 정보가 없어서 `add -A`가 추적 파일 전부를 다시 읽고 해시한다. canon 저장소(추적 파일 41,796개, 1.2 GB)의 체크아웃에서 잰 값은 이렇다.

- 지금 길: `read-tree HEAD` 0.19초, `add -A` 27.07초(처음 잰 값은 11.13초였고 디스크와 부하에 따라 달라진다).
- 저장소 색인을 복사해서 이어받는 길: `ls-files -v` 0.08초, `read-tree --reset HEAD` 0.14초, `add -A` 0.34초, `write-tree` 0.15초. 파일 200개짜리 저장소에서 `add -A`가 clean 필터를 부른 횟수는 지금 길이 207번, 이어받는 길이 2번(바뀐 파일과 새 파일)이었다.

시간은 변경 파일 수에 맞고 저장소 크기에 맞지 않는다. 이 `add -A`는 쓰기 잠금을 쥔 채 도는 탓에 느린 명령 하나가 뒤의 명령 전부를 세운다.

## Desired outcome

- ==**코드 트리 id는 0.40과 한 비트도 다르지 않고, 바뀐 파일만 해시한다.**==
- 색인을 이어받을 수 없거나 이어받은 id가 0.40과 달라질 수 있으면 0.40의 길로 돌아가 같은 id를 낸다. 앞의 것은 건너뛰기 표지가 붙은 색인과 읽을 수 없는 색인이고, 뒤의 것은 변환 규칙이 있는 저장소와 stat 검사를 약하게 한 설정이다. 거절하지 않는다.
- 임시 색인으로 도는 git 호출은 저장소의 `.git`에 파일을 만들지도 지우지도 않는다(공유 색인의 mtime은 git이 읽을 때 갱신한다). 분할 색인(`core.splitIndex`)을 쓰는 저장소에서 0.40이 `.git/sharedindex.*`를 새로 만들고 사용자의 공유 색인을 지울 수 있던 것(잰 값)도 멈춘다.
- 커밋의 코드 트리(`read-tree <sha>`), 0.40의 git 환경 허용 목록과 다이제스트는 그대로다.

## User flow

1. F1 사람이나 에이전트가 큰 저장소에서 `brief approve`, `status`, `evidence add`, `gate check`처럼 작업 트리의 코드 트리를 계산하는 명령을 돌린다 → canon은 저장소 색인의 복사본으로 시작해 바뀐 파일만 해시한다 → ==**같은 작업 트리에서 0.40이 낸 id를 몇 초 안에 낸다.**==
2. F2 색인이나 저장소가 이어받기로 같은 답을 보장할 수 없는 상태다(건너뛰기·가정 표지, 색인 없음, 아직 커밋 없음, 읽을 수 없는 색인 복사본, `.gitattributes`나 변환 설정, stat 검사를 약하게 한 설정) → canon은 0.40의 길(`read-tree HEAD`, `add -A`)로 돌아간다 → id는 같고 시간만 0.40만큼 든다.
3. F3 `core.splitIndex=true`이거나 색인이 분할된 저장소에서 같은 명령을 돌린다 → 같은 id가 나오고 `.git`에는 파일이 생기지 않는다.
4. F4 사람은 CHANGELOG와 설계 문서를 읽는다 → 이어받는 조건, 돌아가는 조건, 알려진 한계가 적혀 있다.

## Step detail

### F1 이어받는 길

==**이어받은 색인은 0.40의 `read-tree HEAD`가 만드는 출발점과 같은 항목만 남기고, 그 항목의 stat만 가져온다.**==

- 대상은 작업 트리의 코드 트리를 만드는 두 곳이다: `code_tree`의 `build(None)`(`git_rev.rs:251-256`)과 `TreeDiffer::new`(`:303-309`). 커밋의 길(`build(Some(sha))`, `changed_since`의 `read-tree <sha>`)과 `dirty`를 재는 두 번째 `build`는 그대로다. `filtered_tree`와 `code_exclude`도 그대로다.
- 이어받을 때의 순서(HEAD가 있을 때만):
  1. 저장소 색인의 경로를 `rev-parse --path-format=absolute --git-path index`로 얻는다. `ScratchIndex::open`이 객체 디렉터리를 얻는 호출(`git.rs:210`)에 `--git-path index`를 더해 한 번에 얻고, 값을 `ScratchIndex`가 들고 있게 한다. 연결된 작업 트리는 자기 색인(`.git/worktrees/<이름>/index`)을 갖고, 잰 값으로 주 작업 트리의 스테이징은 새지 않았다. 0.40의 허용 목록이 `GIT_INDEX_FILE`을 이미 지우므로 이 경로는 저장소가 정하고 셸은 정하지 못한다.
  2. 색인 파일을 한 번만 열고 그 핸들의 metadata에서 mtime을 읽은 뒤 같은 핸들에서 임시 디렉터리의 색인 자리로 복사하고, `File::set_modified`로 복사본에 그 시각을 준다. 경로로 다시 읽지 않는다. 그 사이 다른 git이 새 색인을 rename으로 놓으면 옛 바이트에 새 mtime이 짝지어져 racily-clean 항목이 깨끗한 것으로 통과한다(부하 아래서 명령이 줄을 서는 이 변경의 상황에서 일어난다). 복사본의 mtime을 다시 읽어 원본보다 크면(시각을 올림하는 SMB나 FAT의 임시 디렉터리) 돌아간다. git은 색인 항목의 mtime과 색인 파일의 mtime을 견주어 "같은 순간에 고쳐진 파일"을 다시 읽는다. 복사본이 새 시각을 갖는 순간 이 방어가 사라진다(1초 단위 시계에서 재현한 리뷰의 측정: 옳은 id 대 HEAD의 id).
  3. 복사본에서 `ls-files -v -z`(`tree_listing`)를 돌려 표지를 읽는다. `H`(정상)와 `M`(병합 안 된 항목) 말고 다른 표지가 하나라도 있으면 돌아간다. `S`(건너뛰기 표지, 희소 체크아웃)와 `h`(가정 불변)가 그 표지다. 이 호출은 복사본이 읽히는지도 확인한다.
  4. 복사본에 `read-tree --reset HEAD`를 돈다. HEAD와 다른 항목(스테이징한 수정, 스테이징한 새 파일, 색인에서만 지운 파일)은 HEAD의 것으로 바뀌고, HEAD와 같은 항목은 stat을 그대로 둔다. 0.40은 HEAD에서 시작하므로 이 단계가 같은 출발점을 만든다.
  5. `add -A -- .`를 돌고 `filtered_tree`로 넘어간다.
- 임시 색인을 쓰는 모든 호출(`ScratchIndex`를 받는 호출)에 `-c core.splitIndex=false`를 준다(F3). 이어받는 길과 돌아가는 길, `filtered_tree`의 `ls-files`와 `update-index`, `write-tree`가 모두 같다. 시나리오 없이 `GitCall`에 대고 하는 호출(스크래치 없는 호출)의 명령줄은 0.39·0.40과 같아 `git.rs:499`의 시험이 그대로 통과한다.
- 이어받은 뒤 단계 2~5 가운데 어느 git 호출이 실패하면, 임시 색인을 지우고 0.40의 길을 한 번 돈다. 보고되는 실패는 돌아간 길의 것이다. 돌아간 길도 실패하면 지금의 `COMMIT_REQUIRED`(`git could not compute the code tree: …`)다.
- 이어받아도 stat이 바뀐 파일(`touch`, 브랜치 전환)은 해시한다. canon은 저장소의 색인을 쓰지 않으므로 사용자의 `git status`가 stat을 새로 쓰기 전에는 그 파일이 명령마다 다시 읽힌다. 비용은 바뀐 파일 수에 비례한다.

### F2 돌아가는 조건

실험은 아래 상태마다 0.40의 길로 얻은 트리와 이어받은 길로 얻은 트리를 견주었다(`git ls-files -s`의 md5까지 같음). 복사본만 쓰고 `read-tree --reset`을 건너뛴 길은 ①②에서 달랐다.

| # | 상태 | 이어받은 길 | 이유 |
|---|---|---|---|
| ① | `git add -f`로 스테이징한 무시된 파일(아직 커밋 안 함) | `--reset`이 지운다, 같음 | 0.40은 HEAD에서 시작해 이 파일이 없다 |
| ② | 무시 규칙에 걸리는 추적 파일을 `git rm --cached`로 색인에서 뺀 상태 | `--reset`이 되살린다, 같음 | 위와 같다 |
| ③ | `--skip-worktree` 표지가 붙은 파일을 고친 상태 | 돌아간다 | 이어받으면 `add -A`가 건너뛰어 HEAD의 내용이 나오고 0.40은 고친 내용을 해시한다(달랐다) |
| ④ | `--assume-unchanged` 표지가 붙은 파일을 고친 상태 | 돌아간다 | 위와 같다(달랐다) |
| ⑤ | 희소 체크아웃(non-cone, cone, 희소 색인) | 돌아간다(표지 `S`) | ③과 같은 표지다. 희소 체크아웃은 파일이 적어 0.40의 길도 싸다 |
| ⑥ | 병합 충돌(색인에 단계 1~3 항목), 충돌 파일을 지운 상태, 손으로 푼 상태 | 이어받는다, 같음 | `--reset`이 병합 안 된 항목을 버린다 |
| ⑦ | `git add -N`한 파일, 그 파일을 지운 상태, 스테이징한 이름 변경, 작업 트리에서 지운 추적 파일 | 이어받는다, 같음 | |
| ⑧ | 서브모듈 gitlink가 새 커밋을 가리킴, 서브모듈 안만 더럽힘, 파일이 심볼릭 링크로 바뀜, 실행 비트 | 이어받는다, 같음 | |
| ⑨ | untracked cache 확장이 든 색인, `fsmonitor-valid` 표지, `core.fsmonitor=true` 설정 | 이어받는다, 같음 | canon은 `core.fsmonitor=false`를 주어 표지가 꺼지고, 새 추적 안 하는 파일이 이미 훑은 디렉터리에 생겨도 `add -A`가 본다 |
| ⑩ | 색인 파일 없음 | 돌아간다 | 복사가 실패한다 |
| ⑪ | 커밋이 아직 없음(스테이징한 파일 있음·없음) | 돌아간다 | 되돌릴 HEAD가 없고 이런 저장소는 작다 |
| ⑫ | 색인이 비었거나 잘렸거나 `sharedindex.*`가 사라진 분할 색인 | 돌아간다 | `ls-files -v`가 실패한다(사용자의 `git status`도 실패한다) |
| ⑬ | 색인의 바이트 하나를 뒤집은 복사본 514가지(색인 514바이트, 바이트마다 한 번) | 18가지는 `ls-files -v`가 거절해 돌아가고 496가지는 이어받아 전부 0.40과 같은 트리 | `--reset`이 HEAD와 다른 항목을 HEAD 것으로 바꾸고, 뒤집힌 stat이 우연히 파일과 맞는 일은 없다. 그래서 색인의 체크섬을 따로 검증하지 않는다 |
| ⑭ | 변환 규칙이 있다: 색인·HEAD·`add -A` 뒤의 목록 어디에든 `.gitattributes`가 있다, `.gitignore`가 가린 추적 안 하는 `.gitattributes`가 있다(`ls-files -z -o -i --exclude-standard -- ':(glob)**/.gitattributes'`), `.git/info/attributes`가 비지 않았다, `git var GIT_ATTR_SYSTEM`이나 `git var GIT_ATTR_GLOBAL`이 가리키는 파일이 비지 않았다, `core.autocrlf`·`core.eol`·`core.attributesFile`이 설정돼 있다 | 돌아간다 | 아래 |
| ⑮ | stat 검사가 약하다: `core.trustctime=false` 또는 `core.checkStat=minimal`(`git config`가 읽는 값, 허용 목록이 통과시키는 `GIT_CONFIG_GLOBAL`·`GIT_CONFIG_SYSTEM` 포함) | 돌아간다 | 같은 크기로 다시 쓰고 mtime을 되돌린 파일(`touch -r`, `cp -p`, `rsync -t`, `tar`)이 stat이 같아 보여 이어받은 길이 HEAD의 내용을 낸다. 잰 값: 0.40 `51e27260…`, 이어받은 길 `d1cb4777…`(HEAD의 트리, 고친 코드가 숨는다). `checkStat=minimal`은 inode도 안 보므로 rename으로 덮는 도구도 못 본다. 기본 설정에서는 ctime이 달라 같은 재작성이 잡힌다 |

⑭의 이유: git은 stat이 같은 항목을 다시 변환하지 않는다. 항목의 stat을 적은 뒤에 변환 규칙이 바뀌면 이어받은 길은 옛 규칙의 blob을 그대로 쓰고 0.40은 지금 규칙으로 새로 해시한다. `.gitattributes`와 `filter.up.clean=tr a-z A-Z`를 stat이 깨끗한 저장소에 나중에 더한 실험에서 두 트리가 달랐다(`b4085e08…` 대 `c1686d43…`). `.gitignore`가 가린 새 `.gitattributes`(0.40 `7a12e6cc…`, 이어받은 길 `d8973ce8…`)와 `core.attributesFile`(0.40 `0325b92e…`, 이어받은 길 `d1cb4777…`)도 같았다. 위 조건이 있으면 돌아가 막고, 돌아간 길은 이 실험들에서 0.40과 같은 id를 냈다. `ignored` 목록을 읽는 호출은 무시된 큰 트리(`target/`)를 훑으므로 해시는 아니어도 읽기 비용이 든다(이 저장소에서 0.41초).

- `.gitattributes`는 이름만 본다(내용을 읽지 않는다). 변환과 상관없는 속성만 담은 파일도 돌아가게 한다. 어디에 있든 그렇다(제외 경로 안의 것도). 단순하고 틀리지 않는 쪽이다.
- `filter.*` 설정만 있는 경우(이 기계의 전역 설정에는 git-lfs의 `filter.lfs.*`가 있다)는 돌아가지 않는다. 속성이 그 필터를 가리키지 않으면 어떤 파일도 변환되지 않는다.
- 알려진 한계: 항목을 적은 뒤에 변환 규칙이 사라진 경우(전역 `core.autocrlf`나 전역 속성 파일을 지움)는 막지 못한다. 그 항목은 옛 규칙으로 변환된 blob을 갖고 있다. 이어받은 id는 항목을 적던 때의 id로 남고 0.40은 원본 바이트로 새 id를 낸다(잰 값: 필터를 전역 속성 파일로 주고 커밋한 뒤 파일을 지움, `git status`는 깨끗하다, 이어받은 id는 HEAD의 `898a958f…`, 0.40은 `18eb80fb…`). 바이트가 바뀐 파일은 stat이 달라져 다시 해시되므로 이 한계가 코드의 변화를 숨기지는 않는다. 어긋나는 방향은 규칙만 바뀐 작업 트리가 새 증거를 요구하지 않는 쪽이고, `git status`가 말하는 작업 트리와 같다.

### F3 `.git`에 쓰지 않는다

- 0.40의 `ScratchIndex`는 "저장소를 읽기만 하고 아무것도 쓰지 않는다"고 말하지만(`git_rev.rs:57-60`, `git.rs:186-187`), `core.splitIndex=true`이면 임시 색인에 쓰는 `add`와 `write-tree`가 `.git/sharedindex.<해시>`를 새로 만든다(잰 값: 색인이 분할되지 않은 저장소에서 파일 1개, 분할된 저장소에서 1개 더). 더 나쁜 경우도 잰 값으로 있다: `splitIndex.sharedIndexExpire=now`(기본은 2주)이고 새 공유 색인이 필요해지면 그 쓰기가 사용자의 색인이 가리키는 `.git/sharedindex.<해시>`를 지우고, 사용자의 다음 `git status`가 `index file open failed`로 실패한다. 이어받는 길은 분할 색인을 그대로 복사하므로 같은 일이 더 쉽게 일어난다.
- 고침: 임시 색인을 받는 모든 호출에 `-c core.splitIndex=false`를 준다. 잰 값: 이렇게 하면 분할 색인 복사본도 같은 트리를 내고(공유 색인은 읽기만 한다) `.git`의 파일 목록이 호출 전후로 같다. 호출 하나라도 빠지면 새는 것을 확인했다(`write-tree`만 빠져도 샌다). 그래서 호출마다 붙이지 않고 `ScratchIndex`가 있으면 `GitCall::command`가 붙인다.
- 같은 "읽기만 한다"는 말이 또 하나 어긋나 있다: 0.40의 `add -A`는 저장소에 이미 있는 느슨한 객체를 신선하게 하려고 그 파일의 mtime을 바꾼다(`GIT_ALTERNATE_OBJECT_DIRECTORIES`로 보이는 객체도 그렇다). 잰 값: 느슨한 객체 62개 중 55개의 mtime이 바뀌었고, 이어받은 길은 0개였다. 이 변경은 돌아가는 길의 이 부작용을 고치지 않는다. 객체의 내용이 아니라 파일의 mtime이고, 시험은 이 mtime을 해시한 파일 수의 신호로 쓴다(Acceptance).
- 문서(`git.rs`의 맨 위 문서, `git_rev.rs`의 `code_tree` 문서)에 "임시 색인을 쓰는 호출은 `core.splitIndex=false`로 돌고 `.git`에 파일을 만들지도 지우지도 않는다. 분할 색인의 공유 색인은 git이 읽을 때 mtime을 갱신한다"와 이어받는 길을 적는다.

### F4 변경 기록

- CHANGELOG의 Changed: 이어받는 길과 시간(변경 파일 수에 비례), 돌아가는 조건(⑩~⑮), id가 달라지지 않는다는 것, 임시 색인 호출의 `core.splitIndex=false`(0.40에서 `.git/sharedindex.*`를 만들고, 만료 설정에 따라 사용자의 공유 색인을 지울 수 있던 것을 고침), 알려진 한계와 그 방향. 새 옵션, 새 환경 변수, 새 오류 코드는 없다.
- `docs/design`에서 코드 트리를 설명하는 곳과 `rust-coding-rules.md`의 git 길 규칙이 `read-tree HEAD` 뒤 `add -A`만 말하면 고친다.

## Acceptance

- F1:
  - 시험은 기대 id를 canon의 코드 없이 평범한 git으로 계산한다: 임시 `GIT_INDEX_FILE`, `read-tree HEAD`, `add -A -- .`, `.canon/`와 명세 루트와 `code.exclude`에 걸린 경로의 `update-index --force-remove`, `write-tree`. 상태는 깨끗한 저장소, 고친 추적 파일과 새 파일, 지운 추적 파일, 스테이징 뒤 다시 고침, 스테이징한 새 파일을 지움, ①, ②, ⑥의 세 상태, ⑦, ⑧, ⑨의 `fsmonitor-valid`와 untracked cache, 연결된 작업 트리(주 작업 트리에 따로 스테이징한 파일이 있어도 그 색인을 쓰지 않는다)다. 모든 상태에서 `code_tree(repo, None)`의 `tree`와 `dirty`가 기대와 같다.
  - 해시한 파일 수를 시계 없이 잰다: 느슨한 객체만 있는 저장소(`gc.auto=0`, 커밋한 파일 100개)에서 모든 느슨한 객체의 mtime을 먼 과거로 돌려 둔 뒤 파일 하나를 고치고 코드 트리를 계산해, mtime이 바뀐 객체가 하나도 없음을 확인한다. 돌아가는 상태를 같은 저장소에 만들 때는 필터가 일부 파일만 덮게 한다(`*.txt filter=up`, 변환되지 않는 커밋한 파일이 남아 있게). 모든 파일을 변환하는 필터는 모든 blob이 새것이라 아무 객체도 신선해지지 않아 돌아간 길이 0으로 보인다(리뷰에서 잰 값). 돌아간 길은 mtime이 바뀐 객체가 0보다 많고 이어받은 길은 정확히 0이어야 하며, 이것이 시험이 신호가 해시를 따라가는지 스스로 확인하는 방법이다.
  - 복사본의 mtime이 원본과 같고, 파일시스템이 시각을 올림해서 복사본의 mtime이 원본보다 크게 나오면 돌아간다(시각을 주입하는 단위 시험). 색인을 한 번만 열고 그 핸들에서 mtime을 읽는다.
  - 실패 경로: ==**이어받은 길이 틀린 트리를 낼 수 있는 상태에서는 돌아가고**==, 돌아간 id는 평범한 git의 값과 같고 HEAD의 id가 아니다. ③, ④, ⑤(희소 체크아웃), ⑭(스테이징 안 한 새 `.gitattributes`, `.gitignore`가 가린 새 `.gitattributes`, `filter.up.clean`을 stat이 깨끗한 저장소에 나중에 더함, 추적 중인 `.gitattributes`의 필터 명령을 나중에 바꿈, `core.autocrlf=true`, `.git/info/attributes`, `core.attributesFile`, HOME 아래 XDG 전역 속성 파일), ⑮(`core.trustctime=false`, `core.checkStat=minimal`, `GIT_CONFIG_GLOBAL`이 가리키는 파일에 둔 같은 설정을 각각, 커밋한 파일을 같은 크기로 다시 쓰고 `touch -r`로 mtime을 되돌린 상태)에서 id가 평범한 git과 같다. 기본 설정에서는 같은 재작성도 ctime이 달라 이어받은 길이 해시한다.
- F2:
  - ⑩(색인 지움), ⑪(커밋 없음, 스테이징 있음·없음), ⑫(빈 색인, 잘린 색인, 공유 색인을 지운 분할 색인)에서 id가 평범한 git과 같고 오류가 없다.
  - 색인의 바이트를 뒤집은 복사본으로 돌린 시험이 모든 위치(또는 고정 시드로 고른 서른 위치 이상)에서 0.40과 같은 id를 낸다(⑬).
  - 실패 경로: 복사본이 읽히지 않으면 거절하지 않고 돌아간다. 색인이 디렉터리이거나 읽기 권한이 없는 경우(복사 실패, root가 아닐 때)에도 `code_tree`는 평범한 git과 같은 id를 낸다. 돌아간 길도 실패하면(저장소가 아니거나 HEAD의 객체가 없음) 오류는 0.40과 같은 `COMMIT_REQUIRED`와 같은 메시지다. 0.40의 시험(`approval_tree.rs`, `commits.rs`, `code_scopes.rs`)이 고치지 않고 통과한다.
- F3:
  - `core.splitIndex=true`와 분할된 색인(`update-index --split-index`)과 분할되지 않은 색인의 세 저장소에서, `code_tree`와 `TreeDiffer::new`를 돈 전후로 `.git`의 파일 목록(`sharedindex.*` 포함)이 같고 id가 평범한 git과 같다.
  - `GitCall`에 `ScratchIndex`를 줄 때의 명령줄에 `-c core.splitIndex=false`가 들어 있고 주지 않을 때의 명령줄은 `git.rs:499`의 값과 같다. 환경 허용 목록(`ALLOWED_ENV`)과 `gate.git-env.*`, `gate.git-boundary.*`의 시험, 컨텍스트 팩 다이제스트의 시험이 그대로 통과한다.
  - 실패 경로: 임시 색인을 받는 호출 가운데 하나만 빠져도 시험이 실패한다. `filtered_tree`의 `ls-files`, `update-index`, `write-tree`와 `changed_since`의 `read-tree`가 그 호출이고, 시험은 `core.splitIndex=true`에서 각 호출을 돌린 뒤 `.git`에 새 파일이 없음을 본다.
  - `splitIndex.sharedIndexExpire=now`와 작은 `splitIndex.maxPercentChange`로 새 공유 색인이 필요해지는 분할 색인 저장소(수정 파일 여러 개 스테이징)에서 `code_tree`를 돈 뒤 사용자의 `git status`가 성공하고 `.git`의 파일 목록이 같다. 0.40에서는 이 상태가 `.git/sharedindex.<해시>`를 지워 `git status`가 `index file open failed`로 실패했다(잰 값). 공유 색인의 mtime은 git이 읽을 때 갱신하므로 시험은 파일 목록만 견주고 mtime을 견주지 않는다.
- F4:
  - CHANGELOG에 항목이 없거나 ⑭·⑮의 조건, 알려진 한계(그 방향 포함), `core.splitIndex`, 0.40이 분할 색인 저장소에서 사용자의 공유 색인을 지울 수 있었다는 말이 빠지면 릴리스 확인 시험이 실패한다(0.39의 `git_boundary.rs:499,525` 방식).
  - 실패 경로: `rust-coding-rules.md`나 `git_rev.rs`의 문서가 `read-tree HEAD` 뒤 `add -A`만 말하면 같은 시험이 실패한다.
- `cargo test --workspace`, `just atip-deep`, `canon gate check`가 통과한다.

## Questions and assumptions

- 결정: 이어받기는 `read-tree --reset HEAD`로 출발점을 0.40의 `read-tree HEAD`와 같게 맞춘다. 복사본만 쓰는 길은 ①②에서 틀린 트리를 냈다(실험).
- 결정: 병합 안 된 항목(`M`)은 돌아가지 않는다. 세 상태 모두 같은 트리를 냈고, 병합 충돌 중이 canon을 가장 자주 돌리는 때다.
- 결정: HEAD가 없으면 이어받지 않는다. 되돌릴 트리가 없고 첫 커밋 전의 저장소는 작다.
- 결정: 색인의 체크섬은 따로 검증하지 않는다. 바이트 뒤집기 514가지에서 틀린 트리는 없었다(⑬). 복사본이 읽히는지는 `ls-files -v`가 본다.
- 결정: 분할 색인은 돌아가지 않고 이어받는다. `core.splitIndex=false`를 모든 임시 색인 호출에 주면 같은 트리를 내고 `.git`에 쓰지 않았다(F3). 공유 색인이 사라진 경우만 읽기에 실패해 돌아간다.
- 결정: 변환 규칙이 있는 저장소는 돌아간다(⑭). 같은 id를 지키는 길이 이것뿐이다. 대신 이 저장소들은 빨라지지 않는다.
- 결정: 새 옵션, 새 환경 변수, 새 오류 코드는 없다. 이어받기를 끄는 스위치도 두지 않는다(돌아가는 조건이 안전을 맡는다).
- 결정: 되돌리는 방법은 한 줄이다: 이 변경을 되돌리면 0.40의 길이 돌아온다. 저장소 데이터를 쓰지 않는 변경이라 사전 검증을 더 하지 않는다.
- 결정: 이 변경은 코드 트리를 명령 안에서 재사용하는 일(같은 명령이 `bind`, `code_tree_if_committed`, `CodeTreeMemo`로 두세 번 계산하는 것)을 건드리지 않는다. 한 번의 비용이 초에서 분 단위로 줄면 중복은 문제가 되지 않는다.
- 질문: `.gitattributes`가 있는 저장소(LFS를 쓰는 곳이 대표다)도 이어받게 하는가. 기본은 아니다(돌아간다). 그 저장소가 빨라지려면 변환 속성(`text`, `eol`, `filter`, `ident`, `working-tree-encoding`)이 있는 파일만 골라 돌아가게 하는 조건이 필요하고, 속성을 제거한 경우를 걸러낼 수 없다는 한계가 남는다. 승인하면 후속 변경이 이것을 한다.
- 결정: 제외 pathspec(F4 초안)은 이 변경에서 뺀다(리뷰 1라운드, 사용자가 위임한 결정). 그 초안은 `add -A`를 거절시키는 모양(`docs`나 `.canon`이 심볼릭 링크, 서브모듈 안의 패턴, 앞에 `/`가 붙은 패턴, `../out`)과 id를 조용히 바꾸는 모양(`./src`, `a/../src`)을 냈고, 이어받기만으로 보고된 병목이 사라진다. 후속 변경의 조건은 Out of scope에 적었다.
- 결정: id가 0.40과 바이트까지 같다는 불변식을 지킨다. 코드의 변화를 숨길 수 있는 차이가 생기는 상태는 모두 돌아간다. 거기에 stat 검사를 약하게 하는 설정(`core.trustctime=false`, `core.checkStat=minimal`)이 든다(⑮). 설정은 `git config`로 읽어 git의 우선순위를 따른다.
- 결정: 알려진 한계로만 두는 것은 하나다: 항목을 적은 뒤에 변환 규칙이 사라진 경우. 이어받은 id는 항목을 적던 때의 id(규칙 아래서 변환된 blob)로 남고 0.40은 원본 바이트로 새 id를 낸다(잰 값: `898a958f…` 대 `18eb80fb…`, `git status`는 깨끗하다고 한다). 바이트가 바뀐 파일은 stat이 달라져 다시 해시되므로(약한 stat 설정은 돌아간다) 코드의 변화를 숨기지 않는다. 틀리는 쪽은 규칙만 바뀌었을 때 새 증거를 요구하지 않는 쪽이다.
- 결정: 시스템·전역 속성 파일은 손으로 경로를 짜지 않고 `git var GIT_ATTR_SYSTEM`과 `git var GIT_ATTR_GLOBAL`이 말하는 파일을 본다. 둘 중 하나라도 비지 않으면 돌아간다. 이 두 변수는 `core.attributesFile`과 XDG·HOME을 git이 보는 대로 따른다. 이 기계의 시스템 파일(`/opt/homebrew/etc/gitattributes`)은 없고 전역 파일(`~/.config/git/attributes`)도 비어 있다.

## Out of scope

- 저장소의 색인을 canon이 새로 쓰는 일(`git update-index --refresh`를 대신 돌리는 것). canon은 저장소에 쓰지 않는다.
- 변환 규칙이 있는 저장소를 빠르게 하는 일(위 질문).
- 코드 트리를 명령 안에서 한 번만 계산하게 하는 메모이제이션.
- 커밋의 코드 트리(`read-tree <sha>`)와 `changed_since`의 옛 트리 계산.
- 0.40의 git 환경 허용 목록, 읽는 출력의 모양 고정, 다이제스트.
- 큰 저장소를 위한 다른 git 기능(`core.untrackedCache`, `feature.manyFiles`, fsmonitor 데몬)을 canon이 켜는 일.
- 제외 pathspec으로 `add -A`가 읽는 경로를 줄이는 일(`.canon/`, 명세 루트, `code.exclude`의 패턴을 `:(exclude…)`로 넘기는 것). 큰 추적 안 하는 제외 트리가 있을 때만 이득이라 이 변경에서 뺐다. 실험은 20,000개 파일 `docs/gen/`에서 18.1초가 0.04초가 됨을 보였다. 후속 변경이 하려면 1라운드 리뷰의 재현을 규칙으로 가져와야 한다. 앞뒤의 `/`를 `Glob::new`처럼 벗긴다. 빈, `.`, `..` 조각이 든 패턴으로는 pathspec을 만들지 않는다(`./src`와 `a/../src`는 git이 `src/`로 접어 stale한 id를 조용히 냈다). `docs`나 `.canon`이 심볼릭 링크이거나 패턴이 서브모듈 안을 가리키면 `add -A`가 `beyond a symbolic link`, `is in submodule`로 죽으니, pathspec이 있을 때 `add -A`가 실패하면 pathspec 없이 한 번 다시 돈다.
