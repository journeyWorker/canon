---
kind: chore
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "워크플로 파일, 액션 이름, lint 이름은 이 변경이 바꾸는 대상 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "워크플로 파일, 키 문자열, 레시피, 스크립트 경로가 이 변경이 바꾸거나 지켜야 하는 대상 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "확인은 파일과 명령의 철자를 그대로 말해야 구현자와 리뷰어가 같은 것을 돌린다"
  - rule: "prose.decision-history"
    section: "intent"
    match: "2026-10-19"
    reason: "러너 라벨이 옮겨 가는 기간의 시작일이다. 시행 날짜가 동작의 일부다"
  - rule: "prose.length"
    section: "brief"
    reason: "캐시, 시간 제한, 취소, 액션 핀, 러너를 한 변경으로 묶어 측정값과 불변식이 모두 필요하다"
  - rule: "prose.length"
    section: "intent"
    reason: "실행 기록에서 잰 시간, 이미지, 캐시 크기, 액션 버전이 결정의 근거라 빼면 구현자가 다시 재야 한다"
  - rule: "prose.length"
    section: "invariants"
    reason: "여섯 항목의 불변식과 잡별 시간 제한 표가 모두 필요하다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변식마다 CI 실행에서 보는 확인이 하나씩 필요하다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 실행 번호와 값의 출처다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 키 이름 구간과 시험 이름의 예다"
  - rule: "prose.paragraph-length"
    section: "checks"
    reason: "병합 전후 확인이 한 항목에서 이어져야 순서가 보인다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "키 문자열과 그룹 식의 철자가 한 문장이다"
  - rule: "prose.sentence-length"
    section: "checks"
    reason: "확인 명령과 기대 결과의 열거가 한 문장이다"
---

# CI는 캐시를 main에서만 저장해 다음 실행에 복원하고, 시간 제한과 취소를 갖고, 액션과 러너를 고정된 최신으로 쓴다

Source: 0.41.0 소스의 워크플로 네 개(`.github/workflows/test.yml`, `build-native.yml`, `pr-guard.yml`, `publish.yml`)와 `rust-toolchain.toml`, 보류한 0.41 brief `canon-0-41-ci-nextest`(`git show plan/0.41:.canon/changes/canon-0-41-ci-nextest/brief.md`)와 그 브랜치 `wip-0.41/ci-nextest`(`25ac66a79`, `7c897e9df`), 측정 기록 `ci-nextest-measurement.md`(NAS `2026-11-14-0.41-plan-review`), 저장소의 Actions 실행 기록과 캐시 목록(2026-11-14 조회), GitHub 변경 공지 `2026-09-17-ubuntu-26-generally-available-and-latest-migration`

## Intent

0.41 계획은 CI를 nextest로 옮기는 변경이었다. 측정한 결과 nextest는 빨라지지 않았다(실행 43498921480이 33분 32초, 기준선 30분 1초). 시험이 프로세스마다 비싼 준비를 다시 계산하기 때문이고, 그 비용은 `canon-0-42-test-cost`가 줄인다. 같은 측정에서 ==**캐시, 시간 제한, 취소는 nextest와 상관없이 이득이 확인됐다.**== 이 변경은 그 부분만 `cargo test`를 그대로 두고 가져오고, 밀린 갱신(액션, 러너)을 한 번에 한다.

- 캐시: 키가 `Cargo.lock` 해시 하나이고 `restore-keys`가 없다. 릴리스마다 락 파일의 크레이트 버전이 바뀌어 키가 비고 한 번에 3분 29초 컴파일이 돈다. 옛 키에서 복원하면 복원 39~68초에 시험 빌드 2분이었다(실행 43494599136, 43498921480). 저장은 모든 실행의 마지막에 일어난다. 풀 리퀘스트 149의 병합 참조가 잡 여섯의 캐시(오프라인 2,223MB 포함)를 통째로 저장했고, 태그 실행이 약 1.8GB를 저장하며, 활성 캐시는 18개 10.9GB다.
- 시간 제한과 취소: 네 워크플로 어디에도 `timeout-minutes`와 `concurrency`가 없다. 시험 최대는 `cargo test --workspace (offline)` 36.5분이다(test.yml 59회). 묶인 잡은 6시간의 기본 제한까지 러너를 쥔다. 같은 풀 리퀘스트의 앞선 실행은 새 push 뒤에도 끝까지 돈다. 보류한 브랜치의 `concurrency`는 풀 리퀘스트 147에서 앞선 실행을 취소했다.
- 액션 핀: 모두 전체 SHA로 고정돼 있지만 주석이 주 버전만 적는다. 지금 핀과 최신 릴리스(`gh api .../releases/latest`)는 다음과 같다.

| 액션 | 지금 핀 | 최신 | 핀 SHA(최신) |
| --- | --- | --- | --- |
| `actions/cache` | v5.1.0 | v6.1.0 | `55cc8345863c7cc4c66a329aec7e433d2d1c52a9` |
| `actions/upload-artifact` | v6.0.0 | v7.0.2 | `cf430e030ddbb5b0abf93d22962f4752f3646cd9` |
| `actions/download-artifact` | v6 | v8.0.2 | `9000827ccba6bdab643e8b6fd33ac0654aef8333` |
| `actions/checkout` | v7.0.1 | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1` (같다) |
| `actions/setup-node` | v7.0.0 | v7.1.0 | `949feb2413d6458794dcd2491c4babbbce0c15c1` |
| `oven-sh/setup-bun` | v2.2.0 | v2.2.0 | `0c5077e51419868618aeaa5fe8019c62421857d6` (같다) |

  `actions/download-artifact` v8은 내려받은 파일의 해시가 맞지 않으면 기본으로 실패한다(v7까지는 경고). 그 외 주 버전 변경은 모듈 형식(ESM) 이전과 단일 파일 직접 올리기 옵션(`archive: false`, canon은 쓰지 않는다)이다.
- 러너: 모든 워크플로의 Linux 잡이 `ubuntu-latest`다. 이 라벨은 2026-10-19~11-19에 걸쳐 Ubuntu 26.04로 옮겨 간다(GitHub 공지). 실행 기록에서 잰 이미지는 이렇다: test.yml의 clippy 잡은 11-09(기준선 42670247057), 11-13(43498921480, 43520006787, 43523582441), 11-14(43532051574) 실행이 모두 `ubuntu-26.04`였고, 릴리스 빌드 행은 v0.30.0(10-27)부터 v0.41.0까지 `ubuntu-26.04`였다(v0.28.0과 그 이전은 `ubuntu-24.04`). ==**우리가 잰 시간, 캐시 크기, 릴리스는 이미 26.04에서 나온 값이다.**== 라벨이 움직이는 동안은 같은 잡이 이미지를 바꿔 가며 돌 수 있고, 캐시된 `target/`의 빌드 스크립트와 proc-macro 바이너리가 다른 glibc의 이미지로 복원될 수 있다. 릴리스 두 워크플로의 라벨은 `canon-0-42-release-portability`가 정한다(그 brief의 I2). 이 변경은 거기에 손대지 않는다.
- nextest: 0.41.0 소스는 nextest를 어디에도 고정하지 않는다. `.config/nextest.toml`의 주석과 설정만 있고, 0.9.146 고정은 보류한 브랜치(`just`, README, test.yml)에만 있다. 올릴 고정이 없다.

## Category

ci

## Invariants

- I1 ==**canon의 바이너리, 출력, 레저는 바뀌지 않는다.**== 바뀌는 것은 CI 정의, `scripts/`의 스크립트, 주석, CHANGELOG뿐이다. 소스 코드, `justfile`, `.config/nextest.toml`, `rust-toolchain.toml`은 바뀌지 않는다.
- I2 시험은 `cargo test`로 돈다. 오프라인 잡의 시험 명령, 세 호출자 단계(`unit tests under three callers`), witness, gate check 단계의 명령은 그대로다. nextest로 바꾸지 않는다. `.config/nextest.toml`은 그대로 두고, nextest를 CI에 되돌리는 일은 `canon-0-42-test-cost`가 끝난 뒤 다시 잰다(Out of scope).
- I3 test.yml의 캐시 잡 여섯(offline, clippy, atip, grammar-mutation, bench-shape, live-tier)은 복원과 저장을 나눈다. 복원은 모든 실행에서 한다. 저장은 main으로의 push에서 정확한 키로 복원하지 못했을 때만 한다.
- I4 키는 `canon-test-<잡>-deps-<툴체인 해시>-<락 해시>`이고 `restore-keys`는 `canon-test-<잡>-deps-<툴체인 해시>-`, `canon-test-<잡>-cargo-` 순서다. 툴체인 해시는 `hashFiles('rust-toolchain.toml')`다(atip 잡의 검사기 캐시가 이미 그렇게 한다). 두 번째 접두사는 지금 main에 있는 옛 캐시이고, 툴체인이 이 변경에서 움직이지 않으므로 그 산출물을 쓸 수 있다. 이 접두사는 첫 PR을 위한 다리일 뿐이다: 툴체인을 올리는 변경이 이를 지운다. 잡 이름 구간은 지금 키의 것이다: `offline`, `clippy`, `atip`, `mutation`(잡 grammar-mutation), `bench-shape`, `live-tier`.
- I5 저장은 잡의 빌드를 끝내는 단계가 성공했을 때만 한다. 오프라인 잡은 `cargo test --workspace --locked --offline --no-run`을 따로 두고 그 결과를 본다. 시험이 실패해도 저장하고, `cargo fetch` 같은 빌드 전 실패는 저장하지 않는다. 다른 잡은 각자 빌드를 끝내는 단계를 고른다.
- I6 저장 직전에 워크스페이스 크레이트의 빌드 결과를 지워 의존성 빌드만 남긴다(보류한 브랜치의 `scripts/ci-strip-workspace-build.sh`에서 nextest 줄을 뺀 것을 출발점으로 한다). 오프라인 잡은 복원한 키와 정확한 일치 여부, `target/`과 `~/.cargo`의 크기를 잡 요약에 쓰고, main 저장마다 저장 크기도 남긴다.
- I7 atip 잡의 검사기 캐시(`~/.cargo/bin/atip`)는 그대로 `actions/cache` 한 단계이고 그 `key:` 줄은 `key: canon-test-atip-check-v0.3.0-`로 시작한다(`atip_claim16`이 이 철자를 읽는다).
- I8 build-native.yml과 pr-guard.yml의 캐시는 지금처럼 PR에서 저장한다(질문 1의 기본값). publish.yml의 캐시 단계는 그대로다. 액션 핀만 바뀐다.
- I9 test.yml, build-native.yml, pr-guard.yml은 같은 PR의 새 push가 앞의 PR 실행을 취소한다. 그룹은 `${{ github.workflow }}-${{ github.event_name == 'pull_request' && github.event.pull_request.number || github.sha }}`, `cancel-in-progress`는 `${{ github.event_name == 'pull_request' }}`다. main push는 서로 취소하지 않고 대기열에서 버려지지도 않는다. publish.yml에는 `concurrency`를 두지 않는다.
- I10 네 워크플로의 모든 잡이 `timeout-minutes`를 갖는다. 값은 규칙 하나다: 측정한 가장 긴 실행의 세 배를 5분 단위로 올린 값이다. 측정은 워크플로마다 최근 실행(test.yml 59회, build-native.yml 37회, pr-guard.yml 40회, publish.yml 30회)이다. 구현은 같은 방법으로 다시 재서 값을 확인한다. 지금 값은 다음과 같다.

| 워크플로 | 잡(측정 최대, 분) | 제한 |
| --- | --- | --- |
| test.yml | offline(36.5) | 110 |
| test.yml | atip(7.7) | 25 |
| test.yml | grammar-mutation(6.3) | 20 |
| test.yml | bench-shape(8.4), live-tier(8.5) | 30 |
| test.yml | clippy(3.3) | 10 |
| test.yml | launcher(1.3), platformer(1.1) | 5 |
| build-native.yml | safety(0.2), dashboard(0.7) | 5 |
| build-native.yml | man(5.1) | 20 |
| build-native.yml | build 세 행(14.7) | 45 |
| pr-guard.yml | safety(0.6) | 5 |
| pr-guard.yml | man-check(2.1) | 10 |
| publish.yml | read-version(0.7), dashboard(0.6) | 5 |
| publish.yml | man(5.2) | 20 |
| publish.yml | build 세 행(15.9) | 50 |
| publish.yml | publish(9.7) | 30 |

- I11 외부 액션은 모두 전체 SHA로 고정하고, 주석은 정확한 버전(`# v6.1.0`)을 적는다. SHA는 그 태그가 가리키는 커밋이다. 버전은 구현 때 각 저장소의 최신 릴리스이고, 위 표와 최신이 다르면 최신을 따른다. 같은 액션은 모든 워크플로에서 같은 버전이다. `setup-bun`은 이미 안전 검사가 워크플로 사이의 일치를 강제한다.
- I12 test.yml과 pr-guard.yml의 모든 잡은 `-latest`가 아닌 고정 러너 라벨 `ubuntu-26.04`를 쓴다(결정: 아래). build-native.yml과 publish.yml의 라벨은 `canon-0-42-release-portability`의 일이라 이 변경이 바꾸지 않는다.
- I13 `scripts/check-release-workflow-safety.py`와 그 시험은 새 워크플로를 규칙 추가 없이, 예외 추가 없이 받아들인다.
- I14 test.yml과 justfile을 읽는 canon의 시험은 그대로 통과한다: `atip_check`, `atip_claim14`, `atip_claim15`, `atip_claim16`, `atip_claiml2`, `atip_codestable_07`, `atip_optionforms`, `atip_optionvalues`, `git_boundary`, `ci_evidence_docs`, `hermetic_env.rs`. 잡 이름 `atip`와 `clippy`, 그 단계의 명령, 단계 이름 `unit tests under three callers`는 바뀌지 않는다.

## Checks

- C1(전체) ==**이 PR의 마지막 커밋에서 모든 CI 잡이 통과한다.**== 잡 `cargo test --workspace (offline)`은 `cargo test --workspace`가 26.04에서 시험 전부를 통과한다는 첫 증거다.
- C2(I2) 오프라인 잡 로그에서 시험 단계가 `cargo test --workspace --locked --offline`이고 문서 시험이 돈다. `git diff main -- .config/nextest.toml justfile`이 비어 있다.
- C3(I3~I5) PR의 첫 push: 여섯 잡의 복원 단계가 새 키는 찾지 못하고 옛 `-cargo-` 접두사로 main의 캐시를 복원했다고 요약에 복원한 키로 말하며, test.yml의 어떤 잡도 캐시를 저장하지 않는다. 병합 뒤 main의 첫 push 실행이 여섯 새 키를 저장한다. 같은 `Cargo.lock`의 다음 실행은 정확한 일치이고 저장 단계가 건너뛰어진다. `Cargo.lock`만 바뀐 다음 PR(릴리스 범프)은 `deps-<툴체인 해시>-` 접두사로 복원하고 저장하지 않는다. 이 병합 뒤 확인은 병합한 세션이 남긴다.
- C4(I5) 시험이 실패하는 버리는 브랜치에서, main 대신 `workflow_dispatch`나 임시 조건으로 저장 단계가 도는 상태를 만들어 저장 단계가 실패한 시험 뒤에도 도는 것을 본다. `cargo fetch`를 깨뜨린 버리는 브랜치에서는 저장 단계가 건너뛰어진다.
- C5(I6) 저장 단계 직전 `target/`의 크기가 요약에 있고, 저장된 오프라인 캐시가 이전 2,223MB보다 작다. `target/debug/deps`에 워크스페이스 크레이트의 시험 바이너리가 없다.
- C6(I7) `rg "key: canon-test-atip-check-v0.3.0-" .github/workflows/test.yml`이 한 줄을 낸다. `atip_claim16`이 통과한다.
- C7(I8) `git diff main`에서 build-native.yml, pr-guard.yml, publish.yml의 캐시 단계의 `key:`, `path:`, 저장 조건이 그대로다(액션 핀의 줄만 다르다).
- C8(I9) PR에 두 번째 push를 하면 첫 실행이 `cancelled`다. 같은 PR의 세 워크플로가 서로를 취소하지 않는다. main에 연달아 두 push를 하면 둘 다 끝까지 돈다(병합한 세션이 확인한다). `rg "^concurrency" .github/workflows/publish.yml`이 비어 있다.
- C9(I10) `timeout-minutes`가 없는 잡이 네 워크플로에 0개다. 확인은 워크플로 파일마다 `jobs:`의 키 수와 `timeout-minutes:` 줄 수를 비교하는 한 번의 실행이고, 결과를 리뷰에 적는다. 표의 값은 `gh api`로 다시 잰 최대의 세 배를 5분 단위로 올린 값과 같다. 다르면 측정이 이긴다. 한 번 실행에서 오프라인 잡의 시간이 110분에 닿지 않는다.
- C10(I11) 모든 `uses:` 줄에 대해 SHA가 `git ls-remote <저장소> refs/tags/<주석의 버전>`(주석 태그의 `^{}`가 있으면 그것)과 같다. 한 번의 실행이다. 각 저장소의 `releases/latest`가 주석의 버전과 같다. build-native.yml의 PR 실행은 `actions/download-artifact` v8로 `dashboard-app` 하나만 내려받는다. 게시 잡의 내려받기 넷(`canon-binary-*`와 `man-pages`)과 `actions/setup-node` v7.1.0은 태그 실행에서만 돌아 이 PR이 증명하지 못한다(v8은 해시 불일치를 기본으로 실패시킨다). 그 둘은 `canon-0-42-release-portability`의 시험 실행(`dry_run`, 그 brief의 I14)이 0.42.0 태그 전에 처음 돌린다. 그 변경이 이 변경 위에서 돌기 때문이다. 이 한계와 그 의존을 리뷰에 적는다.
- C11(I12) `rg "ubuntu-latest" .github/workflows/test.yml .github/workflows/pr-guard.yml`이 비어 있다. 두 워크플로의 모든 잡 로그가 `Image: ubuntu-26.04`를 말한다.
- C12(I13) `just release-check`가 통과한다. 체크섬 줄을 지운 duckdb 설치 단계의 복사본에서는 "SHA-256 verification"으로 실패한다(안전 검사가 새 워크플로를 받는다는 것은 위반을 넣어도 여전히 잡힌다는 뜻이다). 핀을 40자리 SHA에서 태그로 바꾼 복사본에서는 "full 40-hex commit SHA"로 실패한다.
- C13(I14) 위 시험 목록이 `cargo test -p canon-cli`에서 통과한다. 오프라인 잡이 이를 포함한다.
- C14(I1) `canon gate check`와 `just atip-deep`가 통과한다. `git diff main --stat`이 `.github/`, `scripts/`, CHANGELOG, 이 변경의 brief 밖의 파일을 담지 않는다(`crates/`, `justfile`, `.config/`, `rust-toolchain.toml`이 없다). 이 변경은 워크플로를 바꾸므로 코드 변경으로 센다. 이 변경이 adopt된 출시된 subject는 다시 열어 새 evidence를 기록하고 출시한다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.
- C15(전체) 리뷰에 오프라인 잡의 표를 남긴다: 이 PR의 잡 전체와 단계별 초, 기준선(0.41.0 main push 실행 43532051574, 31분 7초에 `cargo test` 단계 27분 27초)의 같은 수치, 복원한 키와 캐시 크기. 첫 PR도 옛 캐시를 복원하므로 이 변경의 이득이 이 PR에서 이미 보인다(0.41 측정: 복원 39~68초, 시험 빌드 2분, 미스의 컴파일 3분 29초).

## Questions and assumptions

- 결정: nextest는 이번에 넣지 않는다. 0.41 측정에서 nextest 단계가 느려진 원인은 시험마다 프로세스가 비싼 준비(`LazyLock`)를 다시 계산하는 것이었고(합산 시험 시간 1,502초가 6,578초), 그 해결은 `canon-0-42-test-cost`다. nextest는 그 뒤에 다시 잰다. `wip-0.41/ci-nextest`가 돌아올 때 nextest 고정은 0.9.152가 되고 타르볼의 체크섬은 그때 다시 계산한다. 지금 이 저장소에는 올릴 고정이 없다(위 Intent).
- 결정: 보류한 브랜치의 캐시 구조(복원/저장 분리, 빌드 끝 단계 기준 저장, 워크스페이스 빌드 제거, 요약), `concurrency`, `timeout-minutes`를 그대로 가져온다. 키에는 툴체인 해시를 더 넣는다(툴체인이 움직일 때 옛 빌드 결과를 복원하지 않게 하는 비용 없는 장치이고, 이 변경은 툴체인을 올리지 않으므로 옛 접두사 복원도 남긴다). nextest 전용 단계(`nextest run --no-run`, JUnit 올리기, `ci`와 `ci-aux` 프로필, 문서 시험 단계, witness의 nextest 명령, `rm -rf target/nextest`, 작업 트리 검사)는 들이지 않는다. JUnit과 작업 트리 검사는 이 변경의 범위 밖이다.
- 결정: 저장은 시험을 기다리지 않는다. 저장된 캐시가 `cargo test`가 빌드한 `target/debug/deps`의 워크스페이스 부분을 지운 의존성 빌드뿐이므로 시험이 `target/`의 이 부분을 바꾸지 않아도 된다.
- 결정: 러너는 `ubuntu-26.04`다(test.yml, pr-guard.yml). 고정하는 이유는 재현성이다: 라벨이 11-19까지 움직이는 동안 같은 잡이 이미지를 바꿔 가며 돌 수 있고, 캐시된 `target/`은 이미지가 달라지면 깨질 수 있다. 26.04를 고르는 이유는 우리가 잰 값(시간, 캐시, 릴리스 열두 번)이 모두 거기서 나왔고, 24.04로 고정하면 잰 환경에서 벗어나기 때문이다. `canon-0-42-release-portability`도 같은 라벨을 릴리스 쪽에 쓴다. 이미지를 바꾸는 일은 그 PR에서 캐시 키 이름 공간도 함께 바꿔야 한다(키에 이미지가 없다). 이 변경은 이미지를 바꾸지 않으므로 해당 없다. 첫 PR 실행에서 26.04 때문에 canon과 상관없는 실패가 나오면 그 잡만 `ubuntu-24.04`로 내리고 이유를 주석에 적는다. 내려간 잡은 이미지 구간이 든 자기 키 이름(`canon-test-<잡>-u2404-deps-<툴체인 해시>-<락 해시>`)을 쓰고 `-cargo-` 접두사로는 복원하지 않는다. 26.04에서 만든 `target/`의 빌드 스크립트와 proc-macro 바이너리를 다른 glibc의 이미지가 복원하지 않게 하려는 것이다.
- 결정: 러너 라벨의 영구 검사(안전 검사에 `-latest` 거부 규칙)는 이 변경에서 만들지 않는다. 같은 스크립트와 그 시험을 `canon-0-42-release-portability`(그 brief의 I10)가 규칙 단위로 고치고, 이 변경은 안전 검사를 건드리지 않는다는 것이 I13이다.
- 결정: 두 변경이 `publish.yml`과 `build-native.yml`의 같은 잡 주변 줄을 고친다(`runs-on`과 `timeout-minutes`는 이웃 줄이다). 병합 순서는 이 변경이 먼저, `canon-0-42-release-portability`가 뒤다. 뒤의 변경이 이 변경 위로 옮기고, 그 변경이 더하는 잡(smoke, arm 행)에는 I10의 규칙으로 `timeout-minutes`를 정한다. 그 변경의 컨테이너 설치 단계가 Linux 빌드 행을 늘리면 그때 build 행의 제한을 다시 잰다.
- 결정: 액션을 정확한 버전 주석으로 적는다. 구현이 SHA를 `git ls-remote`로 다시 확인하며, 위 표는 2026-11-14의 값이다.
- 결정: build-native.yml과 pr-guard.yml의 라벨은 pr-guard.yml만 이 변경이 `ubuntu-26.04`로 바꾼다. build-native.yml의 Linux 잡(safety, dashboard, man, build 행)은 릴리스 두 워크플로의 라벨 일이라 `-latest`로 남긴다.
- 질문: build-native.yml(PR에서만 도는 잡)과 pr-guard.yml의 캐시를 지금처럼 PR에서 저장하게 둘지? 이 잡들은 복원할 main 캐시가 없어서 main에서만 저장하면 캐시가 사라진다. 지금처럼 두면 PR마다 약 2GB(535+460+462+365+177MB)가 쌓인다. 기본: ==**지금처럼 PR에서 저장한다.**== 대안은 캐시를 없애는 것과 이 잡들이 main push에서도 돌게 하는 것(러너 시간이 든다)이다.

## Out of scope

- nextest로의 전환, JUnit 보고서, 작업 트리 검사, `ci`와 `ci-aux` 프로필, nextest 고정(0.9.152). `canon-0-42-test-cost`가 끝난 뒤 다시 잰다.
- 시험 자체의 비용(바이너리 통합, `LazyLock` 준비, 느린 atip 시험).
- witness와 gate check가 `-p canon-cli`로 서드파티 크레이트를 다시 컴파일하는 비용(보류한 브랜치에서 32초로 쟀다). 이 변경은 그 단계에 손대지 않는다.
- 릴리스 두 워크플로의 러너 라벨, 컨테이너, 바닥값, 새 플랫폼(`canon-0-42-release-portability`).
- 안전 검사에 `timeout-minutes`, `concurrency`, `-latest` 규칙을 더하는 일.
- publish.yml의 캐시 정책과 `concurrency`. 릴리스는 취소하지 않는다.
- 툴체인 올리기(1.99.1 → 1.100.0). owner가 4개 Rust 프로젝트의 공유 툴체인을 나중에 함께 옮기기로 했고 1.100.0은 나온 지 며칠이다. 올릴 때는 `rust-toolchain.toml`의 규칙대로 PR 하나에서 clippy 잡의 `-D` 목록을 다시 확인하고, 이 트리에서 1.100.0의 clippy가 `clippy::map_unwrap_or` 한 곳(`canon-model`의 `handoff.rs:174`)을 걸었다는 2026-11-14의 측정을 출발점으로 쓴다. clippy 잡의 이름과 test.yml 주석에 적힌 버전 문구도 그 변경이 다룬다. 그 변경은 옛 `canon-test-<잡>-cargo-` restore-key도 지워, 옛 툴체인의 `target/`이 복원되지 않게 한다.
- 캐시 크기 정리(쓰이지 않는 `target/*/deps` 항목 삭제). 오프라인 캐시가 3GB를 넘으면 후속 변경이다.
- `canon evidence add --from-ci`로 CI가 증거를 남기는 일과 `spec_coverage.require_ci_run` 정책.

Approved: journey 2026-11-16T05:46:07Z — "오너가 채팅에서 오케스트레이터에게 승인을 위임함 (2026-11-16, '일단 한번에 가자 기본안대로'). 독립 계획 리뷰 1라운드 approve, P0/P1 없음. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree f41bb88d9c43e1d43b2e67ad37c1cc46ab96a8e3) (brief sha256:49c3a408b2d59a5c5e7d383d17d4f83102f82eb1d3a3ace46c34136447c72cf0)
