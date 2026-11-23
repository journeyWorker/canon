---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "저장소 조사, 옛 설정 처리표, 없어지는 것의 표가 구현자가 지켜야 하는 조건이다"
  - rule: "prose.length"
    section: "intent"
    reason: "조사 표가 이 변경의 근거 수치다"
  - rule: "prose.length"
    section: "outcome"
    reason: "저장 설정 모양과 위치 표가 결과의 본체다"
  - rule: "prose.length"
    section: "flow"
    reason: "일곱 단계가 각각 사용자 흐름 한 줄이다"
  - rule: "prose.length"
    section: "detail"
    reason: "migrate가 옛 설정마다 하는 일과 없어지는 것의 표를 열거해야 구현자가 다시 조사하지 않는다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "단계마다 점검과 실패 경로가 있어야 한다"
  - rule: "prose.length"
    section: "questions"
    reason: "오너가 답할 실제 상충 다섯과 어림 근거가 한 곳에 있어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "설정 키와 파일 이름이 이 변경이 바꾸는 대상의 철자다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "설정 키와 경로는 사용자가 보는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "명령과 설정 키는 사용자가 치는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "오류 코드, 사유, 필드, 경로는 사용자와 스크립트가 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "점검은 코드, 필드, 경로의 철자를 그대로 말해야 같은 것을 돌린다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "질문은 설정 키와 환경 변수 이름으로 물어야 오너가 같은 것을 본다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 백엔드와 저장소 이름의 목록이다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 코드와 필드의 목록이다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "괄호는 주제별 시나리오 수의 목록이다"
  - rule: "prose.sentence-length"
    section: "detail"
    reason: "의존성과 CI 항목을 열거한 문장은 나누면 대응이 끊긴다"
  - rule: "prose.sentence-length"
    section: "acceptance"
    reason: "점검마다 조건, 코드, 필드를 한 문장에 열거해야 한다"
  - rule: "prose.paragraph-length"
    section: "detail"
    reason: "migrate, init, F7의 규칙은 한 항목에 묶여야 대응이 끊기지 않는다"
  - rule: "prose.paragraph-length"
    section: "acceptance"
    reason: "한 단계의 성공과 실패 점검이 한 항목이다"
  - rule: "prose.key-sentence"
    section: "detail"
    reason: "F1과 F5에 각각 놓치면 안 되는 규칙이 있어 굵은 문장이 여럿이다"
---

# canon은 기록을 한 곳에 둔다: 계층·배치·보관 이동과 Postgres·S3를 없애고 canon.yaml의 `storage:` 한 키로 줄인다

Source: 오너 방향(2026-11-19~23, 대화), canon 0.45.0(`996293df4`), `crates/canon-store/src/policy.rs:498,639`(`TierPolicy`), `crates/canon-store/src/registry.rs:76,379`(`TierRegistry`, `age_all`), `crates/canon-store/src/pg_tier.rs`, `r2_tier.rs`, `artifact_blobs.rs:60`(`ArtifactsBackend`), `crates/canon-cli/src/tiers.rs:131`, `tier.rs:53`, `artifacts.rs:431`(`run_move`), `init.rs:69`(`HOT_KINDS`), `canon.yaml:27-29,61-66`, `.github/workflows/test.yml:600`(`live-tier`), `publish.yml:505`(`smoke-network`), `justfile:14`, `crates/canon-store/Cargo.toml:19-28`, 작업 폴더의 canon.yaml 아홉 곳과 `.canon/hot.db`를 읽기만 한 조사(2026-11-23)

## Intent

canon은 기록 종류마다 저장 위치를 `canon.yaml`로 정한다. `tiers:`는 local·hot·cold 세 계층에 백엔드(git, sqlite, postgres, s3)를 붙이고, `routing:`은 기록 종류 열세 개를 계층에 배치하고, `aging:`은 오래된 기록을 다른 계층으로 옮기고, `artifact_store:`는 증거 파일의 바이트를 객체 저장소에 둔다. 이 선택지를 받치려고 canon은 Postgres와 S3 클라이언트, 환경 변수 아홉 개, 오류 코드 열 개와 경고 코드 넷, CI 작업 둘(컨테이너 둘을 띄우는 `live-tier`, 정적 바이너리의 네트워크 스모크 `smoke-network`)을 유지한다. 저장소 아홉 곳의 설정은 이 선택지를 거의 쓰지 않는다.

| 저장소 | hot | cold | 증거 바이트 | 읽은 값 |
| --- | --- | --- | --- | --- |
| 일곱 곳(3d-oshiz-lab, intranet, eevee, forca, kino, pixel-forge, quire) | sqlite | 없음 | git | 기본값 그대로. `.canon/hot.db`는 kino(4행)와 pixel-forge(0행)에만 있다 |
| canon 자기 저장소 | postgres(연결 안 됨) | s3(비어 있음) | git | `trajectory`는 cold, `handoff`는 30일 뒤 cold. `.canon/hot.db`가 없다 |
| oshiz-qa | sqlite | s3 | 객체 저장소(NAS의 S3) | `artifact_store`가 cold를 가리킨다. 증거 기록 1,168개 |

canon은 저장 방식을 둘로 줄인다. 전부 로컬이거나 전부 원격(canon 서버)이다. 이 변경은 로컬 모드만 만든다. 원격 모드는 팀의 기록·메모리 구조가 정해진 뒤에 따로 다루고, 이 변경은 그것을 설계하지도 자리만 잡아 두지도 않는다.

## Desired outcome

- `canon.yaml`에 `tiers:`, `routing:`, `aging:`, `artifact_store:`가 없다. 저장 설정은 선택 키 `storage:` 하나다.

  ```yaml
  storage:
    mode: local        # 이 릴리스의 유일한 값이고, 키가 없을 때의 값이다
    attachments: git   # git(기본) 또는 lfs
  ```

- ==**기록이 놓이는 곳은 고정이고 설정으로 바꿀 수 없다.**==

  | 기록 | 위치 | git에 올리는가 |
  | --- | --- | --- |
  | `task`, `handoff`, `run` | `.canon/hot.db`(sqlite, 작업 트리마다 하나) | 아니오 |
  | 나머지 열 종류 | `.canon/ledger/`(최신 버전)와 `.canon/archive/`(대체된 버전) | 예 |
  | 증거 첨부 바이트 | `.canon/artifacts/sha256/<다이제스트>` | 예. `attachments: lfs`이면 LFS로 |
  | 학습 parquet | `.canon/learn/` | 아니오(지금과 같다) |

- 옛 설정(위 네 키 중 하나)은 거절된다. 거절은 한 줄이고, `canon migrate`가 파일을 고친다. 옛 설정을 읽는 호환 경로, 별칭, 경고 뒤 계속 실행은 없다.
- 없어지는 것은 F6의 표와 F7이 정한다. 명령 둘, 환경 변수 아홉 개, 오류·경고 코드, Postgres·S3 의존성, CI 작업 둘이다.
- 로컬 저장소에서 기록, 종료 코드, 보고서 값은 지금과 같다. 달라지는 출력 필드는 F6의 표가 전부다.

## User flow

1. F1 ==**옛 `canon.yaml`을 가진 사용자가 저장소를 읽는 명령을 돌린다 → 한 줄 거절과 `canon migrate`를 본다 → 미리보기를 보고 `--commit --plan-digest`로 적용한다 → `canon.yaml`이 `storage:` 블록으로 바뀌고 명령이 다시 돈다.**==
2. F2 원격 설정(Postgres, S3, 객체 저장소 증거)을 가진 사용자가 `canon migrate`를 돌린다 → 옮겨지지 않는 것과 막는 것을 미리보기에서 본다 → 가져갈 것이 없으면 적용하고, 막는 것이 있으면 거절을 본다 → 저장소가 로컬 모드가 되거나 그대로 남는다.
3. F3 새 저장소의 사용자가 `canon init`을 돌린다 → `storage:` 블록이 든 `canon.yaml`을 본다 → 증거, 지적, 실행을 기록한다 → 기록이 위 표의 자리에 놓인다.
4. F4 로컬 모드 사용자가 지금 하던 일을 한다(`gate check`, `query`, `search`, `report`, `export`, `purge`, `dispatch`) → 같은 값을 본다 → 작업이 끊기지 않는다.
5. F5 큰 증거 파일을 쓰는 사용자가 `attachments: lfs`를 고른다 → 증거를 기록한다 → 바이트가 git LFS로 올라가고, git-lfs가 없거나 포인터만 받은 클론은 이유와 함께 막힌다.
6. F6 없어진 명령·옵션·환경 변수를 쓰던 사람이 그것을 부른다 → 사용법 오류를 본다 → 로컬 모드의 길을 따른다.
7. F7 기여자가 canon을 빌드하고 CI를 돌린다 → Postgres·S3 의존성과 두 컨테이너 작업이 없다 → 빌드 그래프와 CI가 그만큼 줄어 있다.

## Step detail

### F1 옛 설정은 거절되고, `canon migrate`가 고친다

- ==**기록 저장소를 여는 모든 명령은 옛 키를 만나면 거절한다.**== `canon migrate`와 저장소를 열지 않는 명령(`format`, `brief check`, `schema`)은 거절하지 않는다. 종료 코드 2, 코드 `CONFIG_INVALID`, 사유 `STORAGE`(옛 사유 `TIERS`와 `ARTIFACT_STORE`를 대체한다), 필드는 찾은 키 이름이다. 본문은 한 줄이다: 어떤 키를 찾았는지, `canon migrate`를 돌리라는 것. 아무것도 쓰지 않고 `retryable`은 false다. `canon init --check`도 같다.
- ==**`canon migrate`는 네 블록을 지우고 `storage:`를 쓴다.**== 이미 있는 migrate의 규칙을 그대로 따른다: 미리보기(`planDigest`), `--commit --plan-digest`로 적용, `--revert`로 되돌림(백업은 `.canon/migrate-backup/`). 편집은 텍스트 편집이다. 키 블록과 그 바로 위에 붙은 주석을 지우고, `tiers:`가 있던 자리(없으면 끝)에 `storage:`를 쓰고, 결과가 의도한 변경만 담지 않으면 거절한다. 다른 키는 바이트 그대로다. 저장소의 파일, `.canon/hot.db`, `.canon/ledger`는 옮기지 않는다.
- migrate는 `routing`과 `aging`의 항목을 읽어 고정 위치와 다른 것이 있는지 본다. 같은 항목은 조용히 지워지고, 다른 항목은 F2가 백엔드별로 정한다.
- 둘째 실행은 할 일이 없다고 답한다. 0.30 이전 저장소의 Postgres·S3 재키잉 단계는 없어진다. 그런 저장소는 canon 0.45로 먼저 올린다.
- 실패 경로: 파일을 읽을 수 없거나 편집 결과가 의도와 다르면 거절하고(종료 코드 2) 파일은 그대로다. 미리보기 뒤에 `canon.yaml`이 바뀌었으면 `PLAN_STALE`이다.

### F2 원격 설정은 가져가지 않거나 막는다

canon은 Postgres와 S3 클라이언트를 잃으므로 원격 저장소에서 읽어 오지 못한다. 그래서 migrate는 원격에 있는 것을 옮기지 않고, 바이트가 모자라 게이트가 깨질 때만 막는다.

| 옛 설정 | migrate의 처리 |
| --- | --- |
| `tiers.hot`이 postgres | 적용한다. 미리보기가 경고 `STORAGE_ROWS_NOT_CARRIED`(스키마 이름을 필드로)를 낸다. 로컬 `.canon/hot.db`는 비어 시작한다 |
| `tiers.cold`가 s3 | 적용한다. 같은 경고를 낸다(버킷 이름을 필드로) |
| `tiers.local.root`가 `.canon/ledger`가 아니거나 `tiers.hot.path`가 `.canon/hot.db`가 아님 | 거절한다(종료 코드 2, 사유 `STORAGE`, 필드는 키). 디렉터리를 기본 위치로 옮기고 다시 돌린다 |
| `routing.<종류>` 또는 `aging.<종류>.to`가 원장 종류(열 종류)를 로컬 백엔드(git 또는 sqlite)의 고정 위치와 다른 곳에 보냄. 예: `evidence: hot`이 sqlite, `aging: { evidence: { to: hot } }`가 sqlite, `task: local` | 거절한다(종료 코드 2, `CONFIG_INVALID`/`STORAGE`, 필드는 그 항목 `routing.evidence` 또는 `aging.evidence`). 그 행이 로컬에 남아 있고 지우면 아무 읽기도 찾지 못하게 되기 때문이다. `.canon/hot.db`나 원장을 옮기고 다시 돌린다 |
| 같은 항목이 postgres 또는 s3 쪽을 가리킴. 예: `evidence: cold`, `aging: { handoff: { to: cold } }`, postgres hot에 `task: hot` | 적용한다. 경고 `STORAGE_ROWS_NOT_CARRIED`를 항목 이름으로 낸다(위의 `tiers.hot`·`tiers.cold` 행의 경고와 한 번만 센다). 오너가 정한 대로 원격 행은 옮기지 않는다 |
| `artifact_store`가 객체 저장소 | 거절한다. 게이트가 읽을 바이트가 로컬에 없게 된다. 안내는 한 줄이고, CHANGELOG가 바이트를 로컬로 내려받는 방법을 싣는다 |
| `artifact_store: { backend: local }` 또는 키 없음 | 적용한다 |

- `trajectory`는 두 행 모두의 예외다. `routing.trajectory`와 `aging.trajectory`는 어느 백엔드를 가리키든 경고 없이 지운다. 근거: 학습 보관함은 `.canon/learn`의 parquet이고 `RecordKind::Trajectory`를 계층에 쓰는 호출이 없다[INFERENCE: 쓰는 호출을 찾아 읽었고 실행으로 확인하지 않았다].
- 객체 저장소의 바이트는 `<접두사>sha256/<다이제스트>.zst`(zstd 한 프레임) 또는 옛 평문 이름이다. 로컬 이름은 `.canon/artifacts/sha256/<다이제스트>`이고 `canon artifacts verify --full`이 내려받은 결과를 다시 해시로 검증한다.

### F3 새 저장소

`canon init`은 `storage:` 블록(`mode: local`, `attachments: git`)을 쓰고 `tiers:`·`routing:`·`aging:`·`artifact_store:`를 쓰지 않는다. 지금 쓰는 `.canon/r2` 무시 줄은 더 쓰지 않는다. 나머지 `.gitignore` 줄과 `AGENTS.md` 블록, 시작 정책은 같다. `canon init --check`는 `storage` 항목과 `.canon/hot.db`를 여는 점검(열 수 없으면 실패)을 낸다. 기록 종류는 위치 표대로 놓이고 `task`·`handoff`·`run`을 쓰는 명령은 환경 변수나 서버 없이 돈다. 실패 경로: `storage:`의 값이 닫힌 집합(`mode`는 `local`, `attachments`는 `git`·`lfs`) 밖이거나 모르는 키가 있으면 `CONFIG_INVALID`/`STORAGE`, 필드는 그 키다. `mode: remote`도 같은 거절이고, 원격을 위한 별도 분기나 안내는 없다.

### F4 로컬 모드의 동작은 같다

- 기록의 내용과 위치, 모든 명령의 종료 코드와 `--json`의 `data`는 지금과 같다. 달라지는 필드는 F6의 표뿐이다.
- 한 종류만 읽는 명령은 그 종류의 저장소만 연다. `.canon/hot.db`가 열리지 않아도 `query --kind evidence`는 돈다. `--kind task`는 이름을 대고 실패한다.
- `purge`와 `export`는 원장과 `.canon/hot.db`를 지금처럼 다룬다(퇴역한 `session`·`event`의 행 삭제 포함).
- 보고서(`report build|check|snapshot`)는 DuckDB 뷰로 같은 값을 낸다. `source_tier`는 `git`과 `hot` 두 값이고 `r2` 가지와 `stg_r2_records`, 조사용 시드 parquet이 없어진다. 학습 parquet(`.canon/learn`)은 계층과 무관해 그대로다. 보고서의 "직접 읽지 않는 종류" 구역과 경고는 모든 종류를 직접 읽으므로 나올 일이 없어 없어진다.
- `canon search`와 `canon trace`는 기록 입력과 색인이 같다. `unreadStores` 필드는 없어진다(읽지 못하는 원격 저장소가 없다).

### F5 `attachments: lfs`

canon은 LFS 프로토콜을 쓰지 않는다. 바이트는 같은 경로 `.canon/artifacts/sha256/<다이제스트>`에 실제 내용으로 놓이고, git의 LFS 필터가 `git add` 때 포인터로 바꾼다. canon이 더하는 것은 넷이다.

- ==**바이트를 쓰기 전에 필터 파일을 보장한다.**== 증거나 검토 첨부를 저장할 때 `.canon/artifacts/.gitattributes`에 `sha256/* filter=lfs diff=lfs merge=lfs -text`가 없으면 만든다. `canon migrate`도 `attachments: lfs`인 저장소에서 그 줄이 없으면 더한다(이미 빠진 `.gitignore` 줄을 더하는 단계와 같은 자리이고, 바꿀 것이 없는 저장소에서도 돈다). 새 저장소의 `canon init`은 `git`만 쓰고, 이미 `canon.yaml`이 있는 저장소에서는 거절하므로(`CONFIG_EXISTS`) 줄을 쓰지 않는다. `canon init --check`는 `attachments: lfs`인데 그 줄이 없으면 실패하고 고치는 명령으로 `canon migrate`를 가리킨다.
- ==**git-lfs가 없으면 쓰기 전에 거절한다.**== 첨부를 저장하는 명령(`evidence add`, `review scenario --artifact`, `artifacts store`)은 `git lfs version`이 성공하고 `git config filter.lfs.clean`이 값을 가질 때만 바이트를 쓴다. 아니면 종료 코드 2, 기존 코드 `ARTIFACTS_STORE_REQUIRED`이고 아무것도 쓰지 않는다. 필터 없이 올라간 바이트가 git에 평문으로 남는 일을 막는 규칙이다.
- ==**LFS 포인터 파일은 받지 않은 바이트로 읽는다.**== 블롭 이름과 같은 `oid sha256:`를 가진 포인터 파일(`version https://git-lfs.github.com/spec/v1`로 시작하는 1KiB 이하 파일)은 `artifacts verify`에서 상태 `lfs_pointer`(종료 코드 1)이고, 게이트는 그 첨부를 위반으로 센다. 안내는 `git lfs pull`이다. 포인터를 "바이트가 바뀜"으로 읽지 않는다.
- 이미 git에 평문으로 든 블롭은 canon이 바꾸지 않는다. `git add --renormalize`와 기록 다시 쓰기(`git lfs migrate`)는 사용자의 git 명령이다.

실패 경로: `attachments: lfs`인데 `git`이 없거나 git-lfs가 없으면 위 거절이다. 읽기 명령은 git-lfs 없이도 돈다.

### F6 없어진 것

| 종류 | 없어지는 것 | 사용자가 보는 것 |
| --- | --- | --- |
| 명령 | `canon tier age`(`tier` 그룹째), `canon artifacts move`, `--tier` | 모르는 명령/옵션: 종료 코드 2, `USAGE`. 대체 안내 없음. 옛 이름 표(`removed`)에서 이 둘을 가리키던 행은 지운다 |
| 오류 코드 | `TIER_FAILED`, `TIER_INCOMPLETE`, `TIER_UNAVAILABLE`, `ARTIFACTS_MOVE_INCOMPLETE`, `ARTIFACTS_TIER_INVALID`, `RETENTION_TIERS_REQUIRED` | `canon schema --errors --json`에 없다 |
| 오류 코드(이름 바뀜) | `RETENTION_TIERS_DENIED`·`_INVALID`·`_UNAVAILABLE`·`_CORRUPT`는 로컬 원인이 남는다(`_CORRUPT`는 `purge`·`export --kind trajectory`가 읽는 `.canon/learn`의 깨진 parquet) | `RETENTION_STORE_DENIED`·`_INVALID`·`_UNAVAILABLE`·`_CORRUPT`로 바뀌고 뜻에서 계층과 백엔드가 빠진다 |
| 사유 | `CONFIG_INVALID`의 `TIERS`, `ARTIFACT_STORE` | `STORAGE` 하나 |
| 경고 코드 | `DISPATCH_TIER_NOT_PERSISTED`, `LEDGER_KIND_ROUTED_AWAY`, `REPORT_TIER_NOT_READ_DIRECTLY`, `RETIRED_KIND_KEYS` | `canon schema --warnings --json`에 없다. 새 경고는 `STORAGE_ROWS_NOT_CARRIED` 하나다 |
| 출력 필드 | `purge`의 `rungs[]`(`rung`, `backend`)와 `unavailable`, `search`·`trace`의 `index.unreadStores`, `hotStore`의 "없을 수 있음" | `purge`는 `stores[]`(`store`는 `ledger` 또는 `hot`, `matched`, `deleted`)를 낸다. `hotStore`는 `.canon/hot.db` 경로로 항상 있다 |
| 환경 변수 | `CANON_PG_DSN`, `CANON_R2_BUCKET`, `CANON_R2_LOCAL_ROOT`, `CANON_R2_ROOT`, `CANON_S3_ENDPOINT`·`_BUCKET`·`_REGION`·`_ACCESS_KEY`·`_SECRET_KEY`, `CANON_REQUIRE_LIVE` | canon이 읽지 않는다(설정해도 아무 일도 없다) |

선언(`canon schema --json`, `docs/cli/canon.cli.json`)은 다시 만든다. `contractVersion`은 ATIP 규격의 판을 따르므로 이 변경이 올리지 않는다. `hot`이라는 말은 `.canon/hot.db` 파일 이름, `hotStore` 필드, 보고서의 `source_tier` 값에만 남는다.

### F7 기여자

`canon-store`는 `object_store`와 `arrow`·`parquet`(`canon-learn`과 `canon-report`는 각자 계속 쓴다)을 잃고, `sqlx`는 `postgres`·`tls-rustls` 기능을 잃어 `sqlite`·`chrono`·`json`·`runtime-tokio`만 남긴다. 남는 코드가 쓰지 않는 그 밖의 의존성(`bytes`, `futures`, `zstd`, 개발용 `async-trait`·`uuid`)도 뺀다. 기능 `live-pg`와 `live-r2`가 없어진다. `Cargo.lock`은 425개 중 대략 66개를 잃는다[INFERENCE: 잠금 파일의 그래프에서 `object_store`와 `sqlx-postgres` 뿌리를 뺀 값이고, 다른 뿌리가 `rustls`를 쥐면 줄어든다]. CI에서 `live-tier`, `smoke-network`, 그 경로 필터와 `record`의 `needs`, `clippy`의 live 단계가 없어지고, `docker-compose.yml`, `docker/`, `justfile`의 `live`와 `live_env`, `release-smoke.sh network`, 릴리스 점검기(`check-release-workflow-safety.py`)와 그 시험·픽스처의 `smoke-network` 요구가 없어진다. 정적 바이너리에는 네트워크 호출이 남지 않는다[INFERENCE: `canon dashboard`는 루프백 서버뿐이다].

## Acceptance

- F1 ==**옛 키 네 개(`tiers`, `routing`, `aging`, `artifact_store`)는 각각 거절되고 `migrate`가 고친다.**== 기본 `canon.yaml`에 키를 하나씩 더한 저장소에서 `status`, `query --kind evidence`, `gate check`, `init --check`는 종료 코드 2, `error.code` `CONFIG_INVALID`, 사유 `STORAGE`, 필드 그 키이고, 작업 트리의 디렉터리 해시는 그대로다. `migrate` 미리보기는 종료 코드 0이고 `planDigest`를 내며 아무것도 쓰지 않는다. `--commit --plan-digest`는 네 블록만 지우고 `storage:`를 쓴다(다른 줄의 diff가 비어 있다). 그 뒤 위 명령들이 돈다. 둘째 `migrate`는 바꿀 것이 없다고 답한다. `--revert`는 파일을 바이트 그대로 되돌린다. 실패: 미리보기 뒤 파일이 바뀌면 `PLAN_STALE`이고, 편집을 안전하게 할 수 없는 파일은 종료 코드 2이며 그대로다.
- F2 원격 설정은 가져가지 않거나 막고, 로컬 행이 갈 곳을 잃는 설정은 막는다. 표의 행마다 한 저장소: postgres hot, s3 cold, `evidence: cold`, postgres hot의 `task: hot`은 적용되고 `meta.warnings`에 `STORAGE_ROWS_NOT_CARRIED`가 있다. 기본이 아닌 `root`/`path`, 로컬 쪽 오배치(`routing: { evidence: hot }`가 sqlite, `aging: { evidence: { after: 90d, to: hot } }`가 sqlite, `task: local`), 객체 저장소 `artifact_store`는 종료 코드 2, `CONFIG_INVALID`/`STORAGE`, 필드는 그 항목이고 `canon.yaml`의 해시가 그대로다. `routing.trajectory`와 `aging.trajectory`는 어느 백엔드든 경고 없이 지워진다.
- F3 빈 저장소에서 `canon init` 뒤 `canon.yaml`이 파싱되고 네 옛 키가 없고 `storage.mode`가 `local`, `storage.attachments`가 `git`이다. `subject new`, `evidence add`, `dispatch begin`/`end` 뒤 `.canon/ledger/kind=evidence/` 아래에 증거 파일이 있고 `query --kind run --json`의 `count`가 1이며 그 행이 `.canon/hot.db`에 있다. `init --check`는 종료 코드 0이고, `.canon/hot.db`를 파일이 아닌 디렉터리로 두면 1이다. `mode: remote`, `attachments: s3`, 모르는 `storage` 키는 `CONFIG_INVALID`/`STORAGE`다. `attachments: lfs`로 바꾼 저장소에서 `.gitattributes`의 줄이 없으면 `init --check`가 1이고, `canon migrate --commit`이 줄을 쓰면 0이다.
- F4 적합성 말뭉치 117건이 통과한다. 픽스처의 `canon.yaml`만 새 모양으로 다시 쓰고, `expected/`가 바뀌는 것은 F6 표의 필드와 거절 사례뿐이며 바뀐 사례의 목록을 증거 메모에 남긴다. 같은 픽스처 저장소의 `report snapshot` 표들이 변경 전후로 같다(`source_tier`의 `r2` 행만 없다). `.canon/hot.db`를 깨뜨린 저장소에서 `query --kind evidence`는 종료 코드 0이고 `--kind task`는 1이다. `purge --kind session`은 원장과 `.canon/hot.db`의 행을 지운다.
- F5 `git-lfs`가 있는 환경에서: `attachments: lfs`로 `evidence add`한 뒤 `.canon/artifacts/.gitattributes`가 그 줄을 갖고, `git add` 뒤 색인의 블롭이 LFS 포인터이고(`git cat-file -p`가 `version https://git-lfs.github.com/spec/v1`로 시작), `artifacts verify --full`이 종료 코드 0이다. 블롭 파일을 포인터 텍스트로 바꾸면 `artifacts verify`가 종료 코드 1에 상태 `lfs_pointer`이고 `gate check`가 위반을 내며, 실제 바이트로 되돌리면 깨끗하다. 실패: 실행 파일 경로에서 `git-lfs`를 숨기면 첨부를 저장하는 세 명령이 종료 코드 2, `ARTIFACTS_STORE_REQUIRED`이고 `.canon/artifacts`가 그대로다. 시험은 `git lfs version`이 실패하면 건너뛰지 않고 실패한다.
- F6 `canon tier age`와 `canon artifacts move --tier cold`는 종료 코드 2, `USAGE`이고 쓰는 것이 없다. `canon schema --json`에 `tier` 그룹과 `artifacts move`가 없고, `--errors`와 `--warnings`에 위 표의 코드가 없고 `STORAGE_ROWS_NOT_CARRIED`가 있다. `purge --json`의 `data.stores[].store`가 `ledger`·`hot`이고 `rungs`가 없다.
- F7 `Cargo.lock`에 `object_store`와 `sqlx-postgres`가 없고 `cargo tree -p canon-store`에 `arrow`·`parquet`이 없다(`canon-learn`·`canon-report`는 `arrow`·`parquet`을 계속 쓴다). `.github/workflows`에 `live-tier`와 `smoke-network`가 없고 릴리스 점검기와 그 시험이 통과한다. `docker-compose.yml`과 `docker/`가 없다. `just lint`, 워크스페이스 시험, `canon gate check`가 통과한다. 시험은 오류 `message`·`hint` 문구를 단언하지 않고 종료 코드, 코드, 사유, 필드, 해시, 개수만 본다.
- 문서: CHANGELOG의 Unreleased에 깨지는 변경 한 구역(`canon migrate` 한 줄, 없어진 것의 목록, 객체 저장소 바이트를 내려받는 방법, 위험이 남는 저장소)을 싣는다. `canon-storage` 가이드, 역할 가이드, `README.md`, `ARCHITECTURE.md`, `docs/design/ledger-layout.md`, 사이트의 `tiered-storage`(영어·한국어)가 로컬 모드를 말한다.

## Questions and assumptions

- 질문 1(오너, 아직 열림): oshiz-qa는 증거 바이트(스크린샷, 영상)를 NAS의 S3에 두고 git에는 아무것도 두지 않는다. 로컬 모드는 바이트를 git이나 LFS에 두는 길뿐이어서 이 저장소는 0.46으로 옮겨지지 않는다. 어느 쪽이든 코드는 같다. migrate는 객체 저장소 `artifact_store`를 거절하고(F2), 내려받기 명령은 없다. (A) 원격 모드가 나올 때까지 canon 0.45에 머문다. (B) CHANGELOG에 적은 손 절차로 바이트를 `.canon/artifacts/sha256/`에 내려받은 뒤 migrate를 돌린다. 오너의 답이 어느 쪽이든 이 변경의 범위는 바뀌지 않고 CHANGELOG의 문장만 달라진다. `artifacts fetch` 같은 한 번 쓰는 명령은 넣지 않는다(S3 클라이언트를 한 릴리스 더 들고 가므로 "호환 코드 없음" 규칙과 어긋난다).
- 결정(오너, 2026-11-23): Postgres 행과 S3 객체는 옮기지 않고 경고(`STORAGE_ROWS_NOT_CARRIED`)만 낸다. 조사한 아홉 곳 가운데 Postgres나 S3에 기록이 있는 곳은 없다(canon 자기 저장소의 Postgres는 연결된 적이 없다. oshiz-qa의 증거 바이트는 질문 1이다).
- 결정(오너, 2026-11-23): `attachments: lfs`를 0.46에 넣는다. 크기는 키 하나, 한 줄짜리 파일, 사전 점검 하나, 포인터 판별 하나이고 시험은 `git-lfs`가 있는 환경을 요구한다.
- 결정(오너, 2026-11-23): `storage:`는 선택 키다. 없으면 로컬 모드와 `git` 첨부다.
- 결정(오너, 2026-11-23, 승인은 오케스트레이터에게 위임): 이 변경을 주제 열한 곳에 채택한다. `tier-storage`(시나리오 58), `evidence-attestation`(17), `atip-contract`(12), `report-generation`(7), `atip-schema`(5), `authoring-scaffold`(4), `context-capability`(4), `ingest-pipelines`(4), `search-index`(4), `subject-lifecycle`(3), `query-reads`(2)다[INFERENCE: 시나리오 1,453개 가운데 저장 관련 낱말이 든 약 120개를 키워드로 센 값이고 하나씩 분류하지 않았다]. 출시된 주제를 다시 출시할 때 변경 수준 검토 하나와 증거가 필요하다.
- 결정(오너, 2026-11-19~23): 저장 방식은 전부 로컬이거나 전부 원격이다. 이 변경은 로컬만 만든다. 원격 모드, 오프라인 쓰기 캐시는 하지 않는다. 호환 코드를 두지 않고 옛 설정은 거절한 뒤 `canon migrate`로 고친다. 테스트는 문구를 바이트 단위로 고정하지 않는다.
- 결정(오너): 로컬 모드에서도 `task`·`run`·`handoff`는 지금 자리(`.canon/hot.db`, git에 올리지 않음)에 그대로 둔다. 옮기거나 다시 설계하지 않는다.
- 가정(구현자의 몫): 위치 표를 한 곳에서 정하는 함수(기록 종류 → 원장 또는 `.canon/hot.db`)를 두고, `TierPolicy`·`TierRegistry`·`LoadedTiers`의 자리를 이것이 잇는다. 이름과 모양은 구현이 정한다. 한 종류만 읽을 때 그 종류의 저장소만 여는 지금의 동작은 지킨다.
- 가정: 삭제 규모는 파일 크기로 어림한 값이다(실측 아님). 프로덕션 코드 약 6,900줄(`pg_tier` 762, `r2_tier` 1,197, `artifact_blobs` 935, `policy` 약 950 순감, `registry` 약 350, CLI `tiers`·`tier`·`artifacts`·`artifacts_prune`·`retention` 약 1,400, 게이트의 객체 저장소 약 350, 보고서의 경계·시드 약 400, 나머지 호출처 약 600), 시험과 픽스처 약 4,400줄(전체 파일: `pg_tier_live`, `r2_tier_live`, `ledger_pg_live`, `artifacts_object_store`, `artifacts_zstd`의 객체 저장소 시험, `tier_age`, `report_tier_boundary`, canon-store의 `e2e_write_age_query_duckdb`와 `query_tier_degradation`·`write_unavailable`·`session_kinds_removal`의 일부), 문서·스킬·CI·docker 약 1,500줄을 지운다. canon-store의 `e2e_scope_status_duckdb`는 `R2Tier`로 시드하던 부분을 원장 기록으로 바꿔 남긴다. `e2e_write_age_query_duckdb`가 보던 보관 이동 뒤 읽기는 이 변경이 없애는 동작이고, 원장과 `.canon/hot.db`를 뷰로 읽는 일은 `report_hot_tier`와 canon-report의 마트 시험이 이미 본다[INFERENCE: 두 파일을 읽었고 실행으로 대응을 확인하지 않았다]. 더하는 것은 설정 읽기와 migrate 단계 약 400줄과 LFS 약 300줄이다. 삭제 약 1만 2,800줄, 추가 약 700줄이다. 구현자는 실제 diff 크기를 CHANGELOG에 적는다.
- 가정: 시나리오 약 45개는 이 변경으로 의미를 잃어 `.feature`에서 지우고(0.43이 한 것처럼), 나머지 약 75개는 문장이나 예시만 고친다. 지우는 쪽은 계층·백엔드 짝(`tier.storage.01-07`), 보관 이동, 연결 안 되는 계층, 객체 저장소 증거(`artifacts.object-zstd.01-10`, `evidence.add.118-120`), `tier.retired`의 옛 키 사례, 보고서 경계 경고다. `artifacts.object-zstd.11`·`.12`(로컬 저장소는 평문 블롭만 쓰고 `.zst`를 쓰지 않는다, 블롭 디렉터리의 `<다이제스트>.zst`는 블롭이 아니다)는 지키는 계약이 남으므로 객체 저장소 말을 빼고 고쳐 쓰며 저장소·정리(prune) 표면 아래로 옮기고, 로컬 전용 시험은 `artifacts_zstd.rs`에서 남긴다. 오류 `RETENTION_TIERS_CORRUPT`의 `atip.codestored` 시나리오는 `RETENTION_STORE_CORRUPT`와 `.canon/learn`의 깨진 parquet으로 고쳐 쓴다. 새 시나리오(주제 `tier-storage`)는 F1~F7마다 성공과 실패 하나씩이다.
- 가정: 이 계획은 cargo를 돌리지 않았다. 줄 번호는 `996293df4` 기준이고 구현자가 이름으로 다시 찾는다.

## Out of scope

- 원격 모드(canon 서버): 설정, 프로토콜, 인증, 자리 잡기, `mode: remote`의 수용. 오프라인 쓰기 캐시.
- `task`·`run`·`handoff`의 저장 위치, 파일 이름(`.canon/hot.db`), 작업 트리마다 하나라는 성질. 작업 트리 사이 공유.
- Postgres 행이나 S3 객체를 꺼내 오는 일, 증거 바이트를 객체 저장소에서 내려받는 canon 명령.
- git 기록의 LFS 변환(`git lfs migrate`), LFS 서버 설정, `git lfs pull`의 자동 실행.
- 보고서 마트의 설계, `canon-learn`의 parquet 형식. `policy.yaml`의 `risk_tiers`, `risk_routing`.
- 주제 `tier-storage`의 이름 바꾸기, `area=tier` 디렉터리 이름 바꾸기, 남아 있는 `.canon/r2` 폴더 지우기.
- 0.30 이전 저장소의 Postgres·S3 재키잉(canon 0.45로 먼저 올린다).

Approved: journey 2026-11-23T06:34:40Z — "Approved on the owner's delegation (ask, 2026-11-23) after independent plan review r1 (revise, 1 P1) and r2 (approve, no P0/P1). The owner did not read the brief text. The oshiz-qa object-store question stays open and needs no code change either way." (relayed by planner planner-oco via chat) (code tree 015da5e386c11c3c068fee90e2981b6b5dd47729) (brief sha256:d2b53fcb115f343a476a05ea0663fd7d458415d7337a6c3e4285ad284f4ce928)
