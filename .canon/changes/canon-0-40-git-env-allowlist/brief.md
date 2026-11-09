---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "환경 변수 이름, 설정 키, git 옵션은 호출자와 시험이 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "환경 변수, 설정 키, 오류 코드의 이름은 시험이 확인하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "버린 설계를 재현하는 설정 키와 변수 이름은 철자 자체다"
  - rule: "prose.length"
    section: "detail"
    reason: "허용 이름, 호출처별 고정 옵션, 건드리지 않는 설정을 한 곳에 적어야 구현자가 목록을 다시 찾지 않는다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "환경 변수와 설정 행렬이 시험의 입력이라 철자를 모두 적는다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정 열두 줄과 실패 방식 셋이 버린 설계마다 이유를 달았다"
  - rule: "prose.sentence-length"
    section: "detail"
    reason: "설정 키와 변수 이름의 열거가 한 문장이라 80자를 넘는다"
  - rule: "prose.sentence-length"
    section: "acceptance"
    reason: "설정 키와 변수 이름의 열거가 한 문장이라 80자를 넘는다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 호출처 줄 번호와 대상 환경 변수다"
  - rule: "prose.parenthetical"
    section: "acceptance"
    reason: "괄호는 시험 입력의 변수 이름이다"
  - rule: "prose.paragraph-length"
    section: "detail"
    reason: "가리지 않는 이유 하나를 이어서 말한다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "흐름의 환경 변수와 설정 이름은 사용자가 쓰는 철자 자체다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "GIT_*는 변수 군의 이름이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "잰 값의 변수와 설정 이름은 재현하는 철자 자체다"
  - rule: "prose.length"
    section: "brief"
    reason: "환경 허용 목록, 출력 모양 고정, 사용자가 보는 기록을 한 길로 묶는 변경이라 섹션마다 예산 안이어도 합이 4000자를 넘는다"
---

# canon의 git 호출은 허용한 GIT_* 변수만 받고, 읽는 출력의 모양을 설정에 맡기지 않는다

Source: canon-0-39-git-boundary의 리뷰 발견 1_2(`.canon/ledger/kind=finding/canon-0-39-git-boundary_1_2_9417eb75.json`, 열린 note), git 2.56.0으로 잰 값(임시 저장소), 0.39.0 소스(main 693c6ff)

## Intent

0.39의 git 길은 `GIT_*` 13개를 지우는 거부 목록이라 목록 밖 변수는 git에 닿는다. 더티 트리의 context pack 다이제스트는 `GIT_DIFF_OPTS=-u0`, `GIT_CONFIG_COUNT`의 `diff.context=0`, 같은 값을 둔 홈 gitconfig 어느 쪽으로도 바뀐다. git 2.56.0으로 쟀을 때 `diff.noprefix`, `color.ui=always`, `status.showUntrackedFiles=no`도 읽는 출력을 바꾸고, `diff.ignoreSubmodules=all`은 낡음 검사가 서브모듈 갱신을 놓치게 한다. `-U3`은 `GIT_DIFF_OPTS`를 이기지 못하고 `-c`는 `GIT_CONFIG_COUNT`를 이긴다.

## Desired outcome

- git 자식은 canon이 직접 정한 `GIT_*`와 허용 목록의 이름 말고는 어떤 `GIT_*`도 받지 않는다. 목록의 이름만 닿는다.
- canon이 읽거나 해시하는 diff와 status의 모양은 어느 gitconfig에도 달려 있지 않다.
- 설정이 없는 사용자에게는 아무것도 달라지지 않는다. 다이제스트도, 코드 트리도, 낡음 판정도 0.39와 같은 값이다.
- ==**canon의 git 답은 저장소가 정하고, 셸과 홈 디렉터리의 git 설정은 정하지 않는다.**==
- 자식 프로세스에만 닿는다. 사용자의 셸, 환경, gitconfig는 읽기만 하고 바꾸지 않는다.

## User flow

1. F1 에이전트나 훅은 `GIT_DIFF_OPTS`, `GIT_CONFIG_COUNT`, `GIT_CONFIG_PARAMETERS`, `GIT_TRACE`처럼 목록에 없는 `GIT_*`가 걸린 셸에서 canon을 돌린다 → 그 변수는 canon의 git에 닿지 않는다 → ==**더티 트리의 context pack id가 변수 없이 돌린 것과 같다.**==
2. F2 사람은 `diff.context=0`, `diff.noprefix`, `status.showUntrackedFiles=no`, `diff.ignoreSubmodules=all`, `color.ui=always` 같은 설정을 홈이나 저장소의 gitconfig에 둔다 → canon의 diff와 status는 git 기본 모양으로 읽는다 → context pack id와 낡음 판정이 설정 없는 저장소와 같다.
3. F3 사람은 CHANGELOG와 설계 문서를 읽는다 → 무엇이 닿고 무엇이 안 닿는지, 어떤 환경이 이제 실패하는지가 적혀 있다.

## Step detail

### F1 허용 목록

- `GitCall::command`(`git.rs:227`)는 `CLEARED_ENV` 열세 개를 지우는 대신, 프로세스 환경에서 `GIT_`로 시작하는 이름을 모두 모아 허용 목록에 없는 것을 지운다. 지운 뒤 canon이 정하는 `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`, 호출이 청할 때의 `GIT_NO_LAZY_FETCH=1`, 임시 색인을 줄 때의 `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`를 세운다(`:233-239`). 긴 수명 `cat-file --batch` 자식(`migrate_legacy_pins.rs:231`)도 같은 명령줄이다.
- ==**허용 목록은 이름 하나씩이고, 지나가는 이름은 로컬 답의 바이트를 바꾸지 못하는 것뿐이다.**== 이유가 하나인 두 묶음이다.
  - 호스트의 gitconfig 파일을 고르는 이름: `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_NOSYSTEM`. `HOME`과 XDG를 그대로 두는 것과 같은 이유다. 어느 파일을 읽든 아래 F2가 모양을 고정한다. 지우면 `safe.directory`를 전역 파일로 주는 환경과 hermetic한 시험 환경이 깨진다.
  - 원격에서 객체를 가져올 때만 쓰는 이름: `GIT_SSH`, `GIT_SSH_COMMAND`, `GIT_SSH_VARIANT`, `GIT_ASKPASS`, `GIT_PROXY_COMMAND`, `GIT_SSL_CAINFO`, `GIT_SSL_CAPATH`, `GIT_SSL_NO_VERIFY`, `GIT_SSL_CERT`, `GIT_SSL_KEY`, `GIT_SSL_CERT_PASSWORD_PROTECTED`, `GIT_SSL_CERT_TYPE`, `GIT_SSL_KEY_TYPE`, `GIT_SSL_VERSION`, `GIT_SSL_CIPHER_LIST`, `GIT_HTTP_PROXY_AUTHMETHOD`, `GIT_HTTP_LOW_SPEED_LIMIT`, `GIT_HTTP_LOW_SPEED_TIME`. 부분 복제에서는 `diff HEAD`, `show`, `add -A`가 빠진 객체를 느슨하게 가져오고(`no_lazy_fetch`를 청한 호출만 안 가져온다), 그때 이 이름이 전송을 정한다. 로컬 답의 바이트를 바꾸지 못한다.
- 지우는 것: 0.39가 지우던 열세 개(`GIT_CEILING_DIRECTORIES` 포함. 지금처럼 `--repo` 위쪽 저장소를 찾는다)와 그 밖의 전부다. 눈에 띄는 것: `GIT_DIFF_OPTS`, `GIT_EXTERNAL_DIFF`, `GIT_CONFIG_COUNT`와 `GIT_CONFIG_KEY_<n>`, `GIT_CONFIG_VALUE_<n>`, `GIT_CONFIG_PARAMETERS`, `GIT_EXEC_PATH`, `GIT_TRACE`와 `GIT_TRACE2*`(stderr가 오류 메시지에 들어간다), `GIT_PAGER`, `GIT_EDITOR`, `GIT_AUTHOR_*`와 `GIT_COMMITTER_*`, `GIT_ATTR_SOURCE`, `GIT_NO_REPLACE_OBJECTS`, `GIT_DISCOVERY_ACROSS_FILESYSTEM`, 훅이 세우는 `GIT_QUARANTINE_PATH`. canon은 커밋을 만들지 않아 작성자 변수가 필요 없다(`migrate`는 `rm`, `mv`, `add -f`, `update-index`만 부른다: `migrate.rs:1350,1365`, `migrate_names.rs:787-788,1448,1452`).
- `GIT_` 아닌 이름(`PATH`, `HOME`, XDG, `SSH_AUTH_SOCK`, `LC_*`)은 지금처럼 둔다. `LC_ALL=C`는 canon이 정한다.
- 허용 목록은 `git.rs`의 상수 하나(`CLEARED_ENV`를 대신한다)이고, 이름 판정은 환경 이름 목록을 받는 순수 함수다. 시험이 프로세스 환경을 바꾸지(`set_var`) 않고도 "목록에 있는 것은 남고 없는 것은 지워진다"를 확인할 수 있게 한다.
- 파일 맨 위의 문서 줄(`git.rs:7-10`), `docs/design/rust-coding-rules.md:374`의 "clears the git environment", 함수 문서가 거부 목록을 말하는 곳을 허용 목록으로 고친다.

### F2 읽는 출력의 모양

- 환경과 달리 설정은 파일에서도 온다. 전역과 시스템 gitconfig를 통째로 가리는 길(`GIT_CONFIG_GLOBAL=/dev/null`과 `GIT_CONFIG_NOSYSTEM=1`을 canon이 세우는 것)은 쓰지 않는다. `git add -A`는 `filter.lfs.*`와 `core.autocrlf`를 읽고, 이 값들이 전역이나 시스템 파일에만 있는 사용자가 많다. 가리면 커밋된 트리와 작업 트리가 달라 모든 트리가 더티가 된다. `safe.directory`도 전역 파일에만 둘 수 있어, 가리면 소유자가 다른 체크아웃이 `COMMIT_REQUIRED`가 된다.
- 대신 모양이 바뀌는 호출의 모양만 고정한다. 값은 git의 기본값이라, 설정이 없는 사용자의 출력은 0.39와 바이트까지 같다(위 해시가 설정 없이도 옵션을 주어도 같다는 것을 재서 확인했다).
  - 모든 호출: `-c color.ui=false`와 `-c core.abbrev=auto`를 `core.quotepath`, `core.fsmonitor` 옆에 둔다(`git.rs:229`). 명령줄 `-c`는 파일 설정을 이긴다.
  - `context_pack.rs:551`의 `diff --no-ext-diff --binary HEAD`: `--no-color`, `-U3`, `--inter-hunk-context=0`, `--src-prefix=a/`, `--dst-prefix=b/`, `--find-renames`, `--diff-algorithm=myers`, `--no-relative`, `--ignore-submodules=none`, `--submodule=short`, `-l1000`, `--indent-heuristic`, `-O/dev/null`.
  - `context_pack.rs:552`의 `status --porcelain=v1 -z`: `--untracked-files=normal`, `--find-renames`, `--no-branch`, `--ignore-submodules=none`.
  - `staleness.rs`의 `diff --name-only -z --no-renames <sha>..HEAD`(`git_diff_names`): `--ignore-submodules=none`. `diff-tree` 호출들(`risk.rs:193`, `git_rev.rs:339`)은 같은 설정에도 달라지지 않아 그대로 둔다.
- 두 diff와 status의 고정 옵션은 `git.rs`에 이름 있는 상수로 둔다. 호출처가 크레이트 둘(`canon-cli`, `canon-gate`)이라 한 곳에서 읽는다. `context_pack.rs:878`의 시험이 같은 diff를 직접 부르는 곳도 이 상수를 쓴다.
- 정확한 옵션 집합은 시험이 정한다. 시험은 아래 설정 행렬(`diff.context=0`, `diff.noprefix`, `diff.mnemonicPrefix`, `diff.srcPrefix`, `diff.renames=false`, `diff.relative`, `diff.suppressBlankEmpty`, `color.ui=always`, `core.abbrev=12`, `status.showUntrackedFiles=no`, `status.renames=false`, `diff.ignoreSubmodules=all`, `diff.orderFile`, `diff.submodule=log`, `diff.renameLimit=1`, `diff.indentHeuristic=false`)을 홈과 저장소 gitconfig에 차례로 놓고 설정 없는 출력과 같음을 확인한다. 행렬에서 하나라도 어긋나면 옵션을 더한다. 서브모듈, 부분 일치 이름 변경 셋, 슬라이더 파일이 든 더티 트리를 쓰고, 행마다 설정 아래에서 맨 git의 출력이 달라지는 것을 먼저 확인한다(헛된 행을 막는다).
- 손대지 않는 것: 저장소가 읽는 대로 파일 내용과 집합을 정하는 설정과 속성이다. `core.autocrlf`, `filter.*`, `.gitattributes`, `core.excludesFile`, `core.attributesFile`, `core.symlinks`, `core.fileMode`, `core.ignoreCase`. 코드 트리 id의 일부이고 사용자가 저장소를 쓰는 방식이다.
- 쓰는 곳: context pack, 낡음 검사, 위험 승인(변경 없음), 코드 트리(변경 없음).

### F3 변경 기록

- CHANGELOG의 Changed가 무엇을 말하는지:
  - 이제 지우는 것이 목록 밖의 모든 `GIT_*`라는 것과 지나가는 이름 스물한 개와 그 이유(호스트 gitconfig를 고르는 셋, 원격 가져오기 열여덟).
  - 설정이 없는 사용자에게는 달라지는 것이 없다는 것. 설정이나 변수가 출력을 바꾸던 더티 트리의 context pack은 같은 트리에서 id가 한 번 바뀐다.
  - 쓰던 변수가 실패하는 경우: `safe.directory`를 `GIT_CONFIG_COUNT`나 `GIT_CONFIG_PARAMETERS`로 주던 CI는 소유자가 다른 체크아웃에서 `COMMIT_REQUIRED`를 받고 메시지에 git의 "dubious ownership"이 담긴다. 같은 값을 gitconfig 파일(`GIT_CONFIG_GLOBAL`이 가리키는 것 포함)에 두면 된다.
  - 새 옵션, 새 오류 코드, 새 환경 변수는 없다.
- 0.39 항목이 말한 "셸의 git 환경은 무시한다"가 이제 사실이라는 것은 0.39 항목을 고치지 않고 이 항목에 쓴다.

## Acceptance

- F1:
  - 추적 파일을 고친 더티 트리에서 `GIT_DIFF_OPTS=-u0`, `GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=diff.context GIT_CONFIG_VALUE_0=0`, `GIT_CONFIG_PARAMETERS="'diff.context=0'"`, `GIT_TRACE=1`을 각각 건 `context-pack add`의 id와 상태 다이제스트는 환경 없이 돌린 것과 같다.
  - 목록의 이름은 값 그대로 닿고(`GIT_SSH_COMMAND`, `GIT_CONFIG_GLOBAL`을 건 자식의 환경), 목록에 없는 이름은 git이 모르는 것(`GIT_CANON_TEST_UNKNOWN`)도 지워지며, canon이 정하는 변수는 그 값이다. 이름 판정 시험이 이 셋을 모두 본다.
  - 실패 경로: ==**목록에 없는 이름은 값이 어떻든 닿지 않고**==, 열세 개로 지우던 `GIT_DIR`, `GIT_INDEX_FILE`도 여전히 닿지 않는다. 0.39의 `gate.git-boundary.01`, `02`, `03`이 그대로 통과한다.
- F2:
  - 홈 gitconfig(`GIT_CONFIG_GLOBAL`)에 행렬의 설정을 하나씩 두고 돌린 더티 트리의 `context-pack add`는 상태 다이제스트와 id가 설정 없이 돌린 것과 같다. 이때 설정 없는 쪽의 값은 0.39 바이너리가 낸 값과 같다(고정 값으로 시험에 넣는다).
  - 저장소 자신의 `.git/config`에 같은 설정을 두어도 같다.
  - `diff.ignoreSubmodules=all`이어도 서브모듈을 올린 커밋 뒤에서 그 서브모듈을 표면으로 선언한 증거는 `gate check`에서 `stale-evidence`다.
  - 실패 경로: `core.autocrlf`와 `.gitattributes`는 이 고정에 들지 않는다. LF로 커밋한 파일을 CRLF로 바꾼 저장소에서 `core.autocrlf=true`이면 작업 트리의 코드 트리 id가 HEAD의 것과 같아 `dirty`가 거짓이고(잰 값: 같은 id), 설정이 없으면 다르다. 이 관계가 그대로여야 한다. 행렬에 없는 이름을 고정해 이를 깨는 변경은 시험에 걸린다.
- F3:
  - CHANGELOG에 항목이 없거나 허용 이름, 환경이 실패하는 경우, 다이제스트가 한 번 바뀐다는 말이 빠지면 릴리스 확인 시험이 실패한다(0.39의 `git_boundary.rs:499,525` 방식).
  - 실패 경로: `docs/design/rust-coding-rules.md`의 git 길 규칙이 여전히 "clears the git environment"만 말하면 같은 시험이 실패한다.
- `cargo test --workspace`, `just atip-deep`, `canon gate check`가 통과한다.

## Questions and assumptions

- 결정: 이름 판정은 허용 목록이다. 이름 하나씩이고 접두사로 열지 않아, 새 git이 새 `GIT_*`를 더해도 기본이 닫힘이다.
- 결정: `GIT_CEILING_DIRECTORIES`와 `GIT_DISCOVERY_ACROSS_FILESYSTEM`은 지운다. 허용하면 `--repo` 위쪽 저장소를 찾는 답이 셸에 달린다.
- 결정: `GIT_EXEC_PATH`는 지운다. `git`을 PATH에서 고르는 이상 그 설치의 도우미를 쓰고, 훅이 물려준 값은 같은 설치를 가리킨다. 래퍼가 필요로 하면 자식 래퍼가 스스로 세운다.
- 결정: `GIT_AUTHOR_*`, `GIT_COMMITTER_*`는 지운다. canon은 커밋을 만들지 않는다.
- 결정: 호스트 gitconfig를 가리지 않는다. 가림은 LFS·autocrlf 같은 필터와 `safe.directory`를 깬다(F2). 대신 모양만 고정한다.
- 결정: 고정 값은 git의 기본이다. `--full-index`나 고정 길이 abbrev는 설정 없는 사용자의 다이제스트도 바꾸므로 쓰지 않는다(`core.abbrev=auto`는 저장소 크기에 따라 길이가 변하는 지금의 성질을 그대로 둔다).
- 결정(리뷰 발견 1_4): 원격 가져오기 이름에 `GIT_SSL_CERT`, `GIT_SSL_KEY`, `GIT_SSL_CERT_PASSWORD_PROTECTED`, `GIT_SSL_CERT_TYPE`, `GIT_SSL_KEY_TYPE`, `GIT_SSL_VERSION`, `GIT_SSL_CIPHER_LIST`, `GIT_HTTP_PROXY_AUTHMETHOD`, `GIT_HTTP_LOW_SPEED_LIMIT`, `GIT_HTTP_LOW_SPEED_TIME`을 더한다(git-remote-http가 읽고 전송만 정한다). `GIT_CURL_VERBOSE`는 `GIT_TRACE`처럼 stderr를 채워 오류 메시지에 들어가므로 더하지 않는다. 허용 이름은 모두 스물한 개다.
- 결정: 고정 옵션에 `--submodule=short`, `-l1000`, `--indent-heuristic`, `-O/dev/null`을 더한다(리뷰 발견 1_1: `diff.submodule`, `diff.renameLimit`, `diff.indentHeuristic`, `diff.orderFile`이 상태 다이제스트를 바꿨다). 값은 git의 기본이다.
- 결정: 환경 변수는 지워서 막고 설정은 `-c`와 옵션으로 막는다. `-U3`은 `GIT_DIFF_OPTS`를 이기지 못했고, `-c`와 옵션은 파일 설정과 `GIT_CONFIG_COUNT`를 이겼다(잰 값).
- 결정: 새 오류 코드, 새 옵션, 새 환경 변수는 없다. 실패는 지금의 `COMMIT_REQUIRED`, `CONTEXT_GIT_REQUIRED`가 git의 stderr를 담아 말한다.
- 결정: `log`, `show`, `rev-parse`, `ls-files`, `update-index`, `write-tree`, `read-tree`, `diff-tree`, `merge-base`, `rev-list`는 이 변경에서 모양을 고정하지 않는다. 읽는 값이 40자리 해시와 NUL 구분 이름이고, 위 행렬에서 달라지는 것이 없었다. `log.showSignature`는 서명된 커밋이 있는 저장소에서 재지 않았다.
- 실패 방식: `safe.directory`를 `GIT_CONFIG_COUNT`/`GIT_CONFIG_PARAMETERS`로 주던 환경은 소유자가 다른 체크아웃에서 실패한다. git 문서가 보호된 설정(system, global, command 범위)으로 말하는 곳이라 지운 변수에서 왔다는 이유로 걸러낼 수 없다.
- 실패 방식: `GIT_EXEC_PATH`로 다른 git 빌드를 고르던 사용자는 PATH의 `git`이 가리키는 설치의 도우미를 쓴다.
- 실패 방식: 저장소의 `.gitattributes`가 고른 `diff.<driver>.xfuncname`은 hunk 머리 줄을 바꾼다. 저장소가 정하는 값이라 둔다. 전역 `core.attributesFile`로 오는 같은 속성은 호스트마다 다를 수 있고 이 변경이 막지 않는다.
- 답(승인 2026-11-09, 기본값): 받아들이고 CHANGELOG에 쓴다. 질문은 이랬다: `safe.directory`를 `GIT_CONFIG_COUNT`/`GIT_CONFIG_PARAMETERS`로 주던 CI가 깨지는 것을 받아들이는가. 기본은 받아들이고 CHANGELOG에 쓴다. 아니라면 canon이 환경에서 `safe.directory` 항목만 골라 `-c`로 넘기게 한다(허용 목록의 예외가 하나 더 생긴다).
- 답(승인 2026-11-09, 기본값): 통과시킨다. 질문은 이랬다: 원격 가져오기 이름(승인 당시 여덟 개)(`GIT_SSH`, `GIT_SSH_COMMAND`, `GIT_SSH_VARIANT`, `GIT_ASKPASS`, `GIT_PROXY_COMMAND`, `GIT_SSL_CAINFO`, `GIT_SSL_CAPATH`, `GIT_SSL_NO_VERIFY`, `GIT_SSL_CERT`, `GIT_SSL_KEY`, `GIT_SSL_CERT_PASSWORD_PROTECTED`, `GIT_SSL_CERT_TYPE`, `GIT_SSL_KEY_TYPE`, `GIT_SSL_VERSION`, `GIT_SSL_CIPHER_LIST`, `GIT_HTTP_PROXY_AUTHMETHOD`, `GIT_HTTP_LOW_SPEED_LIMIT`, `GIT_HTTP_LOW_SPEED_TIME`)를 통과시키는가. 기본은 통과다(부분 복제에서 canon의 diff와 add가 객체를 가져올 때 필요하고 답의 바이트를 바꾸지 못한다). 아니라면 부분 복제의 인증은 gitconfig와 `SSH_AUTH_SOCK`로만 된다. 리뷰 발견 1_4로 git-remote-http가 읽는 변수 열 개를 같은 이유로 더했다(아래 결정).
- 답(승인 2026-11-09, 기본값): 고정하지 않는다. CHANGELOG에 호스트마다 작업 트리 코드 트리가 다를 수 있다고 쓴다. 질문은 이랬다: 전역 `core.excludesFile`은 `git add -A`가 보는 추적 안 하는 파일 집합을 바꿔 호스트마다 다른 작업 트리 코드 트리 id를 낼 수 있다. 커밋된 트리는 영향이 없다. 이 변경에서 고정하는가. 기본은 고정하지 않는다(F2: 파일 집합을 정하는 설정은 저장소 사용 방식의 일부로 둔다).

## Out of scope

- 전역과 시스템 gitconfig 전체를 가리는 길. 위에서 버렸다.
- `git add -A`가 읽는 파일 내용과 집합 설정(`core.autocrlf`, `filter.*`, `core.excludesFile`, 속성)의 고정.
- 모양을 고정하지 않는 plumbing 호출들과 `migrate`의 `git status --porcelain --untracked-files=all`(이미 한 옵션을 고정하고 화면에 그대로 보인다).
- `GIT_` 아닌 환경 변수(`PATH`, `HOME`, XDG, `LANG`, `TZ`).
- Windows의 환경 변수 이름 대소문자 처리.
- 기한, 출력 한도, 이름 읽기(0.39에서 끝났다).
- `crates/*/tests`의 통합 시험이 부르는 git.

Approved: Journey Hwang 2026-11-09T14:24:16Z — "재승인: 리뷰 지적에 따라 고친 브리프 3개" (relayed by planner planner-oco via ask) (code tree b9fe5e9bd8ef5436b3e8990ed06245172b3a003c) (brief sha256:14eb201bfed6f0188f5fa17458602dac2fc8d2be43e2bba9801fcb5698b6eca6)
