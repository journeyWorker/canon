---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "잰 값과 레버마다 바뀌는 잡, 검사, 판정 입력을 열거해야 구현자가 다시 재지 않는다"
  - rule: "prose.length"
    section: "intent"
    reason: "낭비 다섯의 측정표가 근거 수치다"
  - rule: "prose.length"
    section: "invariants"
    reason: "불변마다 바뀌는 잡, 단계, 판정 값을 열거해야 한다"
  - rule: "prose.length"
    section: "checks"
    reason: "확인마다 입력과 판정, 실행에서 읽을 값이 필요하다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "워크플로 파일, 잡 이름, 실행 번호는 이 변경이 바꾸는 대상과 측정의 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "워크플로, 잡, 단계, 출력 값, 판정 코드는 검사기와 시험이 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "확인은 명령, 잡, 판정 코드의 철자를 그대로 말해야 구현자와 리뷰어가 같은 것을 돌린다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 실행 번호와 측정값의 출처다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 값의 목록과 예다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 시험 입력의 목록이다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "불변과 확인마다 잡, 단계, 판정 값, 입력을 열거해야 해서 나누면 대응이 끊긴다"
  - rule: "prose.sentence-length"
    section: "checks"
    reason: "불변과 확인마다 잡, 단계, 판정 값, 입력을 열거해야 해서 나누면 대응이 끊긴다"
  - rule: "prose.paragraph-length"
    section: "invariants"
    reason: "불변과 확인마다 잡, 단계, 판정 값, 입력을 열거해야 해서 나누면 대응이 끊긴다"
  - rule: "prose.paragraph-length"
    section: "checks"
    reason: "불변과 확인마다 잡, 단계, 판정 값, 입력을 열거해야 해서 나누면 대응이 끊긴다"
  - rule: "prose.key-sentence"
    section: "invariants"
    reason: "레버마다 핵심 문장이 하나씩이고 여섯 레버를 한 절에서 다룬다"
  - rule: "prose.key-sentence"
    section: "checks"
    reason: "레버마다 핵심 문장이 하나씩이고 여섯 레버를 한 절에서 다룬다"
---

# CI 최적화: 이미 시험한 트리를 main이 다시 시험하지 않고, 일반 PR은 한 행만 빌드한다

Source: .github/workflows/test.yml, .github/workflows/build-native.yml, .github/workflows/pr-guard.yml, scripts/build-scope.py, scripts/test-build-scope.py, scripts/ci-strip-workspace-build.sh, scripts/check-release-workflow-safety.py, scripts/test-release-workflow-safety.py, docker-compose.yml, docker/minio/Dockerfile, 측정 /Volumes/Shelter/storage/journey-temp/projects/canon/2026-11-18-0.44-ci/ci-measure.md(`gh run view`, `gh api actions/runs/<id>/jobs`, `gh api actions/caches`), 실행 44159678352 · 44308449847 · 44222995529 · 43523582441 · 42561612613 · 42281215709

## Intent

한 번의 PR 푸시는 지금 `test.yml`에서 Linux 49분, `build-native.yml`에서 Linux 47분과 가중 191분(macOS가 76%), `pr-guard.yml`에서 2.5분을 쓴다. 병합하면 main 푸시가 `test.yml`을 53분 더 돈다. 아래는 측정에서 나온 낭비다(표본 한 번, 러너 잡음 ±40%: 같은 트리의 `cargo test --workspace`가 PR에서 1013초, main에서 654초).

| 낭비 | 측정 |
| --- | --- |
| main 푸시가 PR이 이미 시험한 트리를 다시 시험한다 | 확인한 7쌍 모두 main 푸시 커밋의 트리가 PR의 병합 ref 트리와 같다(main `dd12bba`는 PR `9871fad`와 트리 `11775118`이 같다). 14일 동안 main 푸시 10번 53분 = 530분, `test.yml` 분량의 35%. main 푸시의 고유한 산출물은 캐시 시딩과 사후 판정뿐인데, 표본의 main 푸시 10개 중 3개가 붉었고(43523582441, 42561612613: `cargo test`, 42281215709: three-callers) 앞의 것은 같은 트리가 PR에서 초록이었다(깜빡임) |
| 일반 PR이 릴리스에만 필요한 빌드를 돈다 | macOS arm64 행 730~1224초(캐시 적중 452), Windows 599~768, Linux arm64 686~751, man 204~217, 대시보드 16. 표본 24개 중 11개가 `release/` 아닌 PR. 이 바이너리는 PR 실행에서 게시되지 않는다 |
| 빌드 캐시가 ref마다 복사본을 쌓는다 | 캐시 27개 11.12GB(한도 10GB). 빌드 계열(`canon-cli-<target>-cargo` 16개 7.55GB, `canon-cli-man-cargo` 4개 1.34GB)이 8.9GB로 80%. 같은 키가 PR ref마다 따로 저장되고 정리되지 않는다(`ci-strip-workspace-build.sh`는 `target/debug`·`target/release`만 안다. 테스트 캐시는 정리해서 200~360MB, 빌드 캐시는 460~500MB) |
| `live-tier`가 서비스 이미지 준비를 cargo 빌드 뒤에 줄 세운다 | `docker compose up` 122~143초와 `minio-init` 45~51초(두 서비스는 Dockerfile의 Go 빌드다: `docker/minio/Dockerfile`), 5~7분 잡의 3분. cargo 빌드(51~178초)는 서비스가 필요 없다 |
| 게이트가 시험 뒤에 돈다 | `canon gate check` 14~24초가 `cargo test --workspace` 650~1000초 뒤에 있다. 0.44 PR 44308449847이 시험 828초를 다 돌고 게이트에서 붉어졌다. 표본에서 PR 실행 20개 중 4개가 붉다 |

게이트는 시험의 출력을 읽지 않는다. 시험이 남기는 기록(`CANON_WITNESS_DIR`)은 디버그 바이너리가 쓰고(`crates/canon-cli/src/json_out.rs:686`) 증인 시험만 읽는다(`crates/canon-cli/tests/grammar_t9_witness.rs:117`). 게이트 단계는 그 변수를 두지 않는다(`test.yml` 210행). 게이트가 읽는 것은 저장소의 `.canon` 기록과 이력이다.

이 변경 뒤: main 푸시는 PR이 초록으로 시험한 트리면 시험을 돌리지 않고 캐시만 채운다. 일반 PR의 `build-native.yml`은 musl x64 한 행만 빌드한다. 빌드 캐시는 릴리스 실행만 저장하고, 줄여서 저장한다. man 생성은 릴리스에서만 돈다. `live-tier`는 이미지 준비를 cargo 빌드와 함께 한다. `offline`은 게이트를 시험 앞에 둔다.

## Category

ci

## Invariants

- I1 ==**canon의 바이너리, 명령, 출력, 레저는 바뀌지 않는다.**== 바뀌는 것은 `test.yml`, `build-native.yml`, `pr-guard.yml`의 머리 주석, `scripts/build-scope.py`와 그 시험, `scripts/ci-strip-workspace-build.sh`, 새 스크립트 `scripts/ci-tested.py`와 그 시험 `scripts/test-ci-tested.py`, 릴리스 검사기와 그 시험, `justfile`의 `release-check`, CHANGELOG다. 소스 코드와 `publish.yml`은 그대로다. 이 변경의 결정 여섯은 오너가 골랐다(질문 아래 "결정(owner)").
- I2 ==**이미 시험한 트리 (레버 1).**== `test.yml`의 첫 잡 `tested`(권한 `actions: read`와 `contents: read`)가 모든 실행에서 돌고 `tested` 출력을 낸다. 값은 `true`이려면 이벤트가 main으로의 푸시이고 이름이 `tested-<이 커밋의 트리>`인 아티팩트가 있어야 하며, 그 아티팩트가 만료되지 않았고 올린 실행이 이 저장소의 `.github/workflows/test.yml`의 `pull_request` 실행이고 `conclusion`이 `success`다. 하나라도 어긋나거나 API를 읽지 못하면 `false`이고 잡은 통과한다(판단 불가는 시험을 돌리는 쪽이다). `scripts/ci-tested.py`가 한 줄 JSON `verdict`(`tested` 또는 사유 `no_record`, `expired`, `run_not_green`, `foreign_repo`, `wrong_workflow`, `wrong_event`, `unreadable`)를 stdout에 쓰고 종료 코드는 항상 0이다. 트리는 `git rev-parse HEAD^{tree}`이고 PR의 트리는 그 실행이 체크아웃한 병합 ref의 것이다.
- I3 PR 실행의 마지막 잡 `record`가 `offline`, `clippy`, `atip`, `bench-shape`, `launcher`, `platformer`, `live-tier`를 모두 기다리고, 모두 성공했을 때만 `tested-<트리>`를 올린다(보존 14일, 내용은 트리와 실행 번호). 일부 잡을 다시 돌려 초록이 되면 같은 이름을 덮어쓴다(`overwrite: true`). 이 잡 말고 이 이름을 올리는 곳이 없다.
- I4 ==**시딩 모드.**== `tested` 출력이 정확히 `true`이고 이벤트가 main으로의 푸시일 때만 시딩 모드다. 그 밖의 값(`false`, 빈 값, 출력 없음, 잡 실패)은 모든 단계를 돌린다. 이 모드에서 `launcher`와 `platformer`는 돌지 않는다. 다섯 캐시 잡(`offline`, `clippy`, `atip`, `bench-shape`, `live-tier`)은 복원하고, 정확한 키가 맞았으면 빌드도 하지 않고 끝난다. 맞지 않았으면 지금처럼 의존 빌드만 하고 정리해 저장한다. 이 모드에서는 `cargo test`, 세 호출자, 증인, 게이트, atip 검사, 측정, docker, 라이브 시험, duckdb 설치가 하나도 돌지 않는다. PR 실행, 아직 시험한 적 없는 트리, 직접 푸시, `workflow_dispatch`는 모든 단계가 지금처럼 돈다. 저장 조건(main, 정확한 키 불일치, 빌드 성공)은 그대로다.
- I5 ==**일반 PR은 한 행만 빌드한다 (레버 2).**== `scripts/build-scope.py`가 `skip` 말고 `mode`를 출력한다. 값: `release`(이벤트가 `pull_request`가 아님, 머리 브랜치가 `release/`로 시작, 변경 목록을 구하지 못함), `native`(그 밖에서 변경이 `Cargo.lock`, 이름이 `build.rs`인 파일, `.cargo/` 아래 중 하나를 건드림), `quick`(그 밖에 `BUILD_PATHS`를 건드림), 건드리지 않으면 지금처럼 `skip=true`. 모르는 값, 빈 값, 출력 없음은 `release`처럼 읽는다(모자랄 때는 더 많이 빌드한다).
- I6 `build` 잡의 행은 `mode`가 `quick`이면 `x86_64-unknown-linux-musl` 하나이고 `native`와 `release`면 네 행이다. 행 표는 하나로 남고 검사기가 읽는다(`scripts/check-release-workflow-safety.py`는 행의 필드와 러너 라벨을 지금처럼 검사한다). 모든 행이 대시보드를 내장한다(`dashboard` 잡은 `skip`이 아닌 모든 실행에서 돌고 `build`는 지금처럼 그것을 기다린다).
- I7 ==**릴리스에서만 도는 잡 (레버 2·5).**== `man`과 `manifest`는 `mode`가 `quick`도 `native`도 아닐 때만 돈다. `native`는 네 행을 돌리되 `man`이 없어 `manifest`를 만들지 않는다. 릴리스 PR(`release/*`)과 수동 실행은 네 행, 대시보드, man, `manifest`를 모두 돌린다(`release-build-once`의 `manifest`가 네 행과 man 전부를 기다리는 계약 그대로). 일반 PR 실행은 `build-manifest-<트리>`를 올리지 않는다. `scripts/release-build.py`는 바뀌지 않는다. 행이 모자란 매니페스트는 `record`가 `incomplete`로 거절하고 `publish.yml`의 `find`는 매니페스트 없는 실행에서 통과하지 못한다.
- I8 `pr-guard.yml`은 바뀌지 않는다(머리 주석만). man 생성기의 `cargo check`와 `safety`는 모든 PR에서 돈다. `build-native.yml`의 `safety`는 그대로 둔다(`publish.yml`이 의존하는 검사의 두 번째 자리).
- I9 ==**빌드 캐시는 릴리스만 저장하고 줄여 저장한다 (레버 3).**== `build-native.yml`의 `build`와 `man`이 캐시를 모든 실행에서 복원한다(`actions/cache/restore`). 저장(`actions/cache/save`)은 성공했고 정확한 키가 맞지 않았고 머리 브랜치가 `release/`로 시작하거나 이벤트가 `workflow_dispatch`일 때만 한다. 저장 앞에 정리 단계가 온다. 두 단계는 아티팩트 업로드 뒤에 있고 둘 다 `continue-on-error`라서 캐시 위생이 릴리스를 막지 못한다. 저장 단계의 `if:`는 정리 단계의 `outcome == 'success'`를 요구하고, 정리가 성공하지 않으면 저장하지 않는다(줄이지 않은 캐시를 저장하지 않는다).
- I10 `scripts/ci-strip-workspace-build.sh`가 타깃 인자(`--target <triple>`)를 받는다. 지우는 것은 지금과 같다(워크스페이스 크레이트의 빌드 출력, `cargo clean -p`). 호스트용 빌드(빌드 스크립트, proc-macro, `target/release`)와 타깃용 빌드(`target/<triple>/release`)에서 워크스페이스 크레이트 이름을 단 산출물이 남지 않는다. aarch64 musl 행은 호스트에 cargo가 없으므로 같은 이미지(`alpine_image`) 안에서 돌고 파일 소유를 호스트에 돌려준다. 이미지에는 `bash`와 `jq`가 없으므로 정리 앞에 `apk add --no-cache bash jq`를 한다. 저장한 크기는 지금처럼 작업 요약에 나온다.
- I11 ==**`live-tier`의 이미지 준비 (레버 6).**== 이미지 준비(`postgres`는 받기, `minio`와 `minio-init`은 Dockerfile 빌드)가 cargo 시험 빌드와 한 단계 안에서 함께 돌고 단계 끝에서 둘 다 기다린다. 둘 중 하나라도 실패하면 단계가 실패한다(백그라운드의 실패가 묻히지 않는다). 이후 `compose up`은 준비된 이미지를 쓴다. 시험 단계, 환경 변수, `CANON_REQUIRE_LIVE=1`은 그대로다.
- I12 ==**게이트가 시험 앞에 온다 (레버 7).**== `offline`에서 `canon inventory sync`와 `canon gate check`(과 앞의 `cargo build -p canon-cli`)가 `--no-run` 빌드 뒤, `cargo test --workspace` 앞에 온다. 세 호출자, 증인 단계는 시험 뒤에 그대로 있고 `CANON_WITNESS_DIR`은 시험과 증인 단계에만 걸린다. 게이트가 붉으면 시험은 돌지 않고 잡이 붉다.
- I13 새로 쓰거나 고치는 시험은 판정만 단언한다. `ci-tested.py`는 입력 API 응답에 따른 `verdict`와 종료 코드, `build-scope.py`는 입력(이벤트, 머리 브랜치, 변경 파일)에 따른 `mode`와 `skip`, 검사기는 입력 워크플로를 통과시키는가 거절하는가를 본다. 오류 문구, 단계 이름, 워크플로 소스의 글자는 단언하지 않는다.
- I14 ==**검사기가 모양을 지킨다.**== `scripts/check-release-workflow-safety.py`에 규칙이 더해진다: `man`과 `manifest`의 `if:`가 `mode` 값 `release`와 모르는 값에서 돌고 `quick`과 `native`에서 돌지 않는다. `dashboard`는 `mode`와 무관하게 `skip`이 아니면 돈다. `build-native.yml`의 캐시 저장 단계의 `if:`가 일반 PR 머리, `release/0.44.0` 머리, `workflow_dispatch`에서 각각 저장 안 함, 저장, 저장이고 `restore`가 모든 실행에서 있다. 저장 단계가 정리 단계 뒤에 오고 `if:`가 정리 단계의 `outcome == 'success'`를 요구한다. `build`의 행은 `mode`가 `quick`이면 musl x64 하나이고 `native`, `release`, 모르는 값, 빈 값이면 네 행이다. `tested` 잡은 `actions: read`를 가진다. 지금 규칙(`scope` 규칙, 러너, 행 필드)은 그대로다.
- I15 문서. `test.yml` 머리 주석의 캐시 문단에 `tested`, `record`, 시딩 모드를 적고, `build-native.yml` 머리 주석에 모드 표(`quick`, `native`, `release`)와 캐시 저장 규칙을 적는다. `pr-guard.yml` 머리의 "release-mode man-page build stays in build-native.yml behind its path filter"를 고친다. CHANGELOG `Unreleased`에 한 항목. `canon/skills-dev/repo-scaffold/SKILL.md`나 설치본이 "모든 PR이 네 행을 빌드한다"고 적었으면 고친다.

## Checks

- ==**`just release-check`가 통과한다.**== 목록에 `scripts/test-ci-tested.py`가 든다. 새 규칙마다 통과 시험과 거절 시험이 있고 모두 판정만 단언한다(I13).
- `test-ci-tested.py`(API 응답을 입력으로 준다, 실제 호출 없음): 유효한 마커 하나는 `tested`. 마커가 없으면 `no_record`. 만료된 마커만 있으면 `expired`. 실행의 `conclusion`이 `failure`이거나 진행 중이면 `run_not_green`. `head_repository`가 포크이면 `foreign_repo`. 경로가 `test.yml`이 아니면 `wrong_workflow`. 이벤트가 `push`이면 `wrong_event`. API 응답이 깨지거나 호출이 실패하면 `unreadable`이고 종료 코드는 0이다. 같은 이름의 마커가 둘이고 하나만 유효하면 유효한 쪽을 고른다. 이 입력들에서 `tested`는 첫 입력에서만 나온다.
- `test-build-scope.py`: 일반 PR에서 `crates/`만 바꾸면 `quick`. `Cargo.lock`을 바꾸면 `native`. `crates/canon-cli/build.rs`나 `crates/x/y/build.rs`를 바꾸면 `native`(`foo/build.rs.bak`은 아니다). `.cargo/config.toml`을 바꾸면 `native`. `release/0.44.0` 머리는 파일과 무관하게 `release`. `workflow_dispatch`는 `release`. 변경 목록을 구하지 못하면 `release`. 문서만 바꾸면 `skip=true`.
- 검사기 거절 시험의 입력: `man`의 `if:`에서 `mode` 조건이 빠진 모양, `manifest`가 `quick`에서 도는 모양, `dashboard`에 `mode` 조건이 붙은 모양, 캐시 저장이 `if:` 없이 모든 PR에서 도는 모양, 저장이 `workflow_dispatch`를 빼먹은 모양, 저장이 정리 앞에 있는 모양, 저장의 `if:`에 정리의 `outcome` 조건이 없는 모양, `quick`에서 네 행이 도는 모양, `release`나 모르는 값에서 행이 모자란 모양, `tested` 잡에 `actions: read`가 없는 모양, 복원이 `pull_request`에서 빠진 모양. 실제 `build-native.yml`은 통과한다.
- ==**실제 PR 실행 하나 (레버 2·6·7).**== 이 변경의 PR 실행(브랜치 `wip-0.44/ci-optimize`, `release/`가 아니고 기본 설정에서 `native` 대상 파일이 아니다)이 `gh api repos/journeyWorker/canon/actions/runs/<id>/jobs`에서 이렇게 보인다. `build-native.yml`: `build`가 `x86_64-unknown-linux-musl` 한 행이고 `dashboard`, `man`, `manifest`는 `skipped`. `test.yml` `offline`: `canon gate check` 단계의 완료 시각이 `cargo test --workspace` 단계의 시작 시각보다 앞선다. `live-tier`: `docker compose up`과 `minio-init` 단계가 합쳐 60초 안이다(지금 167~194초). 비교 값으로 0.43 PR 44159678352와 0.44 PR 44308449847의 같은 단계 시간을 증거에 적는다.
- ==**수동 빌드 두 번 (레버 3·2).**== 같은 브랜치에서 `gh workflow run build-native.yml --ref wip-0.44/ci-optimize`를 한 번, 끝난 뒤 또 한 번. 첫 실행은 네 행, 대시보드, man, `manifest`가 돌고 각 행의 로그에 저장이 있다. 저장된 캐시 크기(`gh api repos/journeyWorker/canon/actions/caches`)가 0.43의 같은 타깃 캐시(aarch64-apple 2004MB, x64 musl 1857, Windows 1848, arm musl 1846, man 335)보다 작다. 둘째 실행은 복원이 정확히 맞고 저장 단계가 돌지 않는다. 이 실행이 `native`·`release` 쪽 길과 줄여 저장하는 길(macOS, Windows, 컨테이너 안의 정리 포함)의 증거다.
- ==**실제 main 푸시 하나 (레버 1).**== 이 변경이 병합된 뒤 다음 병합의 main 푸시 `test.yml` 실행: `tested`가 `true`이고 `cargo test`, 게이트, atip 검사, docker가 `skipped`이며 다섯 캐시 잡이 복원한 키를 적고 정확히 맞았으면 빌드도 없다. 실행 시간(약 25~40초 × 5잡 분량)을 0.43 main 44222995529(892초, 잡 합 53분)와 나란히 적는다. Cargo.lock이 바뀐 푸시(0.44.0 릴리스 병합)에서는 다섯 캐시가 빌드되고 저장된다.
- 병합 뒤 비교: 일반 PR 실행의 `build-native.yml` 가중 분이 191에서 한 자리 수 ~ 십여 분으로 내려가고(Linux 한 행과 `scope`, `safety`), `gh api repos/journeyWorker/canon/actions/cache/usage`가 10GB 아래다. 0.43 ref(`release/0.43.0`, `refs/tags/v0.43.0`, `pull/152`, `pull/153`, `scratch/0.44-no-run`)의 빌드 캐시는 오너의 결정에 따라 지운다.
- ==**릴리스가 그대로 돈다.**== 0.44.0 릴리스 PR(`release/0.44.0`) 실행이 네 행, man, 대시보드, `build-manifest-<트리>`를 올리고 `publish.yml` 시험 실행의 `fetch-build`가 그 실행을 고른다(`release-build-once`의 확인). 이 값이 안 나오면 이 변경이 `manifest`나 `release` 모드를 깬 것이다.
- 같은 커밋에서 `just lint`와 `canon gate check`가 통과한다. 새 evidence는 없다.

## Out of scope

- bench-shape 재사용(`test.yml`의 `bench-shape`를 `build-native.yml`의 행으로 옮기거나 릴리스에서만 돌리는 일): 오너가 이번 목록에서 뺐다.
- 잡 합치기(atip를 offline에, `cargo test` 쪼개기, nextest), 테스트 바이너리 통합, 캐시 키를 서드파티 lock 내용으로 바꾸는 일, `safety`의 중복 제거.
- 플랫폼별 변경 감지(`platform*` 경로 등)와 `native`가 아닌 PR의 macOS, Windows, arm 컴파일. 릴리스 PR이 처음 본다.
- 일반 PR에서 main에 캐시를 시딩해 musl 행이 따뜻하게 시작하게 하는 일(캐시를 저장하지 않으므로 일반 PR의 musl 행은 푸시마다 처음부터 빌드한다).
- `publish.yml`, `scripts/release-build.py`, 브랜치 보호, 러너 라벨, 시간 제한, `concurrency`.
- main 푸시의 사후 초록 신호를 다른 곳에서 보충하는 일(오너가 받아들였다: 아래 결정).

## Questions and assumptions

- 결정(owner, 2026-11-18, ask): ==**main 푸시는 PR이 이미 시험한 트리를 다시 돌리지 않는다.**== 캐시를 채우는 시딩은 남긴다. 이유: 확인한 7쌍의 트리가 같다. main에는 브랜치 보호가 없고 `publish.yml`은 main의 시험 결과를 읽지 않는다. 잃는 것은 병합 직후의 초록 신호이고 표본에서 붉은 3개 중 하나는 같은 트리가 PR에서 초록인 깜빡임이었다. 직접 푸시와 기준이 앞서간 병합(트리가 다름)은 마커가 없어 전체를 돈다.
- 결정(owner, 2026-11-18, ask): ==**일반 PR의 `build-native.yml`은 musl x64 행만 돌린다.**== macOS, Windows, arm, man은 `release/*`와 수동 실행에서만, 네 행은 `Cargo.lock`, `build.rs`, `.cargo/`가 바뀌면 일반 PR에서도 돈다. 이유: 일반 PR에서 만든 바이너리는 게시되지 않는다. macOS 한 행이 가중 분의 76%이고 용량 위험(`macos-26`이 0.43 시험 실행에서 배정되지 않았다)을 진다. 대가: `cfg(windows)`나 macOS 배포 타깃, alpine jemalloc에만 걸리는 컴파일 오류가 기능 PR이 아니라 릴리스 PR에서 처음 보인다.
- 결정(owner, 2026-11-18, ask): ==**`build-native.yml`의 캐시는 `release/*`와 수동 실행만 저장하고 저장 전에 줄인다.**== 이유: 캐시는 ref별이라 PR마다 복사본이 쌓인다(빌드 계열 8.9GB). 대가: 일반 PR의 musl 행은 푸시마다 캐시 없이 빌드한다(측정: 미스 368~445초, 적중 263~280초, 즉 푸시당 약 100~170초 더 든다. `Cargo.lock`이 같으면 적중했던 것). 줄인 뒤 크기는 [INFERENCE: 테스트 캐시가 정리로 464MB에서 308MB가 된 것에서 유추]로 약 35% 작다.
- 결정(owner, 2026-11-18, ask): ==**`man` 잡은 릴리스에서만 돈다.**== `pr-guard.yml`의 생성기 `cargo check`는 남는다. 이유: 일반 PR이 release 프로파일 `cargo run`에 204~240초를 쓸 이유가 없다. 대가: 릴리스 프로파일에서만 나는 man 생성 오류(컴파일은 `cargo check`가 잡는다)가 릴리스 PR에서 처음 보인다.
- 결정(owner, 2026-11-18, ask): ==**`live-tier`는 이미지를 cargo 빌드와 함께 준비한다.**== 이유: 서비스 준비 3분이 cargo 빌드와 독립이다. 이 잡은 임계 경로(`offline`)가 아니므로 시계는 같고 Linux 분이 약 2.5~3분 준다.
- 결정(owner, 2026-11-18, ask): ==**`offline`은 `canon gate check`를 `cargo test --workspace` 앞에 둔다.**== 이유: 게이트는 시험 출력을 읽지 않는다(Intent). 붉은 실행에서 14~17분을 아낀다.
- 결정(owner, 2026-11-18, ask): ==**모든 행이 도는 일반 PR의 조건은 `Cargo.lock`, `build.rs`, `.cargo/` 셋뿐이다.**== `rust-toolchain.toml`, `.github/actions/`, `build-native.yml`은 넣지 않는다. 대가: 이 파일들만 바꾸는 PR은 한 행만 돌고, 다른 행은 수동 빌드나 릴리스 PR이 처음 본다.
- 결정(owner, 2026-11-18, ask): ==**`dashboard`는 모든 실행에서 돈다.**== 16초, 1분 미만. 이유: 모든 행이 내장하고 건너뛰면 `build`의 `needs`와 `CANON_DASHBOARD_DIST` 처리가 복잡해진다. 대시보드 빌드 오류도 PR에서 보인다.
- 결정(owner, 2026-11-18, ask): ==**병합 뒤 0.43 ref의 빌드 캐시 20개를 `gh cache delete`로 지운다.**== 오케스트레이터가 병합 뒤 한다(이 변경의 구현이 아니다). 지워도 릴리스 PR이 다시 만든다.
- 가정: 행렬의 `exclude`나 `include`가 `fromJSON(needs.scope.outputs.<값>)` 식을 받는다[INFERENCE: GitHub 문서가 행렬 전체의 `fromJSON`만 밝힌다]. 첫 PR 실행이 증명한다. 안 되면 행 표를 `scripts/build-scope.py`에 두고 `scope`가 고른 행을 JSON으로 내며 행렬이 그것을 `fromJSON`으로 읽게 하고, 검사기는 그 표를 읽는다. 어느 쪽이든 행 표는 한 곳이고 `mode` 값이 모르는 값이면 네 행이다.
- 가정: `canon inventory sync`가 시험 앞에서 `.canon`의 gitignored 시나리오 기록을 쓰는데, 그 기록이 없는 체크아웃을 가정하는 시험은 없다[INFERENCE: 시험은 임시 저장소를 쓴다고 보지만 전수 확인하지 않았다]. 첫 PR 실행에서 `offline`이 같은 시험 수로 초록이면 맞다. 붉어지면 그 시험이 체크아웃을 읽는 것이므로 구현자는 시험을 고치지 않고 멈추고 보고한다.
- 가정: 병합 방식은 PR 병합 ref의 트리를 main에 그대로 올린다(병합 커밋, 스쿼시, 선형 리베이스 모두). 기준이 앞서갔으면 트리가 달라 마커가 안 맞고 전체를 돈다(안전한 쪽).
- 가정: 같은 저장소에 쓰기 권한이 있는 사람이 `test.yml`을 고쳐 마커만 올리는 일은 막지 않는다(포크는 `foreign_repo`로 막는다). main에 브랜치 보호가 없는 지금 신뢰 경계는 쓰기 권한이다.
- 가정: 병합 순서는 `canon-0-44-release-build-once`, `canon-0-44-search-memory`, 이 변경이다. 이 변경은 `build-native.yml`에서 `build-once`의 `manifest`, `dashboard`, `man` 잡과 `scope` 출력을 고치고 같은 검사기와 시험 파일을 다시 고치므로 `build-once`가 먼저 들어가야 한다(`build-once`는 캐시 단계를 건드리지 않는다: I18). `search-memory`는 CI 파일을 바꾸지 않는다[INFERENCE]. 이 변경 하나만 먼저 병합하면 `manifest` 잡이 없어 I7이 가리킬 대상이 없다.
- 가정: 구현은 커밋 셋이다. 첫째: `test.yml`(`tested`, `record`, 시딩 모드, 게이트 순서, `live-tier`)과 `ci-tested.py`와 시험. 둘째: `build-scope.py`, `build-native.yml`(모드, 행, 캐시 저장), 정리 스크립트, 검사기와 시험. 셋째: 문서. 되돌림은 커밋 단위다: 첫째를 되돌리면 main 푸시가 다시 전체를 돈다.
- 추정 절감 [INFERENCE: 위 측정의 표본]: main 푸시당 Linux 약 50분(530분 14일치 중 약 500분에서 시딩 약 25분 제외), 일반 PR당 `build-native.yml` Linux 약 35분과 가중 약 160분(측정 표본이 38분과 165분), `live-tier` 약 2.5분, 붉은 `offline` 실행에서 14~17분(PR 실행의 20%). 초록 실행의 시계는 줄지 않는다(`offline`이 임계 경로: 15~31분). 빌드 캐시 전체는 11.12GB에서 릴리스 한 벌 약 5GB로 내려갈 것이다.

Approved: journey 2026-11-18T14:58:33Z — "오너가 채팅에서 'CI 최적화'를 요청하고 레버 6개와 질문 3개를 ask로 답함(2026-11-18). 승인은 오케스트레이터에게 위임. 독립 계획 리뷰 1라운드 approve(P0/P1 없음), P2/P3 반영. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 93bd33395620d15d38198ac8e92c5749c2369b57) (brief sha256:854a909b5d6ab01b8dbb9e6aaf8d594e33cf9f4cbe4d2036a54faf9cae22c4dc)
