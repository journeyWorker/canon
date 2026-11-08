---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "다섯 가지 쓰기 결함을 한 변경에서 고치고 항목마다 실패 경로와 유닉스·Windows 차이를 정해야 해서 줄이면 조건이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "결함 다섯의 syscall 측정값과 재현하지 못한 부분을 함께 적어야 현재 동작이 빠지지 않는다"
  - rule: "prose.length"
    section: "outcome"
    reason: "결과 일곱 줄이 각각 한 결함의 닫힘 조건이라 줄이면 항목이 빠진다"
  - rule: "prose.length"
    section: "flow"
    reason: "여덟 흐름이 결함마다 하나씩이라 줄이면 흐름이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "단계마다 플랫폼 차이와 호출자 조사 결과를 담아야 하는 변경이다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "여덟 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정 일곱과 실패 양식이 서로 다른 선택이라 함께 적어야 한다"
  - rule: "prose.sentence-length"
    section: "detail"
    reason: "함수와 호출 순서를 한 문장에 적어야 앞뒤 관계가 보인다"
  - rule: "prose.sentence-length"
    section: "acceptance"
    reason: "시험이 확인하는 순서와 횟수를 한 문장에 적어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "함수·파일·옵션 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "함수·파일 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "함수·파일 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "함수·파일·시험 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "함수·파일·시험 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "함수·파일 이름은 호출자가 읽는 철자 자체다"
---

# canon의 파일 쓰기는 반환하기 전에 디렉터리 항목까지 디스크에 남기고, 쓰는 길은 하나다

Source: 2026-11-06 벤더 감사의 재검증(D7·D8·D11·D16, 패턴 R2·R9)과 채택 브리프, canon 0.37.0 디버그 바이너리에 syscall을 끼워 잰 값(0.38.0에서 해당 코드는 그대로다)

## Intent

canon이 디스크에 남기는 기록 파일은 거의 모두 한 함수, 곧 `canon-store`의 `write_atomic`을 지난다. 이 함수는 임시 파일에 쓰고 `sync_all`한 뒤 `rename`하고 돌아온다. 그 `rename`이 만든 디렉터리 항목을 디스크에 내리는 부모 디렉터리 fsync는 어디에도 없다. `evidence add --promote` 한 번을 libc 호출 가로채기로 재면 파일 fsync 3번, rename 3번, 디렉터리 fsync 0번이다. 증거 블롭을 놓는 `publish_paths`도 같다. 블롭 내용은 `stage`에서 sync하지만 놓는 `rename`은 맨몸이다. 전원이 나가면 돌아온 명령의 기록이 사라질 수 있다. 이 결과는 POSIX 의미에서 추론한 것이고 장애 주입 파일시스템으로 재현하지는 못했다. 빠진 호출 자체는 추적에 찍힌다.

`write_atomic`은 하나가 아니다. 임시 파일을 만들고 `rename`하는 사본이 네 개 더 있다. `migrate.rs`(canon.yaml을 다시 쓴다), `legacy.rs`, `migrate_briefs.rs`, `canon-learn`의 `promote.rs`이고 모두 fsync가 없다. `migrate.rs` 사본은 임시 이름도 `.canon.yaml.canon-migrate.tmp`로 고정이다. 두 `migrate --commit`은 저장소 쓰기 잠금으로 직렬이라 이름이 겹쳐 부딪히지 않는다. 문제는 fsync 없는 사본이 넷 남아 있다는 점이다.

감사 기록 `.canon/audit/access.jsonl`의 줄은 `writeln!`으로 쓴다. 0.37.0 바이너리에서 `canon query`가 한 줄을 붙일 때 write 호출이 둘로 나뉘었다. 171바이트 JSON이 한 번, 개행 1바이트가 또 한 번이다. 둘 사이에 프로세스가 끊긴 상태를 마지막 바이트를 잘라 흉내 내면 다음 줄이 앞 줄에 붙는다. 3개 기록이 2줄이 되고 `serde_json::from_str`로 줄마다 읽는 쪽은 그 줄을 읽지 못한다. 실제 SIGKILL을 그 틈에 맞춰 보지는 못했다. 틈이 마이크로초 아래이기 때문이다.

핫 계층 `SqliteTier::rekey`는 `canon migrate`가 쓰는 유일한 읽고-쓰는 경로다. 일반 `BEGIN`으로 시작해 held 확인 `SELECT`를 하고 그 뒤 `UPDATE`나 `DELETE`를 한다. WAL에서 읽은 뒤 다른 쓰기가 커밋되고 나서 쓰기로 올리면 `database is locked`가 난다. SQLite 엔진을 직접 건드린 탐침에서 `busy_timeout=5000`인데도 0.000초에 실패했다. canon 안에서는 재현하지 못했다. 호출자인 `migrate --commit`이 저장소 쓰기 잠금을 쥐고 있어 다른 canon 쓰기가 끼지 못하고, 창은 첫 `SELECT`와 첫 `UPDATE` 사이 마이크로초다. 파일이 `.canon/hot.db`이고 `canon` 밖의 쓰기 프로세스가 같은 파일을 열면 닿는다. `write_rows`는 첫 문장이 `INSERT`라 해당이 없다.

핫 계층 파일은 만들 때 모드를 정하지 않는다. 기본 umask를 받고, 서로 다른 사용자가 공유하는 작업 트리에서 읽을 수 있다. 같은 곳의 `-wal`과 `-shm`은 SQLite가 본 파일의 모드로 만든다.

## Desired outcome

- ==**`write_atomic`이 `Ok`로 돌아오면 파일 내용과 그 이름이 놓인 디렉터리 항목이 모두 디스크에 내려가 있다.**== 유닉스에서 그렇다.
- 증거 블롭을 놓는 일도 같다. 블롭들을 놓은 뒤 디렉터리를 한 번 sync하고 그 sync가 실패하면 놓은 블롭을 되돌리고 실패를 말한다.
- 디렉터리 fsync가 실패하면 조용히 삼키지 않고 호출자에게 오류로 낸다.
- canon에는 임시 파일을 만들고 `rename`하는 쓰기 사본이 `canon-store`의 것 하나뿐이다. `canon.yaml` 다시 쓰기, 레거시 이행 파일, 이행 중인 브리프, 전략 승격 파일이 그 하나를 쓴다.
- 감사 기록의 한 줄은 개행까지 한 번의 write 호출로 나간다.
- `SqliteTier::rekey`는 시작할 때 쓰기 잠금을 잡는다. 읽고-쓰는 사이에 다른 쓰기가 끼어도 `database is locked`로 실패하지 않는다.
- 새로 만드는 `.canon/hot.db`는 소유자만 읽고 쓴다(0600). `.canon/` 디렉터리와 장부 기록 파일의 모드는 바꾸지 않는다.
- Windows에서 이 변경이 바꾸는 것은 없다. 디렉터리 fsync는 유닉스에서만 하고 Windows는 어제와 같다는 말이 코드 주석과 CHANGELOG에 있다.

## User flow

1. F1 오케스트레이터는 기록을 쓰는 명령(`evidence add`, `dispatch`, `context-pack`, `ledger` 계열)을 돌린다 → `write_atomic`이 파일을 sync하고 놓은 뒤 부모 디렉터리를 sync한다 → 명령이 성공으로 돌아왔을 때 기록 파일의 디렉터리 항목이 디스크에 있다.
2. F2 오케스트레이터는 `--artifact`가 붙은 `evidence add`나 `artifacts` 명령으로 블롭을 놓는다 → 블롭을 놓은 뒤 증거 저장소 디렉터리를 sync한다 → 기록이 그 블롭을 가리키는 시점에 블롭의 이름도 디스크에 있다.
3. F3 오케스트레이터는 부모 디렉터리를 sync하지 못하는 환경에서 쓰기를 돌린다 → 쓰기가 오류로 끝난다 → ==**디스크에 닿았는지 모르는 기록을 성공으로 말하지 않는다.**==
4. F4 오케스트레이터는 `canon migrate --commit`이나 레거시 이행이나 브리프 이행이나 전략 승격을 돌린다 → 모두 `write_atomic`으로 쓴다 → 쓰는 길이 하나라 F1의 보장이 거기에도 있다.
5. F5 오케스트레이터는 접근 감사가 붙는 명령(`query`, `export`, `retention`)을 돌린다 → 감사 줄이 한 번의 write로 나간다 → 줄이 둘로 쪼개지거나 다른 줄과 섞일 틈이 없다.
6. F6 오케스트레이터는 핫 계층이 sqlite인 저장소에서 `canon migrate --commit`을 돌린다 → `rekey`가 쓰기 잠금을 먼저 잡는다 → 같은 파일에 canon 밖의 쓰기가 끼어도 `database is locked`로 실패하지 않는다.
7. F7 오케스트레이터는 핫 계층이 sqlite인 저장소를 처음 연다 → `.canon/hot.db`가 소유자 전용 모드로 만들어진다 → 같은 기계의 다른 사용자는 그 파일을 읽지 못한다.
8. F8 사람은 CHANGELOG를 읽는다 → 무엇이 어떤 플랫폼에서 달라졌는지 있다 → 이미 있는 `hot.db`의 모드는 그대로이고 직접 줄이는 방법이 있다.

## Step detail

### F1 쓰기 하나의 내구성

- ==**`write_atomic`은 임시 파일 `sync_all`, `rename`, 부모 디렉터리 sync 순서다.**== 디렉터리 sync는 `rename` 뒤에 한다. 부모는 지금도 임시 파일이 놓인 같은 디렉터리다.
- 디렉터리를 sync하는 일은 `canon-store`의 작은 함수 하나로 두고 `write_atomic`과 증거 블롭 쪽이 같은 함수를 쓴다.
- 디렉터리 sync는 유닉스 전용이다. Windows는 디렉터리를 `File::open`으로 열 수 없고 이 변경은 Windows의 `rename` 동작을 바꾸지 않는다. Windows에서는 파일 sync 뒤 `rename`까지가 전부이고, 그 사실을 주석에 쓴다. Windows가 `WRITE_THROUGH`를 쓴다고 쓰지 않는다. 확인하지 못한 말이다.
- 그 디렉터리를 만들면서 새로 생긴 상위 디렉터리의 항목은 sync하지 않는다. 처음 쓰는 `kind=…` 디렉터리가 사라지면 그 기록도 사라지는데, 그 창은 한 디렉터리가 처음 생길 때 한 번뿐이고 그 뒤 쓰기는 보호된다. 이 한계를 주석에 쓴다.
- `rename`이 성공한 뒤 디렉터리 sync가 실패하면 `path`에는 새 내용이 놓여 있다. 오류는 "놓였지만 내려갔는지 모른다"를 뜻한다. 호출자는 지금까지의 실패 경로와 똑같이 처리한다. 예컨대 `git_tier`의 일괄 쓰기는 이미 쓴 기록을 지우고 오류로 돌아온다.
- 한 번에 많이 쓰는 경로는 기록마다 디렉터리 sync를 한 번씩 더 한다. 1,000개 기록 일괄 쓰기를 변경 전후로 재어 PR에 적는다. macOS는 `sync_all`이 `F_FULLFSYNC`라 비용이 가장 크다.
- 모듈 머리 주석의 "canon-store는 Windows 대상이 아니다"는 틀리다. 릴리스가 `x86_64-pc-windows-msvc`를 빌드한다. 이 주석을 바로잡는다.

### F2 증거 블롭 놓기

- 증거 블롭의 `publish_paths`는 임시 파일들을 목표 이름으로 `rename`하는 반복 뒤에, 실제로 이 호출이 놓은 블롭이 하나라도 있으면 증거 저장소 디렉터리를 한 번 sync한다. 이미 있는 블롭만이면 sync하지 않는다.
- 임시 파일은 목표와 같은 디렉터리에 만들어지므로 디렉터리 하나로 충분하다.
- 디렉터리 sync가 실패하면 이 호출이 놓은 블롭을 `unpublish`로 되돌리고 오류를 낸다. 오류가 가리키는 인덱스는 마지막으로 놓은 블롭이다. `rename` 실패 때와 같은 틀이라 호출자(`evidence_attach`)는 바뀌지 않는다.
- 블롭 내용을 `stage`에서 `sync_all`하는 것은 그대로다.

### F3 실패를 말하기

- 디렉터리 sync 실패는 그 오류 종류와 어느 디렉터리인지를 담아 호출자에게 낸다. 삼키거나 경고만 남기지 않는다.
- 디렉터리 fsync를 거부하는 파일시스템(일부 네트워크·FUSE)에서는 쓰기가 오류로 끝난다. 이것을 알고 가는 선택이다. 내려갔는지 모르는 쓰기를 성공으로 말하는 것보다 낫다고 본다.
- 시험은 유닉스에서 루트가 아닐 때 읽기 권한 없는 부모(0o300)로 만든다. 쓰기와 탐색 권한은 있어 임시 파일과 `rename`은 성공하고 디렉터리를 열어 sync하는 단계만 실패한다. 이 시험은 루트로 돌 때와 Windows에서는 건너뛴다.

### F4 한 곳으로 모으기

- `migrate.rs`의 사설 `write_atomic`(canon.yaml), `legacy.rs`의 `write_file`, `migrate_briefs.rs`의 `write_file`, `canon-learn`의 `promote.rs`의 활성화 파일 쓰기는 지우고 `canon_store::write_atomic`을 호출한다. 이 호출자들이 이미 내는 오류 문구(`failed to write …`, `could not write …`, `could not replace …`)는 그대로 둔다.
- `canon-learn`은 `canon-store`를 직접 의존하지 않는다. `canon-gate`를 거쳐 이미 의존 그래프에 있는 크레이트이므로 `canon-learn/Cargo.toml`에 `canon-store.workspace = true`를 더한다. 새 크레이트가 빌드에 더해지지 않는다.
- `write_atomic`은 부모 디렉터리를 먼저 만든다. `legacy.rs`의 사본은 만들지 않아서 있던 시험 `migrate_failures_read_as_before_and_keep_their_cause`가 "없는 디렉터리 아래에 쓰면 실패"를 쓴다. 쓰기 실패를 만드는 방법을 바꾼다(부모 자리에 파일을 둔다). 실제 호출자는 부모가 있는 파일만 쓰므로 동작은 같다.
- `tests/migrate.rs`의 두 시험(`authoring_migrate_09_…`, `atip_incomplete_10_…`)은 `.canon.yaml.canon-migrate.tmp` 자리에 디렉터리를 두어 canon.yaml 쓰기를 실패시킨다. 임시 이름이 사라지므로 이 방법이 통하지 않는다. 쓰기 실패를 만드는 다른 방법으로 바꾼다(예: 유닉스에서 저장소 루트를 읽기 전용으로). 시험이 지키는 약속 곧 "canon.yaml은 반쯤 쓰이지 않는다", "다시 돌리면 끝낸다"는 그대로다.
- 고정 임시 이름 `.canon.yaml.canon-migrate.tmp`, `*.canon-migrate.tmp`, `*.canon-tmp`, `*.md.activation`은 사라진다. 저장소 안에서 이 이름을 읽는 코드나 문서는 없다(`grep` 확인).
- `canon-store`의 `write_atomic` 호출자 8곳(`artifacts`, `context_pack`, `dispatch` 둘, `migrate_names`, `archive`, `cursor`, `git_tier` 셋)은 코드를 바꾸지 않고 F1의 보장을 받는다.

### F5 감사 줄

- 감사 줄을 `String`으로 만들어 `\n`을 붙이고 그 버퍼를 `write_all` 한 번으로 쓴다. 파일은 지금처럼 0600, 추가 모드다.
- 줄 만들기와 쓰기를 `Write` 하나를 받는 작은 함수로 뽑아, 시험이 쓰기 호출 횟수를 센다.
- 열 때 마지막 줄이 개행으로 끝나는지 보고 고치는 일은 하지 않는다. 한 번의 추가 쓰기는 틈이 거의 없고, 고치려면 읽는 일이 감사 쓰기마다 붙는다.

### F6 `rekey`

- `rekey`의 트랜잭션을 `BEGIN IMMEDIATE`로 시작한다(sqlx의 `begin_with`). 첫 `SELECT` 전에 쓰기 잠금을 잡는다.
- 다른 쓰기가 잠금을 쥐고 있으면 `busy_timeout`(5초)만큼 기다리고 그래도 안 되면 `StoreError::Sql`로 실패한다. 이것이 전과 달라지는 유일한 행동이다.
- `write_rows`는 바꾸지 않는다. 첫 문장이 `INSERT`라 잠금을 먼저 잡는다.
- 두 면 시험은 결정적이다. `rekey`의 held 확인 `SELECT`와 첫 쓰기 사이를 시험에서만 열린 틈(`cfg(test)`, 릴리스에는 없다)으로 만든다. 그 틈에서 다른 연결이 한 행을 커밋한다. 일반 `BEGIN`이면 `rekey`가 `database is locked`로 실패하고, `BEGIN IMMEDIATE`면 다른 연결의 쓰기가 막히고 `rekey`가 성공한다. 두 쪽을 한 시험이 모두 확인해서, 일반 `BEGIN`으로 되돌리면 시험이 실패해야 한다.

### F7 `hot.db` 모드

- 디스크 파일을 열 때 파일이 없으면 먼저 0600으로 만들고 그 파일을 sqlx가 연다. `-wal`과 `-shm`은 SQLite가 본 파일의 모드로 만든다.
- 이미 있는 파일은 모드를 건드리지 않는다. 읽기만 하는 명령이 파일 메타데이터를 바꾸지 않는다.
- 유닉스 전용(`cfg(unix)`)이다. Windows에서는 만드는 방식이 같다.
- `.canon/` 디렉터리, 장부 기록(`.canon/ledger/…`), `write_atomic`의 임시 파일 모드는 바꾸지 않는다. 장부는 git이 추적하고 저장소를 공유하는 사람들이 읽는 파일이다.
- 메모리 DB와 읽기 전용 연결은 해당이 없다.

### F8 안내

- CHANGELOG의 Fixed에 디렉터리 fsync, 사설 쓰기 사본 정리, 감사 줄 한 번 쓰기, `rekey`의 쓰기 잠금을 적는다. Changed에 새 `hot.db`의 0600을 적는다.
- 같은 항목에 이 변경이 유닉스만이고 Windows는 같다는 말, 그리고 이미 있는 `hot.db`를 줄이려면 `chmod 600 .canon/hot.db`(와 `-wal`, `-shm`)를 쓰라는 말을 넣는다.

## Acceptance

- F1:
  - 디렉터리 sync를 기록하는 디버그 빌드 전용 장치(`eio.rs`와 같은 종류, 릴리스에는 없다)로 `write_atomic` 한 번이 "파일 sync → rename → 부모 디렉터리 sync" 순서를 남긴다. 이 순서를 한 시험이 확인한다.
  - 실패 경로: 부모 디렉터리가 없는 경로(`/`)는 지금처럼 `InvalidInput`이고, 파일이 쓰이지 않는다.
- F2:
  - 같은 장치로 두 블롭을 놓는 `publish_paths` 한 번이 모든 `rename` 뒤 증거 저장소 디렉터리 sync 한 번을 남기고, 이미 있는 블롭만 놓는 호출은 sync를 남기지 않는다.
  - 실패 경로: 증거 저장소 디렉터리를 sync하지 못하면(F3과 같은 방법) 놓았던 블롭이 사라지고 오류다.
- F3:
  - ==**읽기 권한 없는 부모에서 `write_atomic`은 오류로 끝나고 오류가 디렉터리를 말한다.**==
  - 실패 경로: 루트 실행이나 Windows에서는 이 시험이 건너뛰어졌다고 시험 이름이 말한다.
- F4:
  - `canon-store` 밖의 소스에서 임시 파일을 만들고 `rename`하는 쓰기 사본이 없다. 소스 검색으로 확인한다. `canon.yaml` 다시 쓰기, 레거시 이행, 브리프 이행, 전략 승격이 모두 있던 시험을 통과한다.
  - 실패 경로: `canon.yaml`을 쓸 수 없을 때 `canon migrate --commit`은 종료 값 1, `MIGRATION_INCOMPLETE`, `STOPPED`로 끝나고 canon.yaml은 그대로이며 다시 돌리면 끝난다(바뀐 방법으로 만든 같은 시험).
- F5:
  - 감사 줄을 `Write` 하나에 쓰는 함수가 쓰기 호출 정확히 한 번에 JSON과 `\n`을 모두 싣는다. 시험이 호출 횟수를 센다.
  - 실패 경로: 감사 파일을 열 수 없으면 지금처럼 오류를 낸다. 파일 모드는 계속 0600이다(있는 시험).
- F6:
  - 위 틈 시험에서 `rekey`가 틈 사이에 다른 연결이 쓰기를 시도해도 성공하고, 그 쓰기는 `rekey`가 끝나기 전에 들어가지 못한다.
  - 실패 경로: 같은 시험의 일반 `BEGIN` 쪽이 `database is locked`로 실패한다. 이 면이 시험이 일반 `BEGIN`으로 되돌린 코드를 잡는다는 증거다.
- F7:
  - 유닉스에서 새 `hot.db`와 만들어진 `-wal`, `-shm`이 모두 0600이다.
  - 실패 경로: 모드가 0644인 기존 `hot.db`를 열고 써도 모드가 그대로 0644다.
- F8:
  - CHANGELOG에 위 두 항목이 있다.
  - 실패 경로: 항목이 없으면 릴리스 확인 시험이 실패한다.
- 작업공간 전체 테스트와 `canon gate check`가 통과한다.

## Questions and assumptions

- 결정: 디렉터리 sync가 실패하면 오류다. 조용히 삼키거나 경고만 하지 않는다. 채택 브리프의 기본이다. 디렉터리 fsync를 거부하는 파일시스템에서는 쓰기가 실패한다. 이 위험을 안고, 필요해지면 그때 좁은 예외를 정한다. 그 가능성은 재지 않았다.
- 결정: 디렉터리 sync는 유닉스에서만 한다. Windows는 이번 변경이 아무것도 바꾸지 않는다. 버린 방법은 Windows의 `WRITE_THROUGH`나 `MoveFileEx` 플래그를 쓰는 것이다. 이 저장소에서 확인하지 못했다.
- 결정: 사설 쓰기 사본 넷은 모두 `write_atomic` 하나로 모으고, `canon-learn`은 `canon-store`를 직접 의존한다. 채택 브리프가 연 질문(canon-learn 의존)의 답이다. 이미 의존 그래프에 있는 크레이트라 새 의존이 빌드에 더해지는 것이 없다. 버린 방법은 canon-learn 사본을 남기는 것이다. 그러면 fsync 없는 쓰기가 한 곳 남는다.
- 결정: 감사 줄은 한 버퍼를 한 번 쓴다. 열 때 개행 보정은 하지 않는다. 채택 브리프가 정한 대로다. 끊긴 마지막 줄이 이미 있는 저장소는 이 변경으로 고쳐지지 않는다.
- 결정: `hot.db`는 새로 만들 때만 0600이다. 이미 있는 파일은 그대로다. 버린 방법은 열 때마다 chmod하는 것이다. 읽기만 하는 명령이 메타데이터를 바꾸고, 다른 사용자 소유 파일에서 열기가 실패할 수 있다. 이미 있는 저장소는 안내문의 `chmod`로 줄인다.
- 결정: `rekey`의 `BEGIN IMMEDIATE`는 시험에서만 열리는 틈으로 결정적인 두 면 시험을 한다. 버린 방법은 두 작업 막대(barrier) 경합 시험이다. 일반 `BEGIN` 쪽이 우연에 기대 깜빡인다.
- 결정: 새로 생긴 상위 디렉터리 항목은 sync하지 않는다. 한 디렉터리가 처음 생길 때 한 번의 창이다. 만들어진 디렉터리를 따라가며 sync하려면 `create_dir_all`을 직접 풀어야 한다.
- 가정: 한 번에 많이 쓰는 경로의 비용 증가는 받아들일 만하다. 숫자는 구현하며 재서 PR에 적는다. 감당이 안 되는 값이면 모아서 sync하는 방법을 새 브리프로 정한다. 지금은 추측이다.
- 실패 양식: 디렉터리 sync는 되고 그 위 디렉터리가 사라지는 경우, 호출은 성공하는데 기록이 없다. 새 디렉터리의 첫 쓰기에서만 있다.
- 실패 양식: `rename` 뒤 디렉터리 sync가 실패해 `write_atomic`이 오류를 내면 `path`에는 새 내용이 있다. 호출자가 롤백하지 않으면 "오류인데 파일이 있다"가 된다. 지금 호출자 가운데 `git_tier`는 롤백하고 나머지는 오류를 올린다.

## Out of scope

- 세션 수집(ingest) 쪽 쓰기. 수집은 0.39에서 별도 변경이 걷어낸다. `cursor`는 `write_atomic`을 부르는 수집 전용 호출자이고 코드를 바꾸지 않는다.
- `fs::write`를 `write_atomic`으로 바꾸는 일 전반과 `disallowed-methods` 린트. 생산 코드에 `fs::write(` 호출이 매우 많고 시험 코드까지 합치면 `clippy.toml`에 못 넣는다.
- 제자리 `fs::write`로 쓰는 무결성 파일(`brief_approve`, 게이트의 `approval`, `policy_approval`)을 `write_atomic`으로 옮기는 일.
- 파생 캐시인 검색 색인(`search/index.rs`)의 임시 파일과 `rename`. 지워도 다시 만들어지는 파일이다.
- 심볼릭 링크 대상 해석(stow 같은 도구 사용자용).
- 접근 감사 파일의 열 때 개행 보정. 이미 끊긴 줄을 고치는 도구.
- `.canon/` 디렉터리 모드 0700, 장부 기록 파일과 임시 파일의 모드, 이미 있는 `hot.db`의 chmod.
- `hot.db` 스키마 버전 사다리, `quick_check`, `auto_vacuum`, 연결 풀 닫기 순서.
- `write_rows`와 Postgres `rekey`.
- 증거 블롭 임시 파일의 이름과 정리 정책.

Approved: Journey Hwang 2026-11-08T06:11:30Z — "승인 (canon 0.39 브리프 7개와 정책)" (relayed by planner planner-oco via ask) (code tree 7c28ab134d4701f16f343da5e87c7582b9521417) (brief sha256:2b4394afff2b8ddce5554dfee1c640bf074353cbbe140c795af30b0a9048f733)
