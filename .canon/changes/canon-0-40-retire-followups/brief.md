---
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "0.39 리뷰가 남긴 서로 다른 네 노트를 한 변경에서 닫고 노트마다 현재 동작과 닫힘 조건과 실패 경로를 적어야 해서 줄이면 조건이 빠진다"
  - rule: "prose.length"
    section: "intent"
    reason: "노트마다 재현 조건과 소스 위치의 측정값을 함께 적어야 현재 동작이 빠지지 않는다"
  - rule: "prose.length"
    section: "outcome"
    reason: "결과 네 줄이 각각 한 노트의 닫힘 조건이다"
  - rule: "prose.length"
    section: "flow"
    reason: "네 흐름이 노트마다 하나씩이라 줄이면 흐름이 빠진다"
  - rule: "prose.length"
    section: "detail"
    reason: "단계마다 호출자 조사 결과와 거절한 설계를 담아야 하는 변경이다"
  - rule: "prose.length"
    section: "acceptance"
    reason: "네 단계마다 실패 경로를 요구하는 규칙 때문에 줄일 수 없다"
  - rule: "prose.length"
    section: "questions"
    reason: "결정과 거절한 설계와 소유자 질문이 서로 다른 선택이라 함께 적어야 한다"
  - rule: "prose.sentence-length"
    section: "detail"
    reason: "함수와 호출 순서를 한 문장에 적어야 앞뒤 관계가 보인다"
  - rule: "prose.sentence-length"
    section: "acceptance"
    reason: "시험이 확인하는 순서와 횟수를 한 문장에 적어야 한다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 소스 위치와 리뷰 노트 번호의 인용이다"
  - rule: "prose.parenthetical"
    section: "detail"
    reason: "괄호는 소스 위치와 타입 이름의 인용이다"
  - rule: "prose.parenthetical"
    section: "questions"
    reason: "괄호는 소스 위치와 모듈 이름의 인용이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "함수·파일·키 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "outcome"
    reason: "함수·파일·키 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "flow"
    reason: "함수·파일·키 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "detail"
    reason: "함수·파일·시험 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "함수·파일·시험 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "questions"
    reason: "함수·파일·키 이름은 호출자가 읽는 철자 자체다"
---

# canon 0.39가 남긴 정리 항목 넷: 마지막 시나리오의 묘비, purge 미리보기와 확정의 일치, 퇴역 키 거절 문구, 릴리스의 추적 할당

Source: 0.39.0 변경 셋의 리뷰어 노트(`canon finding list --change canon-0-39-session-ingest-removal` 1_6, `canon-0-39-session-kinds-removal` 1_5·1_6, `canon-0-39-write-durability` 1_2)와 소유자의 2026-11-09 계획 선택, main 693c6ff의 소스를 다시 읽은 값

## Intent

0.39.0 리뷰는 열린 채로 남은 노트 넷을 남겼다. 넷 다 동작이 틀리거나 브리프의 말과 다르고, 어느 것도 데이터를 잃지 않는다. 시험만 조이는 노트(session-ingest-removal 1_5, session-kinds-removal 1_3·1_4, write-durability 1_1·1_3)는 이 변경에 넣지 않는다.

첫째, 시나리오를 모두 지운 저장소다. `staleness.rs`의 `fold_latest_green_cells`는 "선언되지 않은 시나리오"의 증거를 판정에서 뺀다(`crates/canon-gate/src/staleness.rs:180`). 그 판정 `of_an_undeclared_scenario`는 `!ctx.scenarios.is_empty()`일 때만 참이다(`staleness.rs:134`). `ctx.scenarios`는 묘비가 붙은 시나리오를 모든 세대째 걸러낸 뒤의 목록이다(`crates/canon-gate/src/context.rs:255`, 걸러내는 곳은 `context.rs:331`). 그래서 마지막 `.feature` 파일을 지우고 `inventory sync`로 묘비가 쓰인 저장소는 목록이 비어 "시나리오 없음"과 구별되지 않는다. 리뷰어가 실제 바이너리로 확인했다: 시나리오 하나, 증거, 커밋 넷(`max_commits_behind` 2), 파일 삭제 뒤에도 `gate check`가 `stale-evidence`를 낸다. 가드가 비어 있는 코퍼스를 읽지 않는 이유는 시나리오 종류를 git 계층 밖으로 라우팅했거나 색인을 아직 만들지 않았을 때 모든 증거를 "없는 시나리오"로 판정하지 않기 위해서다(`staleness.rs:130`). 묘비는 그 두 경우와 달리 읽은 사실이다. 묘비는 gitignore된 시나리오 색인에만 쓰이므로 새로 복제한 저장소(얕은 복제 포함)는 묘비를 갖지 않고 0.39와 똑같이 판정한다. 이 변경이 닫는 것은 색인이 있는 작업 트리다.

둘째, `purge`의 미리보기와 확정이 다르다. `TierRegistry::purge_rungs`는 붙지 못하는 계층을 미리보기에서는 `unavailable`에 담고 넘어가고(`crates/canon-store/src/registry.rs:294`), 확정에서는 오류로 되돌린다(`registry.rs:295`). `purge --kind session|event`는 `canon.yaml`의 `tiers:`에 있는 모든 계층을 찾는다(`registry.rs:275-277`). 선언은 했지만 붙지 못하는 계층(예: `bucket_env`가 비어 있는 cold s3)이 있으면 미리보기는 exit 0으로 지울 목록과 `meta.next`의 `--commit` 명령을 내고, 그 명령은 `RETENTION_TIERS_REQUIRED`로 거절된다(`crates/canon-cli/src/retention.rs:77`, 거절이 일어나는 곳은 `retention.rs:375`의 확정 쪽 `purge`). 이 경로는 퇴역 종류만의 것이 아니다. `purge_rungs`는 현재 종류도 같이 지난다(`registry.rs:263-268`). 지워지지 않는 데이터가 읽히는 곳은 없고 오류가 필요한 값을 말한다. 틀린 점은 낼 수 없는 명령을 미리보기가 내는 것이다.

셋째, 퇴역 키 거절 문구다. `canon migrate`가 앵커·태그가 붙은 키나 흐름 매핑 안의 키를 줄 단위로 지우지 못하면 `CONFIG_INVALID`로 거절한다. 문구는 종류를 일반적으로만 말한다: ``canon.yaml names a retired record kind (`session`, `event`) in <why>; delete the `session` and `event` keys under `routing:` and `aging:` by hand`` (`crates/canon-cli/src/migrate_names.rs:647`). 시나리오 `tier.retired.05`는 거절이 "키를 말한다"고 적지만 묶인 시험은 `by hand`만 본다(`crates/canon-cli/tests/session_kinds_removal.rs:327`). 같은 파일의 `retired_key_lines`는 이미 `routing.session` 같은 이름과 줄 번호를 안다(`migrate_names.rs:631`). `RETIRED_KIND_KEYS` 경고는 이미 찾은 키를 하나씩 말한다(`crates/canon-cli/src/retired_kind_keys.rs:24`). 거절만 일반 문구다.

넷째, 내구성 추적 장치의 릴리스 할당이다. 0.39 브리프는 쓰기 순서를 시험으로 보려고 디버그 빌드에만 있는 추적 장치를 두고 릴리스에는 없다고 적었다. `trace::record`는 릴리스에서 아무것도 하지 않지만 인자가 호출 앞에서 만들어진다. `write_atomic`은 unix에서 호출마다 경로를 네 번 복사한다(`crates/canon-store/src/atomic.rs:62`의 `tmp_path.clone()`, `atomic.rs:64`의 `tmp_path.clone()`과 `path.to_path_buf()`, `sync_dir`의 `atomic.rs:85` `dir.to_path_buf()`). 증거 블롭을 놓는 `publish_paths`는 블롭마다 한 번 복사한다(`crates/canon-gate/src/artifact_store.rs:471`의 `target.clone()`). `trace` 모듈과 `Event`는 릴리스에서도 공개 타입이다(`atomic.rs:93-105`). fsync 하나에 비해 작은 값이지만 브리프의 말과 다르다.

## Desired outcome

- ==**시나리오를 모두 지운 저장소에서도 묘비가 붙은 시나리오의 옛 증거는 `stale-evidence`도 `STALENESS_UNRESOLVABLE`의 셀도 아니다.**== 묘비를 읽은 사실로 판정하고, 시나리오 목록이 비었다는 사실로 짐작하지 않는다.
- 시나리오 코퍼스를 읽지 못한 저장소(시나리오 종류를 git 계층 밖으로 라우팅했거나 색인을 만들지 않은 경우)는 0.39와 똑같이 판정한다.
- `canon purge`의 미리보기는 확정이 거절할 계획에 대해 그 확정 명령을 내지 않는다. 붙지 못하는 계층이 있으면 그 계층과 풀어야 할 값을 먼저 말하고, 확정은 지금처럼 `RETENTION_TIERS_REQUIRED`로 거절하며 아무것도 지우지 않는다.
- `migrate`의 퇴역 키 거절은 지우지 못한 키를 정확한 이름(`routing.session`)으로 말하고, 줄을 알면 줄 번호도 말한다.
- 릴리스 빌드에서 `write_atomic`과 `publish_paths`는 추적 때문에 할당하지 않고, `trace` 모듈과 `Event` 타입은 디버그 빌드에만 있다.

## User flow

1. F1 오케스트레이터는 마지막 `.feature` 파일을 지우고 `canon inventory sync`를 돌린 저장소에서 `canon gate check`와 `canon status`를 돌린다 → 지운 시나리오의 옛 증거는 판정에 들지 않는다 → `stale-evidence`가 없고 `STALENESS_UNRESOLVABLE` 셀 수에도 들지 않는다.
2. F2 오케스트레이터는 붙지 못하는 계층이 선언된 저장소에서 `canon purge --kind session|event`를 미리본다 → 지울 목록과 함께 붙지 못한 계층과 풀어야 할 값을 읽는다 → ==**돌려도 거절될 `--commit` 명령은 받지 않는다.**==
3. F3 오케스트레이터는 앵커나 흐름 매핑 형태로 퇴역 키가 든 `canon.yaml`에서 `canon migrate`를 돌린다 → 거절 문구가 지우지 못한 키의 이름과 줄을 말한다 → 그 키만 손으로 고치고 다시 돌린다.
4. F4 개발자는 릴리스 빌드(`cargo build --release`)와 디버그 빌드에서 canon-store와 canon-gate를 빌드한다 → 릴리스에는 추적 장치가 없다 → 디버그의 쓰기 순서 시험은 그대로 돈다.

## Step detail

### F1 묘비를 읽어서 판정하기

- `GateContext`는 묘비가 붙은 시나리오의 키 집합을 따로 든다. `context.rs:255`가 `without_removed`로 걸러내기 전에 읽은 목록에서, 최신 세대가 묘비(`Scenario::removed`)인 `(project_id, scenario_id)`를 모은다. 걸러낸 목록은 지금처럼 `ctx.scenarios`에 남고 아무 읽는 곳도 달라지지 않는다.
- `of_an_undeclared_scenario`는 다음일 때 참이다: 셀이 시나리오의 것이고 `project_id`와 `scenario_id`가 있으며, `latest_scenario`가 없고, 그 키가 묘비 집합에 들어 있거나 `ctx.scenarios`가 비어 있지 않다. 뒤의 조건은 0.39의 "선언한 적 없는 시나리오는 없는 것"을 그대로 둔다. 비어 있는 코퍼스에 묘비도 없으면 판정은 0.39와 같다.
- 다시 선언한 시나리오는 최신 세대가 살아 있어 묘비 집합에 없고 다시 판정된다(`context.rs:328`의 "re-added" 규칙이 그대로다).
- `GateContext`의 구조체 리터럴이 canon-gate 시험 여덟 곳에 있다(`artifact_store.rs:958`, `audit.rs:246`, `coverage.rs:234`, `evidence_binding.rs:300`, `ledger.rs:205`, `risk.rs:435`, `spec_coverage.rs:570`, `staleness.rs:676`). 필드가 하나 늘면 이 리터럴을 모두 고친다. `invalidate_derived`가 지우는 파생 값에는 묘비 집합이 들지 않는다. 코퍼스 벡터를 바꿔 다시 쓰는 호출자는 이 집합도 함께 바꿔야 한다는 말을 필드 문서에 쓴다.
- `staleness.rs`의 모듈 문서(67-71)와 `of_an_undeclared_scenario`의 문서를 새 규칙으로 고친다. 0.39 `ingest_removal_16`은 그대로 통과한다(시나리오가 하나 남는 경우).
- CHANGELOG의 Fixed에 한 줄을 쓴다. 증거 쪽 코드는 바뀌지 않으므로 증거를 다시 쓸 일은 없다.

### F2 미리보기와 확정의 일치

- ==**붙지 못하는 계층이 있을 때 확정은 그대로 거절한다.**== `purge_rungs`의 사전 점검(`registry.rs:299-303`)은 모든 계층을 읽은 뒤에야 첫 삭제를 하는 설계다. 일부 계층만 지우고 다른 계층의 행을 남기는 확정은 이 설계와 어긋나고, 남은 행이 어디 있는지 사람이 알 수 없게 된다.
- 미리보기(`retention.rs:331`의 `run_purge`)는 `preflight.unavailable`이 비어 있지 않으면 `plan.finish_preview` 대신 다른 끝맺음을 쓴다. `data`는 지금과 같다(`unavailable`, 효과의 `skip` 줄, 지울 목록, `planDigest`). 바뀌는 것은 `meta.next`와 사람이 읽는 마지막 줄뿐이다. 첫 항목은 붙지 못한 계층의 이름과 풀어야 할 값을 말하는 단계이고 `--commit` 명령은 내지 않는다. 사람 출력의 마지막 줄도 "To act: <확정 명령>" 대신 같은 안내다.
- 설정되지 않은 계층의 오류 문구(`handle(rung)`, `crates/canon-store/src/tier.rs:375-378`)는 cold s3에서 `no live bucket`만 말하고 변수 이름이 없다. `handle`이 그 계층에 설정된 변수 이름(`bucket_env`, `dsn_env`가 가리키는 값, 예 `CANON_R2_BUCKET`)을 문구에 덧붙이고, 미리보기는 그 문구를 `why`에 싣는다. 설정은 됐지만 답하지 않는 계층(`RETENTION_TIERS_UNAVAILABLE`)은 변수를 풀라고 하지 않고 다시 시도할 수 있는 도달 확인을 말한다.
- 미리보기의 종료 값은 0이다. 지울 목록을 읽는 용도는 계층 하나가 붙지 않아도 유효하고, 읽기 전용이다. 새 코드나 경고를 더하지 않는다(생성된 코드 표와 `atip` 고정 파일을 건드리지 않는다).
- 확정 쪽 오류 문구는 계층 이름을 이미 담는다(`tier.rs:375-378`). 여기서 하는 일은 확정 오류에 미리보기와 같은 안내가 가도록 `hint`를 그대로 두는 것이다. 바꾸지 않는다.
- 이 흐름은 현재 종류(`trajectory`, `run`)의 `purge`에도 같다. `purge_rungs`가 한 곳이라 따로 나누지 않는다.
- 안내문(스킬의 purge 단락)과 CHANGELOG의 Changed에 "붙지 못하는 계층이 선언된 저장소의 `purge`는 그 계층을 먼저 풀어야 확정되고, 미리보기가 그렇게 말한다"를 쓴다. 0.39 리뷰가 CHANGELOG의 purge 항목에 한 줄을 권했다(session-kinds-removal 1_5).

### F3 거절 문구가 키를 말하기

- `remove_retired_keys`(`migrate_names.rs:641`)의 `hand` 문구에 지우지 못한 키 이름들(정렬한 `routing.session, aging.event` 형태, `retired_key_lines`의 `RetiredKeyLines::key`)과, 줄 번호를 아는 키는 `줄 N`을 붙인다. 줄 단위 탐색이 흐름 매핑 안의 키를 찾지 못해 줄이 없으면 이름만 말하고 `routing:` 또는 `aging:` 섹션 이름을 말한다. 이름은 이미 파싱한 YAML(`before`)에서 `routing`과 `aging` 아래 퇴역 이름을 가진 키로 정한다. 줄 번호를 찾기 위해 새 YAML 파서를 쓰지 않는다.
- 앵커·별칭·태그 거절(`migrate_names.rs:653`)은 그 형태를 가진 키만 말한다. 키 하나를 지우지 못하는 이유가 다른 키에 있을 때 모든 퇴역 키를 나열하지 않는다.
- 문구의 "by hand"와 `session`, `event`는 그대로 둔다. 있는 시험 둘(`migrate_names.rs:1573`, `session_kinds_removal.rs:327`)이 그것을 본다. 종료 값, 코드 `CONFIG_INVALID`, 이유 `CANON_YAML`, 거절이 파일을 그대로 두는 것은 바뀌지 않는다.

### F4 추적 장치를 디버그에만 두기

- `trace` 모듈, `Event` 열거형, 세 호출(`atomic.rs:62·64·85`)과 `artifact_store.rs:471`의 호출을 `#[cfg(debug_assertions)]`로 감싼다. 이벤트를 만드는 식이 그 안에 있어 릴리스는 `PathBuf`를 복사하지 않는다. `record`의 릴리스 쪽 분기와 `take`의 빈 벡터는 사라진다.
- 추적을 읽는 시험은 `atomic.rs:182`, `atomic.rs:195`, `artifact_store.rs:1011`이고 모두 `#[cfg(debug_assertions)]` 아래로 옮긴다. 릴리스에서 `cargo test --release`는 그 시험을 건너뛴다. 이미 디버그 전용 장치를 쓰는 시험이 릴리스에서 건너뛰어지는 선례가 있다(`crates/canon-cli/tests/atip_reasons_eio.rs:11`).
- `write_atomic`이 `Ok`로 돌아올 때의 보장, 호출 순서, 오류 문구는 바뀌지 않는다. 이 흐름이 바꾸는 것은 릴리스 빌드의 할당과 공개 타입뿐이다.
- `sync_dir`의 `trace::record`도 같다. 호출 하나가 `cfg!(unix)` 분기 안에 있으므로 그 안에서 감싼다.

## Acceptance

- F1:
  - 시나리오 하나와 그 증거를 가진 저장소에서 커밋을 `max_commits_behind` 위로 쌓고 그 `.feature` 파일을 지운 뒤 `canon inventory sync`를 돌려 묘비를 쓰면, `gate check`는 그 시나리오를 `stale-evidence`로 내지 않고 `status`의 `staleness_unresolvable` 셀 수에도 넣지 않는다. 같은 저장소에서 파일을 되살리고 sync하면 다시 `stale-evidence`가 된다.
  - 실패 경로: 시나리오 종류를 git 계층 밖으로 라우팅해 시나리오 코퍼스를 읽지 못한 저장소는 지금처럼 증거를 판정한다. 같은 증거가 `stale-evidence`로 남고, 판정하는 단위 시험에서 비어 있는 코퍼스와 묘비 집합이 비어 있는 컨텍스트는 선언 안 된 시나리오로 보지 않는다.
- F2:
  - cold 계층이 선언됐지만 `bucket_env`가 비어 붙지 못하는 저장소에서 `purge --kind session --before <시각> --json`은 exit 0이고 `data.unavailable`이 그 계층을 말하며 `meta.next`의 첫 항목이 그 계층의 풀어야 할 값을 말하고 어떤 항목도 `--commit` 명령이 아니다. 파일은 그대로다.
  - 실패 경로: 같은 저장소에서 이전 미리보기의 다이제스트로 `--commit`을 돌리면 `RETENTION_TIERS_REQUIRED`(exit 2)이고 계층 이름을 말하며 행이 하나도 지워지지 않는다. 값을 채워 계층이 붙은 저장소의 미리보기는 `--commit` 명령을 내고 그 확정이 지운다.
- F3:
  - `routing: { change: local, session: hot }`, 앵커가 붙은 `session: &shared hot`, 태그가 붙은 `session: !!str hot`에서 `migrate`와 `migrate --commit`은 `CONFIG_INVALID`(exit 2)이고 메시지가 `routing.session`을 말한다. 앵커와 태그 줄은 `줄 N`도 말한다. 파일은 그대로다.
  - 실패 경로: `routing.event`는 깨끗한 줄이고 `routing.session`만 앵커인 파일의 거절은 `routing.session`만 말하고 `routing.event`는 말하지 않는다. 퇴역 키가 없는 `canon.yaml`의 `migrate`는 이 거절을 내지 않는다.
- F4:
  - 디버그 빌드에서 `write_atomic`의 순서 시험(파일 sync, rename, 디렉터리 sync)과 `publish_paths`의 시험(rename 둘 뒤 디렉터리 sync 하나)이 그대로 통과한다.
  - 실패 경로: `cargo build --release -p canon-store -p canon-gate`와 `cargo test --release -p canon-store -p canon-gate --no-run`이 통과한다. 추적을 읽는 시험이 릴리스 빌드에서 `trace` 모듈을 찾지 못해 컴파일에 실패하는 일이 없고, 릴리스 소스에서 `trace::` 호출이 `debug_assertions` 밖에 없다는 점을 소스 검사 시험이 확인한다.
- ==**변경 뒤 `cargo nextest run --workspace`, 생성 문서 drift 시험, `just atip-deep`, `canon gate check`가 통과한다.**==

## Questions and assumptions

- 결정: F1은 묘비 집합을 `GateContext`에 따로 둔다. 거절한 설계: `without_removed`를 끄고 목록이 비었다는 사실로 구별하기. 묘비 세대를 읽는 다른 호출자(`spec_coverage`, `inventory`)가 "없는 시나리오"로 읽도록 걸러낸 목록이 맞는 값이라 그대로 두고 키 집합만 더한다. 또 다른 거절한 설계: 비어 있는 코퍼스의 가드를 없애고 항상 `latest_scenario` 없음을 "선언 안 됨"으로 보기. 시나리오 종류를 git 계층 밖으로 라우팅한 저장소의 모든 증거를 한꺼번에 판정에서 빼게 되어 0.39 `ingest_removal_16`이 막으려던 오판정의 반대 방향 사고가 난다.
- 결정: F1은 선언한 적 없는 시나리오(묘비도 없고 코퍼스는 비어 있지 않은 경우)를 0.39처럼 "없는 것"으로 둔다. 바꾸는 것은 "묘비를 읽었으면 코퍼스가 비어도 없는 것" 하나뿐이다.
- 결정: F2는 확정을 그대로 거절하고 미리보기를 맞춘다. 붙은 계층만 지우는 확정은 사전 점검을 모든 계층에서 하는 설계(`registry.rs:299-303`)를 약하게 하고, 지우지 못한 행이 남은 곳을 사람이 알아야 한다. 데이터는 잃지 않는다. 미리보기는 지울 목록을 읽는 용도로 계속 exit 0이다.
- 결정: F2의 안내는 새 오류 코드나 경고 코드를 더하지 않는다. `canon schema --errors`, 경고 목록, `atip` 고정 파일을 건드리지 않는다. 있는 `unavailable` 필드와 `skip` 효과가 그 사실을 이미 담는다.
- 결정: F3은 줄 번호를 아는 키에만 줄을 말한다. 흐름 매핑 안의 키는 줄 단위 탐색이 찾지 못하므로 이름만 말한다. 새 파서로 줄을 찾는 일은 하지 않는다.
- 결정: F4는 `cfg(debug_assertions)`로 모듈째 뺀다. 거절한 설계: `record`가 클로저를 받아 릴리스에서 부르지 않기. 공개 타입이 릴리스에 남고 호출 모양이 둘로 갈린다. 0.39 브리프의 말("디버그 빌드의 장치")과 같은 것은 모듈 전체를 빼는 쪽이다.
- 결정: 시험만 조이는 노트(session-ingest-removal 1_5, session-kinds-removal 1_3·1_4, write-durability 1_1·1_3)는 이 변경에 넣지 않는다. write-durability 1_1(루트로 돌 때 실패 경로가 건너뛰어진다)은 CI가 비루트 사용자로 돈다는 것을 리뷰가 확인했다.
- 질문: F2에서 확정이 붙은 계층만 지우고 붙지 못한 계층을 보고하는 쪽(퇴역 종류 한정)을 원하는가, 아니면 기본안인 확정 거절 유지와 미리보기 맞춤인가? 퇴역 종류의 행은 아무도 읽지 않으므로 앞쪽도 안전하지만, 그러면 현재 종류와 규칙이 갈린다.
- 질문: F4의 시험 이동으로 릴리스 빌드의 `cargo test --release`가 추적 시험 셋을 건너뛴다. 릴리스 시험 일정이 이 셋의 통과를 필요로 하는가? 기본안은 필요 없다(디버그 시험이 같은 순서를 본다).
- 실패 방식: F1의 묘비 집합을 코퍼스 벡터와 함께 갱신하지 않는 호출자가 생기면 묘비를 놓칠 수 있다. 이 필드는 `GateContext::load`가 한 번 채우고 시험의 리터럴은 빈 집합으로 둔다.

## Out of scope

- 시험만 조이는 리뷰 노트(위 결정의 목록)와 낡은 주석 정리.
- 붙지 못하는 계층에 대한 `purge`의 부분 삭제. 소유자 질문의 답에 따른다.
- 새 오류 코드·경고 코드.
- 증거를 다시 쓰거나 `stale-evidence` 판정의 다른 규칙을 바꾸는 일.
- `write_atomic`의 보장(파일과 디렉터리 sync)과 비용 측정.

Approved: Journey Hwang 2026-11-09T14:24:16Z — "재승인: 리뷰 지적에 따라 고친 브리프 3개" (relayed by planner planner-oco via ask) (code tree b9fe5e9bd8ef5436b3e8990ed06245172b3a003c) (brief sha256:166c0c458494e9a0c232fd0d3d7a74544a1c1856eaa244ba8ec82defc344e4a8)
