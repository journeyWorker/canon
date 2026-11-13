---
kind: chore
waive:
  - rule: "prose.code-citation"
    section: "intent"
    reason: "시험 바이너리 이름, 설정 파일 경로, 잠금 파일 이름이 이 변경이 다루는 대상 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "워크플로 파일, 시험 이름, 레시피, 스크립트 경로가 이 변경이 바꾸거나 지켜야 하는 대상 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "잠금 파일 이름과 레시피 이름은 검사하는 대상 자체다"
  - rule: "prose.length"
    section: "brief"
    reason: "CI 한 잡의 시간, 캐시, 취소, 시간 제한, 작업 트리 검사를 한 변경으로 묶어 그 측정값과 불변식이 모두 필요하다"
  - rule: "prose.length"
    section: "intent"
    reason: "실행 42559156525에서 잰 시간과 캐시 크기가 결정의 근거라 빼면 구현자가 다시 재야 한다"
  - rule: "prose.length"
    section: "invariants"
    reason: "여섯 항목(시험 실행, 캐시, 취소, 시간 제한, 작업 트리, 아티팩트)의 불변식과 잡별 시간 제한 값이 모두 필요하다"
  - rule: "prose.length"
    section: "checks"
    reason: "항목마다 CI 실행에서 보는 확인과 버리는 브랜치의 위반 넷이 필요하다"
---

# CI가 워크스페이스 테스트를 nextest로 돌리고, 캐시를 릴리스 범프 너머로 복원하며 main에서만 저장한다

Source: PR #143의 CI 실행 42559156525(잡 `cargo test --workspace (offline)`)의 로그와 단계별 시간, 저장소의 Actions 캐시 목록(2026-11-14 조회), 0.40.0 소스의 워크플로 네 개와 `.config/nextest.toml`, 벤더 감사 `codex/c-quality.md` §3.2·3.5, `herdr/h-quality.md` §3.5, `zed/z-quality.md` §3.5, `oh-my-pi/o-natives-build.md` §3.4·3.6, 형제 저장소 lute의 `efa69ea8`

## Intent

오프라인 테스트 잡은 한 번에 26분 4초가 든다. `cargo test`는 4코어 러너에서 테스트 바이너리를 하나씩 차례로 돌리고, 병렬은 한 바이너리 안에서만 일어난다. 캐시는 릴리스마다 비고, 모든 PR이 같은 캐시를 다시 저장한다.

- 시간: 설정 약 25초, 캐시 미스 상태의 컴파일 3분 29초, `cargo test --workspace` 약 19분, 세 호출자 단위 테스트 1분 45초, witness 32초, gate check 20초, 캐시 저장 34초다. 컴파일 가운데 의존성이 약 1분 20초이고 `canon-cli`와 그 테스트 유닛이 2분 8초다.
- 실행: 바이너리 267개의 시간 합이 1,237초다. 가장 느린 셋은 `atip_validate` 182초에 테스트 3개, `arg_rules` 95초에 9개, `schema_codes` 55초에 4개다. 0.40.0 소스의 `cargo nextest list --workspace`는 테스트 4,082개와 테스트 바이너리 221개를 센다. 무시된 시험 2개는 따로다. `canon-cli`가 테스트 2,532개와 통합 테스트 파일 173개다.
- 속도 이득의 한계: 바이너리 끝에서 느린 시험 하나가 코어를 놀리는 시간이 nextest가 채울 수 있는 몫이다. 실행이 이미 CPU로 포화였다면 얻는 것이 작다. 이 기계의 로컬 실행은 CPU 시간 합이 71분(user 54분, sys 17분)이었다. 어느 쪽인지 PR 실행이 말한다.
- 경합: 0.40에서 훅 테스트가 한 프로세스의 스레드로 돌며 같은 스크립트 파일을 서로 잘라 "unit tests under three callers" 단계가 간헐적으로 실패했다(커밋 10f5fbbbd). nextest는 테스트마다 프로세스를 쓴다.
- 설정: `.config/nextest.toml`은 JUnit 경로만 정하고 머리 주석이 "CI는 `cargo test`를 돈다"고 말한다. 0.40 스킬의 GitHub Actions 조리법은 `cargo nextest run --workspace --profile ci`와 `target/nextest/ci/junit.xml`을 말하는데 canon의 CI는 따르지 않는다.
- 캐시 키: `Cargo.lock`의 해시 하나이고 `restore-keys`가 없다. `Cargo.lock`에 워크스페이스 크레이트 11개의 버전이 들어 있어 릴리스 범프마다 키가 바뀐다. 0.35.0부터 0.40.0까지 릴리스 쌍 여섯 가운데 둘(0.37→0.38, 0.39.0→0.39.1)은 의존성이 같았다.
- 캐시 저장: PR 146의 병합 참조가 잡 일곱 개의 캐시를 통째로 저장했다(약 4.4GB, 오프라인 잡이 2,214MB). main에도 같은 것이 있고 활성 캐시는 23개 13.0GB다. 태그 실행(publish)은 태그마다 약 1.8GB를 저장하며 다른 태그는 복원하지 못한다. 같은 태그의 재실행은 쓴다.
- witness 32초 가운데 31.86초는 서드파티 크레이트를 다시 컴파일하는 시간이다. `-p canon-cli`가 `--workspace`와 다른 feature 조합을 고른다. 로컬에서 같은 컴파일 목록이 재현된다. 문서 시험은 라이브러리 11개 모두 0개를 돌고 4초가 든다. nextest는 문서 시험을 돌지 않는다.
- 빠진 것: 네 워크플로 어디에도 `concurrency`와 `timeout-minutes`가 없다. 시험 뒤 작업 트리가 깨끗한지 보는 단계도 없다. 증거, 지적, 주제가 추적되는 파일이라 시험이 남긴 파일이 곧 레저 오염이다.

## Category

ci

## Invariants

- ==**canon의 바이너리, 출력, 레저는 바뀌지 않는다.**== CI 정의, nextest 설정, 그것을 따르는 contributor 레시피와 스크립트, 주석, CHANGELOG만 바뀐다.
- 시험은 지우지도 건너뛰지도 않는다. 뒤에 CI가 도는 시험의 집합은 앞의 `cargo test --workspace`와 같다.
- 오프라인 잡은 `cargo nextest run --workspace --profile ci`로 시험을 돈다.
- `ci` 프로필은 `fail-fast = false`와 `retries = 0`을 정한다. `slow-timeout`은 60초 주기로 느린 시험을 표시하고 열 번째 주기에 종료한다.
- `ci` 프로필은 JUnit을 `target/nextest/ci/junit.xml`에 쓴다. 0.40 스킬 조리법의 경로다. 값마다 이유가 주석에 있다. 기본 프로필은 그대로다.
- nextest는 고정 버전의 릴리스 타르볼로 설치하고 풀기 전에 체크섬을 확인한다. 외부 설치 액션은 새로 들이지 않는다.
- JUnit 보고서는 시험이 실패해도 CI 아티팩트로 올라간다.
- 세 호출자 단계와 witness는 다른 프로필(`ci-aux`)로 돈다. 주 보고서를 덮어쓰지 않는다.
- 세 호출자 단계는 같은 세 환경에서 `--lib` 시험을 nextest로 돈다.
- witness는 같은 `--workspace` 선택에서 무시된 시험 하나만 돈다. 서드파티 크레이트를 다시 컴파일하지 않는다.
- 문서 시험 단계(`cargo test --workspace --doc`)는 남는다.
- 시험 뒤 작업 트리에 추적되지 않은 파일이나 바뀐 파일이 있으면 오프라인 잡이 실패하고 목록을 출력한다.
- test.yml의 캐시 잡 여섯은 복원과 저장을 나눈다. 잡은 offline, clippy, atip, grammar-mutation, bench-shape, live-tier다. 복원은 모든 실행에서 하고 키 접두사의 `restore-keys`를 둔다.
- 저장은 main으로의 push에서 정확한 키로 복원하지 못했을 때만 한다. 잡이 취소되지 않았으면 실패한 잡도 저장한다. 저장 직전에 워크스페이스 크레이트의 빌드 결과를 지워 의존성 빌드만 남긴다. 시험 바이너리가 그 결과에 든다.
- 오프라인 잡은 복원한 키, 정확한 일치 여부, `target/`과 `~/.cargo`의 크기를 잡 요약에 쓴다.
- test.yml, build-native.yml, pr-guard.yml은 같은 PR의 새 push가 앞의 PR 실행을 취소한다. main push는 서로 취소하지 않고 대기열에서 버려지지도 않는다. publish.yml에는 `concurrency`를 두지 않는다.
- 네 워크플로의 모든 잡이 `timeout-minutes`를 갖는다. publish.yml도 든다. 값은 측정한 가장 긴 실행의 약 세 배를 5분 단위로 올린 것이다.
  - test.yml: offline, atip, grammar-mutation, bench-shape, live-tier는 30. clippy는 20. launcher, platformer는 10.
  - build-native.yml: safety 5, dashboard 10, man 15, build(세 행) 45.
  - pr-guard.yml: safety 5, man-check 15.
  - publish.yml: read-version 5, dashboard 10, man 15, build(세 행) 45, publish 15.
- publish.yml의 캐시 단계는 그대로다. build-native.yml과 pr-guard.yml의 캐시는 PR에서 지금처럼 저장한다.
- 릴리스 워크플로 안전 검사와 그 시험은 새 워크플로를 규칙 추가 없이, 예외 추가 없이 받아들인다.
- test.yml과 justfile을 읽는 canon의 시험은 그대로 통과한다. 시험은 `atip_check`, `atip_claim14`, `atip_claim15`, `atip_claiml2`, `git_boundary`, `ci_evidence_docs`다. 잡 이름 `atip`와 `clippy`와 그 단계의 명령은 바뀌지 않는다.
- `just test`, `just test-callers`, `scripts/check-witness.sh`는 CI와 같은 nextest 명령을 돈다. `.config/nextest.toml`의 머리 주석이 이를 말한다.

## Checks

- ==**이 PR의 마지막 커밋에서 모든 CI 잡이 통과한다.**==
- 오프라인 잡을 실행 42559156525와 나란히 놓은 표를 리뷰에 남긴다. 잡 전체, nextest 단계, 컴파일, 세 호출자, witness의 초와 캐시 상태가 든다.
- 잡 전체가 26분 4초보다 짧다. 더 길면 이 변경의 전제(코어가 놀았다)가 틀린 것이므로 리뷰에 그 숫자를 쓰고 멈춘다.
- 목표는 15분 이하다 [추정]. 가장 느린 시험 182초가 단계의 하한이고, 실행이 CPU로 포화였다면 지금과 비슷하다. 못 미치면 JUnit에서 가장 느린 시험 다섯을 리뷰에 적고, 40초를 넘는 시험에 `priority` 오버라이드를 더한다.
- JUnit 아티팩트가 있고 안의 시험 수가 같은 커밋의 `cargo nextest list --workspace` 수와 같다. 실패한 실행의 아티팩트도 있다.
- 버리는 브랜치에서 위반을 하나씩 넣어 해당 단계가 실패하는 것을 확인하고 결과를 리뷰에 남긴다.
  - 실패하는 시험 하나: 다른 시험은 계속 돌고, 그 시험은 한 번만 돌고, JUnit에 실패가 있다.
  - 저장소에 파일 하나를 쓰는 시험: 작업 트리 단계가 실패하고 그 파일을 말한다.
  - 체크섬 한 자리를 바꾼 nextest 설치: 풀기 전에 실패한다.
  - PR에 두 번째 push: 첫 실행이 `cancelled`다.
- `timeout-minutes`가 없는 잡이 네 워크플로에 0개다. 검사는 한 번이다.
- PR의 첫 push에서 복원 단계가 main의 캐시를 복원했다고 말한다. 같은 `Cargo.lock`이면 정확한 일치이고 다르면 접두사 일치다. 어떤 잡도 캐시를 저장하지 않는다.
- 병합 뒤 main의 첫 push 실행이 정확한 키로 저장한다. 같은 `Cargo.lock`의 다음 실행은 정확한 일치이고 저장 단계가 건너뛰어진다. 저장된 오프라인 캐시는 이전 2,214MB보다 작다. 이 확인은 병합한 세션이 남긴다.
- 릴리스 워크플로 안전 검사와 그 시험이 통과한다. 체크섬 줄을 지운 복사본에서는 검사가 "SHA-256 verification"으로 실패한다.
- `just test`, `just test-callers`, `scripts/check-witness.sh`가 통과한다.
- 이 변경은 워크플로와 스크립트를 바꾸므로 코드 변경으로 센다. 이 변경이 adopt된 출시된 subject는 다시 열어 새 evidence를 기록하고 출시한다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Questions and assumptions

- 측정: 0.40.0 소스에서 로컬 `cargo nextest run --workspace`(macOS, 부하 평균 20~76)가 4,082개 중 4,082개 통과, 무시된 2개 건너뜀, 19분 32초였다. 시험 코드 변경 없이 통과한다. 시간은 기계 부하의 값이라 CI 시간의 근거가 아니다. 작업 트리의 잔여물은 같은 작업 트리에서 다른 세션이 레저를 바꾸고 있어 재지 못했다.
- 결정: nextest는 릴리스 타르볼 다운로드와 체크섬 확인으로 설치한다. 버전은 contributor가 로컬에서 쓰는 0.9.146이고 `x86_64-unknown-linux-gnu` 타르볼의 sha256은 `682c21b777c333e96fd532e114d3a5a894e0729ab88d94c0a9f20f8419695428`이다. 프로젝트가 게시하는 `.sha256` 파일과 내려받아 계산한 값이 같음을 확인했다. 구현자가 다시 계산한다. `Install duckdb CLI` 단계와 같은 모양이라 안전 검사가 이미 받아들인다. lute의 `taiki-e/install-action`은 가변 참조라 SHA 고정 규칙과 맞지 않는다. `cargo install`은 소스 컴파일이라 시간이 들고 체크섬을 고정하지 못한다.
- 결정: `retries = 0`이다. 불안정한 시험은 가릴 것이 아니라 보일 버그다(oh-my-pi §3.4). 이번 훅 경합이 재시도가 있었다면 가려졌을 종류다. `fail-fast = false`는 한 실행에서 실패를 모두 보기 위해서다.
- 결정: `slow-timeout`의 600초 종료는 가장 느린 측정값 182초의 세 배 이상이다. 첫 JUnit으로 값을 다시 보고 같은 변경에서 조정할 수 있다.
- 결정: `priority`는 첫 실행의 JUnit에서 40초 넘는 시험의 이름을 읽은 뒤에만 준다(Zed §3.5). 지금 아는 것은 바이너리 이름과 시험 수뿐이다. 로컬 JUnit은 부하가 시간을 왜곡해 쓰지 않는다.
- 결정: 문서 시험 단계는 남긴다. 지금 0개에 4초라 비용이 없고, 나중에 문서 시험이 생겨도 조용히 빠지지 않는다. lute도 같다.
- 결정: 세 호출자 단계는 세 번 모두 남긴다. 첫 번째는 주 실행의 `--lib` 부분과 겹치지만 주 실행은 `CANON_WITNESS_DIR`를 켠 환경이다. 이득은 불확실하다 [추정]. 한 번에 약 34초가 거의 모두 `canon-cli` 단위 시험 바이너리 하나이고 이미 4스레드로 돈다. 시험마다 프로세스를 띄우는 비용이 병렬 이득을 깎을 수 있다. 105초보다 느리면 리뷰에 적는다.
- 결정: witness와 gate check가 `-p canon-cli`로 컴파일을 되풀이하지 않게 한다. 확인은 nextest 단계 뒤 어느 단계의 로그에도 서드파티 크레이트의 `Compiling` 줄이 없는 것이다. 방법은 구현자가 고른다. 이미 빌드된 디버그 `canon`을 gate check가 쓰는 것이 한 방법이다. 안 되면 비용을 옮기기만 한 것이므로 witness와 gate check의 합이 지금의 52초를 넘지 않음을 리뷰에 적는다.
- 결정: 저장 전에 워크스페이스 크레이트의 빌드 결과를 지운다. 로컬 디버그 빌드에서 테스트 바이너리 226개가 `target/debug` 7.0GB의 4.2GB(60%)다. 접두사 복원을 쓰면 릴리스마다 옛 버전의 시험 바이너리가 캐시에 쌓이고 cargo는 치우지 않는다. 한편 `canon-cli`의 라이브러리를 건드리면 시험 유닛 175개가 모두 다시 컴파일되고 링크된다. 로컬에서 그 재빌드의 유닛 시간 628초 가운데 590초(94%)가 이 유닛들이다. 캐시에 시험 바이너리를 남겨 얻는 것이 작다. 컴파일 시간이 늘면 리뷰에 적고 이 결정을 다시 본다.
- 결정: 락 파일 해시에서 워크스페이스 버전을 빼는 방식(oh-my-pi §3.6)은 쓰지 않는다. 릴리스 쌍 여섯 가운데 둘에서만 정확한 일치를 지키고 나머지는 접두사 복원이 같은 결과를 낸다.
- 결정: 취소는 PR 실행에만 한다. main push의 그룹 이름에 커밋 해시를 넣어 실행마다 따로 둔다. 같은 그룹의 대기 실행은 최신 하나만 남으므로 이렇게 하지 않으면 연속 push에서 중간 실행이 버려진다.
- 결정: JUnit 아티팩트 이름은 `junit-offline`이고 보관은 14일이다. 증거로 남기는 것은 레저의 기록이고 아티팩트가 아니다.
- 결정: 작업 트리 단계는 gate check 뒤 잡의 마지막에 `if: always()`로 둔다. 시험이 실패해도 무엇이 남았는지 보인다(codex §3.2).
- 질문: canon 자신의 CI가 `canon evidence add --from-ci`로 증거를 남기게 할지? 레저 기록을 main에 푸시하려면 워크플로에 `contents: write`가 필요하다. `.canon/policy.yaml`에 `spec_coverage.require_ci_run`을 켜는 것은 정책이라 사람이 정한다. 0.40 brief의 질문 2가 아직 열려 있다. 기본: 이번 변경에서는 하지 않는다. JUnit 경로와 `ci` 프로필이 조리법과 같아서 뒤의 변경은 단계 몇 개와 정책 한 줄이다.
- 질문: build-native.yml(PR에서만 도는 잡 네 개)과 pr-guard.yml의 캐시를 지금처럼 PR에서 저장하게 둘지? 이 잡들에는 복원할 main 캐시가 없어서 main에서만 저장하면 캐시가 사라진다. 지금처럼 두면 PR마다 약 2GB(364+535+460+462+177MB)가 쌓인다. 기본: 지금처럼 PR에서 저장한다. 대안은 캐시를 없애는 것(macOS와 Windows 빌드가 처음부터 돈다)과 이 잡들이 main push에서도 돌게 하는 것(러너 시간이 든다)이다.

## Out of scope

- `crates/canon-cli/tests/`의 테스트 바이너리 통합(173개 파일). 링크 시간을 먼저 쟀다. `canon-cli` 라이브러리를 바꾼 로컬 재빌드가 85초이고 시험 유닛 175개가 유닛 시간의 94%다. 시험 바이너리 하나는 바꿔도 다시 컴파일하고 링크하는 데 약 2초다. CI에서 같은 유닛이 컴파일 3분 29초의 약 2분을 차지하므로 이 변경 뒤 가장 큰 컴파일 지렛대다. 그래도 이 변경에 넣지 않는다. 모든 시험의 이름과 JUnit 분류가 바뀌어 시나리오 증거의 묶음을 다시 확인해야 하고, 파일 173개의 이동이라 CI 정의 변경과 따로 리뷰해야 한다. 다음 변경으로 다룬다.
- `--from-ci` 증거를 canon 자신의 CI가 남기는 일(위 질문).
- 샤딩, `nextest archive`, 바뀐 패키지만 도는 rdeps 필터. 규모가 맞지 않는다. 시험 2,532개가 `canon-cli` 하나에 몰려 다른 크레이트를 바꿔도 rdeps가 `canon-cli`로 모인다. 샤딩은 러너와 아티팩트 전달 비용이 들고 이득은 실행 단계의 몇 분이다. nextest 단계가 목표를 못 맞춘 뒤에 다시 본다.
- publish.yml의 캐시 정책과 `concurrency`. 릴리스는 취소하지 않고 재실행을 빠르게 둔다. `timeout-minutes`만 더한다.
- `scripts/check-mutations.sh`와 `grammar_t15_mutation`이 `cargo test`로 돌리는 복사본 빌드, `just docs`.
- 첫 번째 호출자 실행(주 실행과 겹침)을 지우는 일.
- 릴리스 워크플로 안전 검사에 `timeout-minutes`나 `concurrency` 규칙을 더하는 일. 이 변경은 그 검사가 새 워크플로를 받아들이는지만 본다.
- 디버그 정보 프로필 조정(`Cargo.toml`이 이미 줄인 값), `cargo-deny`와 `cargo-shear`.
