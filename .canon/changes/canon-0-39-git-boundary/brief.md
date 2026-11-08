---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "환경 변수, 설정 키, 시험용 변수의 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "환경 변수와 오류 코드의 이름은 시험이 확인하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "린트 이름과 설정 키는 버린 설계를 재현하는 철자 자체다"
  - rule: "prose.length"
    section: "brief"
    reason: "열두 파일의 호출 열아홉 곳과 CI 가드를 한 길로 묶는 변경이라 섹션마다 예산 안이어도 합이 4000자를 넘는다"
---

# canon의 모든 git 호출은 환경을 비우고 기한과 출력 한도가 있는 한 길을 지난다

Source: docs/research/vendor-audits/00-cross-vendor-patterns-rust-2026-11.md (D9, D12, D15, R1, R3), docs/research/vendor-audits/CANON-ADOPTION-BRIEF-rust-2026-11.md (묶음 A), 0.37.0 바이너리로 재현한 값과 0.38.0 소스

## Intent

canon은 git을 19곳에서 부르고 어느 호출에도 기한이나 출력 한도가 없다. 환경을 정하는 곳은 코드 트리의 임시 색인 하나뿐이다. 0.37.0에서 다른 저장소를 가리키는 환경 변수를 걸고 `evidence add`를 돌리면 기록이 그 저장소의 HEAD에 묶였다. 임시 색인을 가리키는 변수는 context pack의 상태 다이제스트를 바꾼다. `é.rs`를 선언한 표면은 고쳐 커밋해도 `gate check`가 clean을 말했다. git이 이름을 인용해 내기 때문이다. PATH 앞의 잠자는 git 래퍼는 `gate check`를 출력 없이 붙잡았다.

## Desired outcome

- 모든 git 호출이 환경을 비우고 설정을 고정한 한 길을 지난다.
- 셸의 git 환경 변수는 canon의 git 호출에 닿지 않는다.
- 파일 이름은 NUL로 읽는다. 비ASCII 글자, 따옴표, 공백이 든 이름도 선언한 표면과 같은 문자열이다.
- 일회성 호출은 300초와 출력 한도 안에서 끝난다. 넘으면 git을 멈추고 한도를 말한다.
- 이 길을 우회하는 새 호출은 CI가 거절한다.
- ==**canon이 git에 묻는 질문과 답을 읽는 규칙은 그대로이고, 환경과 이름 읽기와 기다리는 시간만 바뀐다.**==

## User flow

1. F1 에이전트는 git 환경 변수가 걸린 셸이나 훅에서 canon을 돌린다 → canon의 git 호출은 그 값을 보지 않는다 → 기록은 `--repo`의 커밋과 트리를 말한다.
2. F2 에이전트는 이름에 비ASCII 글자나 따옴표가 든 파일을 표면으로 선언하고 고친다 → 변경 목록을 NUL로 읽는다 → 증거가 stale이 된다.
3. F3 에이전트는 git이 멈추거나 너무 많이 쓰는 환경에서 canon을 돌린다 → ==**호출은 기한과 한도 안에서 끝나고 메시지가 그 한도를 말한다.**==
4. F4 구현자는 새 git 호출을 더한다 → 길을 쓰지 않으면 CI가 파일과 줄을 말하며 거절한다.
5. F5 사람은 CHANGELOG를 읽는다 → 다시 승인해야 하는 경우가 적혀 있다.

## Step detail

### F1 환경과 설정

- 호출마다 저장소와 객체를 가리키는 변수를 비운다. `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR`, `GIT_NAMESPACE`, `GIT_CEILING_DIRECTORIES`, `GIT_PREFIX`, 경로 해석 변수 넷이다. `HOME`, `PATH`, `SSH_AUTH_SOCK`, XDG는 둔다.
- 고정하는 값은 `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`, `core.quotepath=false`, `core.fsmonitor=false`, 페이저 끔, 닫힌 stdin이다.
- 임시 색인은 호출이 밝힌다. 색인 파일, 임시 객체 디렉터리, 저장소 객체 디렉터리와 alternates를 한 묶음으로 줄 때만 해당 변수가 선다.
- 쓰는 곳: 코드 트리, staleness, 위험 승인, 정책 승인 이력, context pack, `migrate`
- 스킬 설치, 참조 검사, 리포트 manifest, `gate selftest`도 쓴다.
- 긴 수명 `cat-file --batch` 자식도 같은 명령줄이다. `GIT_NO_LAZY_FETCH=1`만 이 자식에 남는다.

### F2 파일 이름

- staleness와 위험 승인이 읽는 변경 파일 목록은 NUL 구분이고 이름을 자르지 않는다.
- UTF-8이 아닌 이름은 오늘과 같다. staleness는 그 질의를 답 없음으로 두고 위험 승인은 분석 불명으로 닫는다.
- 비ASCII 이름의 표면과 위험 규칙 경로가 맞기 시작한다.
- ==**그런 파일을 바꾼 커밋에 0.38까지 기록한 승인은 서명한 표면 목록이 달라져 다시 승인해야 한다.**==

### F3 기한과 한도

- 일회성 호출의 기한은 300초 상수다. 릴리스 빌드에는 바꾸는 변수가 없고 디버그 빌드만 시험용 `CANON_TEST_GIT_DEADLINE_MS`로 줄인다.
- 기한이 지나면 git 자식만 죽이고 기다림을 끝낸다. 손자가 파이프를 쥐고 있어도 더 기다리지 않는다.
- 출력은 stdout과 stderr 각각 8 MiB다. 트리 전체를 나열하는 호출(코드 트리의 파일 목록, `migrate`의 색인 목록)은 64 MiB다. 넘으면 자식을 죽이고 읽은 출력을 쓰지 않는다.
- 실패는 시작 못 함, 기한 초과, 출력 초과, 종료 코드다. 앞의 둘은 git 부속 명령과 한도를 메시지에 담는다.

| 자리 | 기한이나 한도를 넘으면 |
|---|---|
| 커밋 묶기와 코드 트리 | 지금과 같은 `COMMIT_REQUIRED` 거절이다. 아무것도 쓰지 않는다. |
| context pack | `CONTEXT_GIT_REQUIRED` 거절이다. 8 MiB를 넘는 추적 파일 변경의 diff도 같다. |
| staleness | 그 질의는 답 없음이다. 한 번 넘기면 같은 실행의 나머지 질의를 건너뛴다. |
| 위험 승인 | 분석 불명으로 닫는다. 한 번 넘기면 나머지 커밋을 묻지 않는다. |
| 그 밖의 호출 | 그 호출이 git 실패에 내던 답이다. |

### F4 한 길

- 스크립트가 `crates/*/src`에서 git을 띄우는 호출을 찾는다. 인자가 다음 줄에 오는 형태와 `#[cfg(test)]` 모듈도 본다. 길을 정의한 파일 밖에서 발견하면 파일과 줄을 내고 1로 끝난다.
- 소스 안 시험의 픽스처도 길을 쓴다. `crates/*/tests`의 통합 시험은 대상이 아니다.
- CI의 clippy 작업과 `just lint`가 스크립트를 돌린다. 규칙 한 줄이 `docs/design/rust-coding-rules.md`에 든다.

### F5 변경 기록

- CHANGELOG의 Changed가 환경 무시, 비ASCII 이름, 기한과 한도, 다시 승인할 경우를 말한다. 새 옵션과 새 코드는 없다.

## Acceptance

- F1:
  - r2에서 `GIT_DIR=<other>/.git`(과 `GIT_WORK_TREE`)을 건 `evidence add`가 기록하는 커밋과 코드 트리는 환경 없이 돌린 것과 같다. 다른 색인을 가리킨 `GIT_INDEX_FILE`에서 `context-pack show`의 상태 다이제스트도 같다.
  - 같은 환경의 `migrate --commit`은 other의 색인을 바꾸지 않는다.
  - 실패 경로: ==**git 작업 트리가 아닌 `--repo`에 `GIT_DIR`를 걸어도 `COMMIT_REQUIRED`로 거절된다.**==
- F2:
  - `é.rs`나 `a"b.rs`를 선언한 증거는 그 파일을 고쳐 커밋하면 `gate check`가 stale-evidence를 보고한다. 승인 페이로드의 표면도 `é.rs`다.
  - 실패 경로: UTF-8이 아닌 이름을 바꾼 커밋에서 위험 승인은 위반을 내고 staleness는 stale로 단정하지 않는다.
- F3:
  - `-c alias.nap='!sleep 30' nap`은 기한 1초에 3초 안에 기한 초과로 실패한다. 손자가 파이프를 쥐어도 같다. 9 MiB 출력은 8 MiB 한도에서 실패하고 64 MiB 한도에서 성공한다.
  - `..HEAD`에서 잠드는 PATH 래퍼 위의 `gate check`가 기한 1초에 몇 초 안에 끝난다.
  - 실패 경로: `add`에서 잠드는 래퍼 위의 `evidence add`는 `COMMIT_REQUIRED`로 거절되고 메시지가 `git add`와 기한을 말하며 아무것도 쓰지 않는다.
- F4:
  - 길 밖 파일에 `Command::new("git")`를 심으면 스크립트가 파일과 줄을 내고 1로 끝난다. 다음 줄 인자와 시험 모듈도 같다.
  - 실패 경로: 발견이 하나라도 있으면 CI의 clippy 작업이 실패한다.
- F5:
  - CHANGELOG에 항목이 없으면 릴리스 확인 시험이 실패한다.
- `cargo test --workspace`, `just lint`, `canon gate check`가 통과한다.

## Questions and assumptions

- 결정: 길은 `canon-store`의 모듈 하나이고 기한은 환경 변수 없는 300초 상수다(큰 트리의 `git add -A`가 끝나는 값). 새 크레이트, 새 외부 의존, `unsafe` 없이 읽기 스레드와 `try_wait` 폴링으로 센다. 자식만 죽인다. 그룹 종료는 `unsafe`나 `rustix`가 필요하다.
- 결정: 환경은 비울 목록이다. 허용 목록은 git이 쓰는 `HOME`, `PATH`를 막는다.
- 결정: 새 오류 코드는 없다. 코드로 갈라 할 호출자가 없다.
- 결정: 가드는 스크립트다. 루트 `clippy.toml` 금지는 `crates/*/tests` 100여 개 파일 160여 곳에서 `--all-targets`가 실패한다. 별도 설정 디렉터리는 길의 `#[expect]`가 `--all-targets`에서 충족되지 않아 실패한다(장난감 크레이트로 확인). 풀려면 조건부 cfg와 git 아닌 호출 다섯 곳의 예외가 든다.
- 결정: `untrusted` 옵션과 `GitQuery` 가짜는 없다. 쓰는 호출자가 없다.
- 결정: 기한 뒤 나머지 질의를 건너뛰는 일은 staleness와 위험 승인만 한다. 프로세스 전체가 기억하면 대시보드가 한 번의 지연으로 영영 git을 거절한다.
- 실패 방식: 변수에 담은 프로그램 이름은 스크립트가 못 본다. 리뷰가 두 번째 방어다.
- 실패 방식: 환경 변수로 저장소를 일부러 가리키던 설정은 따르지 않는다. `--repo`의 `.git`을 쓴다.
- 실패 방식: 8 MiB 넘는 diff를 가진 더티 트리의 context pack은 거절된다. 커밋하면 풀린다.

## Out of scope

- 세션 수집(`canon ingest`)의 git 호출과 결함(D1-D5, D20). 수집은 이 릴리스에서 별도 변경이 지운다.
- context pack의 `dirty`가 깨끗한 저장소에서도 참인 결함.
- 디렉터리와 글롭 표면 매칭, 풀 수 없는 기준 커밋을 건수로 말하는 일. staleness 변경의 몫이고, 이 변경이 끝나도 기한을 넘긴 질의는 `gate check`에서 답 없음이다.
- git이 아닌 프로세스(ssh-keygen, duckdb, 훅의 `/bin/sh`, 자기 재실행).
- `crates/*/tests`의 통합 시험이 부르는 git.
- 긴 수명 `cat-file` 자식의 읽기 기한. 로컬 객체만 읽는다.
- 프로세스 그룹 종료, Windows 전용 처리, 기한을 바꾸는 릴리스 변수.

Approved: Journey Hwang 2026-11-08T06:11:30Z — "승인 (canon 0.39 브리프 7개와 정책)" (relayed by planner planner-oco via ask) (code tree 7c28ab134d4701f16f343da5e87c7582b9521417) (brief sha256:27758f3201750e949344eabf456834909587125ab4009f9304f38cf39b31b4c5)
