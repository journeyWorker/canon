---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "실행 번호와 잰 값이 근거이고 불변마다 검사기와 시험의 입력 모양을 열거해야 해서 빼면 구현자가 다시 재야 한다"
  - rule: "prose.length"
    section: "intent"
    reason: "0.43 릴리스의 여섯 실행의 시간표와 증명 내용, 아티팩트 수치가 근거다"
  - rule: "prose.length"
    section: "invariants"
    reason: "불변마다 바뀌는 잡, 단계, 검사기 규칙을 열거해야 한다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변마다 확인과 거절 시험의 입력이 하나씩 필요하다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "워크플로 파일, 잡 이름, 명령, API 경로는 이 변경이 바꾸는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "워크플로 파일, 잡, 아티팩트 이름, 판정 코드는 검사기와 시험이 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "확인은 명령과 판정 코드의 철자를 그대로 말해야 구현자와 리뷰어가 같은 것을 돌린다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 실행 번호와 값의 출처다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 이름 목록과 옵션의 예다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 시험이 읽는 입력의 예다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "불변과 확인마다 잡, 단계, 판정 코드, 입력을 열거해야 해서 문장을 나누면 대응이 끊긴다"
  - rule: "prose.paragraph-length"
    section: "invariants"
    reason: "불변과 확인마다 잡, 단계, 판정 코드, 입력을 열거해야 해서 문장을 나누면 대응이 끊긴다"
  - rule: "prose.paragraph-length"
    section: "checks"
    reason: "불변과 확인마다 잡, 단계, 판정 코드, 입력을 열거해야 해서 문장을 나누면 대응이 끊긴다"
  - rule: "prose.key-sentence"
    section: "invariants"
    reason: "불변과 확인마다 잡, 단계, 판정 코드, 입력을 열거해야 해서 문장을 나누면 대응이 끊긴다"
  - rule: "prose.key-sentence"
    section: "checks"
    reason: "불변과 확인마다 잡, 단계, 판정 코드, 입력을 열거해야 해서 문장을 나누면 대응이 끊긴다"
  - rule: "prose.contrast"
    section: "intent"
    reason: "PR 실행이 빌드하는 것(병합 ref)과 실행이 가리키는 것(head)의 구분이 이 설계의 근거다"
---

# 릴리스는 한 번만 빌드한다: publish가 같은 트리의 build-native 실행에서 바이너리를 받아 쓴다

Source: .github/workflows/build-native.yml, .github/workflows/publish.yml, scripts/check-release-workflow-safety.py, scripts/test-release-workflow-safety.py, scripts/build-scope.py, scripts/test-build-scope.py, scripts/check-release-binary.py, scripts/release-smoke.sh, crates/canon-cli/build.rs, canon/skills-dev/repo-scaffold/SKILL.md, 실행 44149604709 · 44159678353 · 44153517114 · 44163896209 · 44221374394 · 44223010690(`gh api repos/journeyWorker/canon/actions/runs/<id>/jobs`, `…/attempts/<n>/jobs`, `…/artifacts`, `gh run view <id> --log`), 0.43.0의 npm 출처 증명(`https://registry.npmjs.org/-/npm/v1/attestations/@journeykit%2Fcanon-core-darwin-arm64@0.43.0`), `actions/download-artifact` v8.0.2와 `actions/upload-artifact` v7.0.2의 `action.yml`, 측정 원본 /Volumes/Shelter/storage/journey-temp/projects/canon/2026-11-18-0.44-ci/build-once-measurements.md

## Intent

0.43 릴리스는 같은 트리의 같은 네 바이너리를 세 번 빌드했다. PR 실행(`build-native.yml`), 시험 실행(`publish.yml`, `workflow_dispatch`), 태그 실행이 각자 대시보드, man pages, 네 행을 처음부터 빌드했다. 세 실행이 보는 트리는 같다(`1177511831…`: PR head `9871fad03`와 병합 커밋 `dd12bba71`). 빌드 계열 잡(대시보드, man, 네 행)의 시간은 이렇다.

| 실행 | 빌드 계열 러너 시간(초) | 실행 시간(초) | 비고 |
| --- | --- | --- | --- |
| 44149604709 build-native, PR `efdd93091` | 3,006 | 860 | |
| 44159678353 build-native, PR `9871fad03` | 2,007 | 1,159 | macOS 행이 604초 기다린 뒤 시작 |
| 44153517114 publish 시험, `efdd93091` | 2,220 | 1,898 | macOS 잡이 두 시도 모두 901초 뒤 취소(`not acquired by Runner`) |
| 44163896209 publish 시험, `9871fad03` | 1,443 | 1,918 | 같은 취소 |
| 44221374394 publish 시험 | 2,008 | 709 | 마지막 빌드 행이 +606초에 끝나고 꼬리(팩, 스모크, 게시)가 98초 |
| 44223010690 publish 태그 | 2,999 | 1,015 | 마지막 빌드 행이 +860초에 끝나고 꼬리가 152초 |

- 0.43 한 번에 든 빌드 계열 러너 시간 228분 중 144.5분(63%)이 `publish.yml`에서 돌았다(`build-native.yml`은 83.6분). 시험 실행이 통과하고 태그 실행이 이어지는 정상 경로만 쳐도 릴리스당 83.5분이다.
- 태그 실행의 빌드가 가장 느리다(Windows 829초, 시험 실행은 571초). 다섯 캐시 복원이 모두 `Cache not found`였다. 캐시는 ref별이고 `build-native.yml`은 main에서 돌지 않아 기본 브랜치 캐시가 없다[INFERENCE: GitHub의 캐시 범위 규칙].
- macOS 용량 위험이 릴리스 시점에 걸려 있다. 0.43의 시험 실행 둘이 네 시도 모두 macOS 러너를 못 얻어 62분의 벽시계를 잃었다(아무 빌드도 진행되지 않았다). `publish.yml`에서 macOS 잡이 없어지면 그 위험은 PR 때의 `build-native.yml` 한 번으로 줄어든다(그 PR 실행도 604초 기다렸다).
- `build-native.yml`은 이미 같은 일을 한다. 바닥 검사(`scripts/check-release-binary.py`), 체크섬 짝, `--version` 실행, 아티팩트 올리기(`canon-binary-<target>`, `man-pages`, `dashboard-app`)가 같은 이름과 모양이다. `dashboard-app`은 바이너리에 내장되어 `publish.yml`의 다른 잡이 쓰지 않는다. `man-pages`는 `package`가 받는다.
- ==**PR 실행이 빌드한 것은 PR head가 아니라 병합 ref다.**== 실행 44159678353의 체크아웃은 `Merge 9871fad03… into 5c4f46fab…`(커밋 `40ae8728`)이고 API의 `head_sha`는 `9871fad03`이다. 병합 뒤에는 `refs/pull/152/merge`가 없고(`git ls-remote origin 'refs/pull/152/*'`는 head만 돌려준다) `40ae8728`도 읽히지 않는다. 그래서 "이 실행이 빌드한 트리"는 실행 안에서 적어 두지 않으면 나중에 구할 수 없다. 0.43에서는 기준 `5c4f46fab`가 head의 조상이라 병합 트리와 head 트리가 같았다[INFERENCE: `git merge-base --is-ancestor`로 읽었다]. 최근 태그 22개 중 병합 커밋인 20개는 모두 `태그^{tree}`가 PR head의 트리와 같다(둘은 부모가 하나).
- ==**npm 출처 증명은 바이너리가 어디서 컴파일됐는지 말하지 않는다.**== 0.43.0의 `canon-core-darwin-arm64` 증명에서 subject는 tarball의 sha512 하나, workflow는 `publish.yml@refs/tags/v0.43.0`, 커밋은 `dd12bba71`, 호출은 태그 실행 44223010690이다. 지금은 같은 실행이 컴파일하므로 그 실행의 로그가 바이너리의 출처다. 빌드를 옮기면 이 연결이 증명 밖에서 끊긴다.
- 범위 스크립트가 빌드 입력을 다 알지 못한다. `crates/canon-cli/build.rs`가 `canon/skills/`를 바이너리에 내장하고(`GUIDE_DIR`) 대시보드는 `bun.lock`과 루트 `package.json`으로 설치하는데, 셋 모두 `scripts/build-scope.py`의 `BUILD_PATHS`에 없다. 이 경로만 바꾸는 PR은 빌드 없이 병합된다. 바이너리 하나를 한 번만 빌드해 모두 그것을 쓰게 하면 `build-native.yml`의 범위가 곧 릴리스 바이너리의 범위다.
- 아티팩트가 90일 보존으로 쌓였다. 살아 있는 590개 10.4GB 중 `canon-binary-*`가 369개 8.4GB, `dashboard-app`이 73개 1.3GB다. `main`은 브랜치 보호가 없다(`branches/main/protection`이 404).

이 변경 뒤 `publish.yml`은 컴파일하지 않는다. 대시보드, man, 네 행의 빌드를 `build-native.yml`의 실행 하나에서 받는다. 그 실행은 게시할 커밋과 같은 트리를 빌드한 성공한 실행이고, 없으면 거절한다. 바닥 검사, 체크섬, 스모크, 팩, 게시는 그대로 돈다.

## Category

ci

## Invariants

- I1 ==**canon의 바이너리, 명령, 출력, 레저는 바뀌지 않는다.**== 바뀌는 것은 `build-native.yml`, `publish.yml`, 새 스크립트 `scripts/release-build.py`와 그 시험 `scripts/test-release-build.py`, 릴리스 검사기와 그 시험, 범위 스크립트와 그 시험, `canon/skills-dev/repo-scaffold/SKILL.md`와 그 설치본, CHANGELOG다. 소스 코드, `test.yml`, `pr-guard.yml`은 그대로다.
- I2 ==**컴파일은 `build-native.yml`에서만 한다.**== `publish.yml`에 `cargo`, `rustup`, `rustc`, `.github/actions/setup-rust`, 대시보드 빌드 명령, `host:`가 있는 행렬이 없고 작업 `dashboard`, `man`, `build`가 없다. 최상위 `env`에 `CARGO_*`와 `MACOSX_DEPLOYMENT_TARGET`이 없다. 빌드 행렬은 `build-native.yml`의 `build` 한 곳이다. 러너 라벨과 행 구성은 지금과 같다.
- I3 `build-native.yml`에 새 잡 `manifest`를 둔다. `scope`, `build`, `man`을 기다리고 `scope`의 규칙대로 건너뛴다(`if: needs.scope.outputs.skip != 'true'` 꼴). 네 행과 man이 모두 성공했을 때만 돈다. 이 잡은 자기 체크아웃의 `git rev-parse HEAD^{tree}`를 구하고, 네 아티팩트의 `.sha256`와 man 파일의 sha256을 읽어 매니페스트 `build-manifest.json`을 쓴다. 담는 것은 스키마 번호, 트리, 커밋(`GITHUB_SHA`), 이벤트, 실행 번호와 시도, 네 타깃의 바이너리 sha256, man 파일별 sha256이다. 아티팩트 이름은 `build-manifest-<트리>`다. 이름이 트리를 담아 API가 이름으로 찾는다.
- I3b 매니페스트를 쓰는 코드와 읽는 코드는 `scripts/release-build.py` 한 파일이다(`record`, `find`, `verify`). 스키마는 한 곳에 있다. 표준 라이브러리만 쓰고 러너의 `python3`와 `gh`를 쓴다.
- I4 `manifest` 잡이 매니페스트를 `actions/attest-build-provenance` v4.2.2(`4d101475d8b20a2381f78447822ac1eab6504dd8`)로 증명한다(owner 결정). `id-token: write`와 `attestations: write`는 이 잡에만 둔다. 같은 저장소의 풀 리퀘스트가 아닌 포크 PR 실행에서는 이 단계를 건너뛴다(그 실행의 매니페스트는 증명이 없어 `publish.yml`이 받지 않는다).
- I5 두 릴리스 워크플로의 모든 `upload-artifact` 단계가 `retention-days`를 둔다. `build-native.yml`의 `canon-binary-*`, `man-pages`, `build-manifest-*`는 14일(owner 결정), `dashboard-app`은 1일(같은 실행 안에서만 쓴다)이다. `publish.yml`이 다시 올리는 사본은 7일, `npm-tarballs`는 30일이다.
- I6 `publish.yml`의 새 잡 `fetch-build`가 `dashboard`, `man`, `build`를 대신한다. `ubuntu-26.04`에서 돌고 `read-version`을 기다리며 권한은 `contents: read`, `actions: read`, `attestations: read`다(`gh attestation verify`가 저장소의 증명 API를 읽는다). `timeout-minutes: 10`이다. `package`, `smoke-binary`, `smoke-network`는 `build` 대신 `fetch-build`를 기다린다. 그 잡들의 내용(내려받는 아티팩트 이름, 체크섬 확인, 실행 비트, 팩, 스모크)은 그대로다.
- I7 ==**찾기.**== `fetch-build`는 먼저 자기 체크아웃의 `git rev-parse HEAD^{tree}`를 구하고 `scripts/release-build.py find`를 돌린다. `find`는 아티팩트 API를 이름 `build-manifest-<트리>`로 불러(`repos/<저장소>/actions/artifacts?name=`) 후보를 새것부터 본다. 후보가 유효한 조건은 넷이다. 만료되지 않았다. 그 실행의 워크플로 경로가 `.github/workflows/build-native.yml`이다. 그 실행의 `conclusion`이 `success`다. 실행의 head 저장소와 저장소가 이 저장소다(포크 PR 거절). 이름의 트리와 매니페스트 안의 트리가 같아야 하는 확인은 `verify`가 한다(I9). 첫 유효 후보의 실행 번호를 `run_id` 출력으로 내보내고 작업 요약에 그 실행 번호, 커밋, 트리를 적는다.
- I7b `find`는 기다리지 않는다. 그 트리의 매니페스트가 없을 때는 이 커밋(`head_sha`)의 `build-native.yml` 실행을 한 번 더 보고, 커밋이 병합 커밋이면 각 부모의 `head_sha`도 본다(`fetch-build`의 체크아웃은 `fetch-depth: 2`이고 부모는 `HEAD^1`, `HEAD^2`다). 릴리스 PR의 실행은 PR head를 `head_sha`로 갖고 태그는 병합 커밋을 가리키므로 부모까지 보아야 아직 빌드 중인 PR 실행이 `in_progress`로 읽힌다. 돌고 있으면 `in_progress`, 성공한 실행이 있으나 다른 트리를 빌드했으면 `tree_mismatch`, 없으면 `no_run`이다.
- I8 ==**내려받기.**== `fetch-build`는 `actions/download-artifact`(지금 핀)로 `canon-binary-<target>` 넷, `man-pages`, `build-manifest-<트리>`를 `run-id: ${{ steps.<찾기>.outputs.run_id }}`, `github-token`, `repository`로 받는다. `digest-mismatch`는 기본값(`error`)이며 다른 값이면 검사기가 거절한다.
- I9 ==**확인.**== `scripts/release-build.py verify`가 받은 파일을 매니페스트와 맞춘다. 매니페스트의 트리가 `fetch-build`의 트리와 같고, 네 타깃이 모두 있고, 바이너리마다 sha256이 매니페스트와 `.sha256` 짝에 같고, man 파일이 매니페스트의 목록과 같다. 하나라도 다르면 거절이다. `gh attestation verify`(`--repo`, `--signer-workflow …/build-native.yml`)로 매니페스트의 증명을 확인하고 증명이 가리키는 실행이 `find`가 고른 실행과 같아야 한다.
- I10 ==**바닥 검사.**== `fetch-build`의 단계 `Check the binary keeps the floor of its platform`이 `scripts/check-release-binary.py`를 네 타깃 모두에 받은 파일로 돌린다(`if:` 없음). 이 스크립트는 macOS와 Windows 바이너리를 어느 OS에서나 읽는다(0.43 PR 실행의 네 바이너리를 맥에서 돌려 확인했다: macOS, Linux x64, Windows는 통과하고 Linux arm64는 페이지 크기만 실패한다). 단, aarch64 musl 행의 jemalloc 페이지 크기는 바이너리를 한 번 실행해서 읽으므로 x64 러너에서는 `Exec format error`로 실패한다(맥에서 확인했다). 그 행만 `--page-size skip`을 단계에 적고, `smoke-binary`가 그 바이너리를 받는 `ubuntu-26.04-arm` 행에서 실행 비트를 준 뒤 같은 스크립트를 `skip` 없이 돌린다. 모든 `smoke-binary` 행이 자기 타깃으로 이 검사를 돌린다.
- I11 ==**다시 올리기.**== `fetch-build`는 확인과 바닥 검사가 끝난 뒤에야 같은 이름(`canon-binary-<target>`, `man-pages`)으로 이 실행에 올린다. 뒤 잡이 받는 곳이 지금과 같다. 실패한 잡만 다시 돌려도 앞서 올린 것과 부딪치지 않는다(`overwrite: true`).
- I12 ==**거절은 판정 코드로 끝난다.**== `find`와 `verify`는 한 줄 JSON을 stdout에 쓴다(`verdict`, 거절이면 `reason`). 종료 코드는 0 통과, 1 거절, 2 판단 불가(API를 읽지 못함, 인자 오류)다. 2는 통과로 읽히지 않고 `fetch-build`는 실패한다. 거절이면 `fetch-build`가 실패하고 뒤 잡은 모두 `skipped`이며 아무것도 게시되지 않는다. 오류 안내는 처방 명령 한 줄을 말하지만 글자는 계약이 아니고 판정 코드가 계약이다.

  | 판정 코드 | 뜻 | 처방 |
  | --- | --- | --- |
  | `no_run` | 이 트리의 매니페스트도 이 커밋의 `build-native.yml` 실행도 없다 | `gh workflow run build-native.yml --ref <브랜치 또는 태그>`, 끝나면 `gh run rerun <publish 실행> --failed` |
  | `in_progress` | 이 커밋의 `build-native.yml` 실행이 아직 돈다 | 끝난 뒤 `gh run rerun <publish 실행> --failed` |
  | `tree_mismatch` | 이 커밋의 실행은 있으나 다른 트리를 빌드했다(기준이 앞서가서 병합 ref가 head와 달랐다), 또는 매니페스트 안의 트리가 이름과 다르다 | 브랜치에 기준을 병합해 PR을 다시 돌리거나 `gh workflow run`으로 그 커밋을 빌드 |
  | `expired` | 이 트리의 매니페스트가 모두 만료됐다 | `gh workflow run`으로 다시 빌드 |
  | `run_not_green` | 매니페스트를 올린 실행이 성공이 아니다 | 그 실행의 실패를 고친다 |
  | `foreign_repo`, `wrong_workflow` | 실행이 이 저장소의 `build-native.yml`이 아니다(대개 포크 PR) | 조사한다 |
  | `incomplete` | 매니페스트에 타깃이나 man 파일이 모자라거나 아티팩트가 없다 | 같은 트리를 `gh workflow run`으로 다시 빌드 |
  | `checksum_mismatch` | 받은 바이너리나 man 파일의 sha256이 매니페스트나 `.sha256` 짝과 다르다 | 조사한 뒤 다시 빌드 |
  | `attestation_invalid` | 증명이 없거나 이 저장소의 `build-native.yml`이 서명한 것이 아니거나 고른 실행의 것이 아니다 | 증명이 있는 실행으로 다시 빌드 |

- I13 ==**npm 출처 증명.**== 다섯 게시 단계의 `--provenance`는 그대로다. 증명은 계속 게시한 실행(`publish.yml`, 태그의 커밋)을 말한다. `publish.yml`의 머리 주석이 이 증명이 말하는 것과 말하지 않는 것(바이너리를 컴파일한 곳)을 적고, 바이너리의 출처는 `fetch-build`의 작업 요약과 `build-native.yml`의 매니페스트 증명이라고 쓴다.
- I14 `build-native.yml`의 트리거는 그대로다(`pull_request`, `workflow_dispatch`, main 푸시 없음). 워크플로 이름의 `(Test Only)`는 거짓이 되므로 `Build Native`로 바꾼다. `scripts/build-scope.py`의 `BUILD_PATHS`에 `canon/skills/`, `bun.lock`, `package.json`을 더한다. 릴리스 브랜치(`release/`), 수동 실행, 목록을 구하지 못한 경우는 지금처럼 돈다.
- I15 ==**릴리스 검사기가 바뀐다.**== 지금은 두 워크플로의 `build` 행렬을 비교하고(행 수, `host`·`target`·`package_dir`·`lg_page`·`alpine_image`) `publish.yml` 행의 `package_name`을 매니페스트와 맞춘다. 바뀐 뒤:
  - 두 행렬의 비교와 `package_name` 열이 사라진다. 행렬은 `build-native.yml` 한 곳이고, 플랫폼 패키지의 `package.json` `name`이 이름의 유일한 출처다(게시 단계의 `PACKAGE`, `optionalDependencies`, 사전 점검의 목록과 맞춘다). 행 규칙, 바닥 검사 단계, jemalloc, alpine, 대시보드 내장, `MACOSX_DEPLOYMENT_TARGET`, 러너 라벨 규칙은 `build-native.yml`에만 건다. 패키지 집합, 런처, 스모크 규칙이 읽는 행은 `build-native.yml`의 행이다.
  - 새 규칙은 `publish.yml`이 컴파일하지 않는 것(I2), 두 워크플로의 산출물 계약(`publish.yml`이 내려받는 이름이 `build-native.yml`이 올리는 이름에 있고 행마다 `canon-binary-<target>`, `man-pages`, `build-manifest-` 접두사가 빠지지 않는다), `fetch-build`의 모양(`actions: read`, 내려받기 단계마다 `run-id`와 `github-token`, `digest-mismatch` 값, 모든 타깃의 바닥 검사에 `if:`가 없고 `--page-size skip`이 aarch64 musl 행에만 있다, `package`와 스모크가 `fetch-build`를 기다린다), `smoke-binary`의 바닥 검사에 `skip`이 없는 것, 모든 `upload-artifact`의 `retention-days`, `manifest` 잡의 모양(`build`와 `man`을 기다린다, `build-manifest-` 이름으로 올린다, 증명 권한이 이 잡에만 있다)이다.
  - 스모크는 `publish.yml`에만 있고 `build-native.yml`에 없다는 규칙과 `scope` 규칙은 그대로다. `manifest` 잡도 `scope` 규칙을 받는다.
- I16 새로 쓰거나 고치는 시험은 판정만 단언한다. 검사기가 입력을 통과시키는가 거절하는가, 스크립트가 어떤 `verdict`와 `reason`과 종료 코드를 내는가를 본다. 오류 문구, 문장 조각, 워크플로 소스 글자의 개수나 포함은 단언하지 않는다. 지금 `test-release-workflow-safety.py`에서 `publish.yml`의 `build` 잡이나 행렬을 읽는 시험(`matrix_rows(_job_block(…, "build"))`을 `publish.yml`에 쓰는 곳)은 `build-native.yml`을 읽도록 고치거나, 규칙이 사라졌으면 지운다.
- I17 문서. `publish.yml` 머리 주석에서 시험 실행의 설명("It builds, checks, smokes, packs")과 실패 방지 문단("`build`'s entire matrix")을 고치고, macOS 러너 복구 문단은 지운다(`publish.yml`에 macOS 잡이 없다). 그 자리에 매칭 실행을 얻는 법(`gh workflow run build-native.yml --ref …`)과 다시 돌리는 법(`gh run rerun <실행> --failed`)을 적는다. `build-native.yml` 머리 주석은 이 워크플로가 릴리스 바이너리를 만드는 유일한 곳이라고 적는다. `canon/skills-dev/repo-scaffold/SKILL.md`의 3, 6단계("both … `build-native.yml` and … `publish.yml`'s `build` job", "`package_name` too, in `publish.yml`")를 고치고 설치본(`.agents/skills/repo-scaffold`, `.claude/skills/repo-scaffold`)을 `canon skills install`로 다시 만든다. CHANGELOG에 한 항목을 적는다.
- I18 이 변경은 `build-native.yml`에서 `name:`, 최상위 주석, `safety` 단계 위의 주석("matrix / env drifts from publish.yml's")과 `dashboard` 잡 위의 주석("keeps both workflows doing exactly this")을 한 워크플로 설계에 맞게 고치는 것, 새 `manifest` 잡, 업로드 단계의 `retention-days`, `build`의 `Stage` 단계 이름만 만진다. 캐시 단계와 러너 라벨은 건드리지 않는다.

## Checks

- ==**`just release-check`가 통과한다.**== 이 목록에 `scripts/test-release-build.py`가 든다. 새 규칙마다 통과 시험과 거절 시험이 있고 모두 판정만 단언한다(I16).
- 스크립트 시험은 API 응답을 입력으로 준다(실제 호출 없음). 입력과 판정은 이렇다. 유효한 후보 하나는 통과. 후보가 없으면 `no_run`. 같은 `head_sha`의 성공한 실행이 있고 매니페스트의 트리가 다르면 `tree_mismatch`. 후보가 모두 `expired: true`이면 `expired`. 실행의 `conclusion`이 `failure`이면 `run_not_green`. 포크의 `head_repository`이면 `foreign_repo`. 경로가 `publish.yml`(같은 `man-pages` 이름을 올린다)이면 `wrong_workflow`. 한 타깃이 빠진 매니페스트는 `incomplete`. 받은 바이너리의 한 바이트를 바꾸면 `checksum_mismatch`, `.sha256` 짝만 바꿔도 같다. 새 후보와 옛 후보가 둘 다 유효하면 새것을 고른다. API 호출이 실패하면 종료 코드 2다.
- 검사기의 거절 시험은 이 입력을 쓴다. `publish.yml`에 `cargo build`, `setup-rust`, 대시보드 빌드, `host:` 행, `build` 잡이 되돌아온 모양. `fetch-build`의 내려받기에서 `run-id`가 빠진 모양, `digest-mismatch: ignore`, 바닥 검사 단계가 빠진 모양, 한 타깃만 빠진 모양, `if:`가 붙은 모양, `--page-size skip`이 다른 타깃에 붙은 모양. `package`가 `build`를 기다리는 모양. `retention-days`가 빠진 업로드. `build-native.yml`의 `manifest` 잡이 사라진 모양과 이름이 `build-manifest-`로 시작하지 않는 모양. 두 워크플로의 아티팩트 이름이 어긋난 모양. 실제 `publish.yml`과 `build-native.yml`은 통과한다.
- 범위 스크립트의 시험에 둘을 더한다. `canon/skills/`나 `bun.lock`만 바꾼 PR은 돈다. 문서만 바꾼 PR은 건너뛴다.
- ==**실제 실행 하나가 통과한다.**== 이 변경의 PR 실행(`.github/workflows/**`를 바꾸므로 `scope`가 돌리기로 정한다)이 `build-manifest-<트리>`를 올린다. 같은 커밋에서 `publish.yml`을 시험 실행(`dry_run` 기본값)으로 돌린다. `fetch-build`의 로그가 그 PR 실행을 고른 것을 보이고, `publish.yml`의 로그에 `cargo`와 `rustc` 줄이 없고, 네 타깃의 바닥 검사, 스모크, 팩, 다섯 게시 단계의 `(dry-run)`이 통과한다. `npm view`로 새 버전이 생기지 않았다. 실행 번호와 실행 시간을 0.43 시험 실행(709초)과 나란히 증거에 적는다. 예상은 약 200초다[INFERENCE: `fetch-build` 60~90초를 `package` 잡의 37~39초에서 어림했다].
- ==**실제 거절 하나가 막는다.**== 이 변경의 머리에서 PR 없는 임시 브랜치를 따고 문서 파일만 바꾸는 커밋을 올린다(`build-native.yml`은 푸시 트리거가 없어 그 트리의 실행이 없다. 이 변경의 PR 브랜치에 올리면 PR이 `.github/workflows/`를 바꿔 `scope`가 다시 빌드하므로 `no_run`이 나오지 않는다). 그 브랜치에서 `publish.yml`을 시험 실행으로 돌리면 `fetch-build`가 `no_run`으로 실패하고 아무것도 내려받지 않는다. 처방 명령(`gh workflow run build-native.yml --ref <임시 브랜치>`) 뒤 `gh run rerun <실행> --failed`로 통과한다. 끝나면 임시 브랜치를 지운다. 이 확인이 "문서만 바꾼 PR 다음에 릴리스" 경로를 닫는다.
- `expired`는 실제로 만들 수 없다(14일이 지나야 하고 삭제는 `no_run`으로 읽힌다). 스크립트 시험의 입력이 그 판정을 덮는다. `checksum_mismatch`도 실제로 만들지 않고 시험이 덮는다.
- 같은 커밋에서 `just lint`와 `canon gate check`가 통과한다. 새 evidence는 없다.
- 0.44 릴리스의 태그 실행에서 `fetch-build`가 컴파일 없이 통과하고 실행 시간이 약 250초이다(0.43 태그 실행은 1,015초). 이 값은 릴리스 뒤 CHANGELOG나 증거에 한 줄로 남긴다.

## Out of scope

- main 푸시에서 `build-native.yml`을 돌리는 일. 돌려도 태그가 밀려서(0.43에서 main 푸시 8초 뒤에 태그가 붙었다) 그 실행이 끝나기 전에 `publish.yml`이 찾는다. 릴리스 PR 실행이 이미 그 트리를 빌드한다(owner 결정).
- `find`가 돌고 있는 실행을 기다리는 일, `publish.yml`이 `build-native.yml`을 스스로 띄우는 일, 실행 번호를 손으로 지정하는 입력. 거절하고 처방을 말하는 것으로 끝낸다.
- 브랜치 보호와 "병합 전에 최신" 설정(owner 결정).
- `build-native.yml`의 캐시 정책과 러너, 시간 제한. 같은 0.44의 CI 전체 변경이 고친다.
- 같은 트리를 두 번 빌드해 바이트가 같은지 보는 재현 가능 빌드 검증. 이 변경은 빌드를 한 번만 하므로 필요하지 않다.
- 스모크의 내용, 팩의 순서, 게시 단계와 사전 점검. `package`, 스모크, 게시는 받은 파일 이름이 같아 그대로다.
- 증명으로 man pages를 따로 서명하는 일. 매니페스트의 증명이 man 파일의 sha256까지 덮는다.

## Questions and assumptions

- 결정(설계): 키는 "빌드한 트리"다. 의뢰서의 "head 커밋의 트리"에서 한 걸음 더 간다. PR 실행의 `head_sha`는 PR head이고 빌드한 것은 병합 ref라서 기준이 앞서가면 둘이 다르다. 병합 커밋은 병합 뒤 읽을 수 없다(Intent). 실행 안에서 `HEAD^{tree}`를 매니페스트로 적는 것만이 확인을 보장한다. 트리 전체의 같음은 그대로 요구한다(경로를 골라 비교하는 완화는 하지 않는다: 빌드 입력을 빠뜨리면 다른 소스의 바이너리를 게시한다).
- 결정(설계): 아티팩트 API를 이름으로 부르는 `find`다. 실행 목록을 훑는 안은 실행 100개 밖의 후보를 놓치고 후보마다 매니페스트를 내려받아야 한다. 이름이 트리를 담으면 한 번의 호출이 후보를 주고 `expired`, `workflow_run.id`, `head_repository_id`를 같이 준다. 그 응답에서 `man-pages`처럼 publish 실행도 올리는 이름은 경로 확인으로 거른다.
- 결정(설계): 다시 올리는 한 곳이다. 뒤 잡마다 다른 실행에서 받는 안은 `actions: read`를 잡마다 주고 확인을 되풀이한다. `fetch-build`가 유일하게 다른 실행을 건드리고 뒤 잡은 이 실행의 아티팩트만 읽는다. 비용은 126MB를 한 번 더 올리는 약 15초다[INFERENCE].
- 결정(설계): 바닥 검사를 `fetch-build` 한 곳에서 네 타깃 모두 돌리되 aarch64 musl의 페이지 크기만 arm 러너의 `smoke-binary`가 읽는다. macOS 러너를 다시 부르지 않고 실행해야 읽을 수 있는 값을 읽을 수 있는 호스트에서 읽는다.
- 결정(측정): 릴리스당 아낌은 러너 약 80분, 시험 실행 벽시계 약 8.5분(709초에서 약 200초), 태그 실행 약 12.8분(1,015초에서 약 250초)이다[INFERENCE: `fetch-build`를 90초로 잡았다]. 지난 30일에 태그 21개, `publish.yml` 실행 36개(실패 8개)였다. 빌드 계열 한 번이 33~50분이므로 달에 1,400분쯤이 `publish.yml`에서 사라진다[INFERENCE: 실행마다 0.43의 두 값 사이로 잡았다].
- 결정(owner, 2026-11-18): 바이너리, man, 매니페스트의 `retention-days`는 ==**14일**==로 정했다(질문 1의 기본) (`dashboard-app`은 1일, publish의 사본은 7일, `npm-tarballs`는 30일). 이유: 릴리스 PR에서 태그까지 0.43은 하루, 0.42는 1시간이었다. 만료는 `gh workflow run` 한 번으로 풀리는 실패다. 지금 90일 보존이 10.4GB를 쌓았다(저장소는 공개라 청구는 없다[INFERENCE]). 대안은 30일(릴리스 PR이 오래 열려 있어도 안전)이다.
- 결정(owner, 2026-11-18): `manifest` 잡이 매니페스트를 증명하고 `fetch-build`가 확인한다(질문 2의 기본: ==**한다**==). 이유: 체크섬 짝은 같은 아티팩트에 실려 전송 손상만 막는다. 증명은 바이너리의 sha256과 트리가 이 저장소의 `build-native.yml` 실행에서 왔다는 것을 서명으로 묶고, npm 증명이 말하지 않는 부분을 잇는다. 비용은 `id-token: write`와 `attestations: write`가 한 잡에 더해지는 것, 새 액션 핀 하나, 확인의 `gh attestation verify` 몇 초다. 아니라고 하면 I4의 증명 단계, I9의 증명 확인, `attestation_invalid`가 빠지고 신뢰는 `find`의 확인(저장소, 경로, 성공)에 남는다. 나머지 설계는 그대로다.
- 결정(owner, 2026-11-18): main 푸시에서 `build-native.yml`을 ==**돌리지 않는다.**==(질문 3의 기본) 이유: 지난 30일에 main 푸시 39번(릴리스 병합 23번)이고 한 번에 33~50분이라 한 달에 1,300~1,950분이 든다. 아끼는 양과 같다. 릴리스 병합 커밋의 트리는 20개 태그 모두 PR head와 같았다. 태그는 병합 8초 뒤에 붙으므로 푸시 실행이 끝나기 전에 `publish.yml`이 찾는다. 푸시 빌드의 부수 효과는 main에 캐시가 생겨 PR 빌드가 따뜻하게 시작하는 것이다. 그것은 캐시 정책으로 따로 정한다.
- 결정(owner, 2026-11-18): main의 브랜치 보호에 "병합 전에 최신"을 ==**두지 않는다.**==(질문 4의 기본) 이유: 기준이 앞서가 병합 트리가 head와 달라지면 `tree_mismatch`로 거절되고 처방이 한 줄이다. 보호가 없는 main에서 그 설정을 걸면 직접 푸시하는 릴리스 흐름(`test.yml`의 주석)이 막힌다. 대안은 설정을 걸어 거절 자체를 없애는 것이다.
- 가정: `ubuntu-26.04`의 `gh`(2.103.3)와 `python3`(3.14.4)가 `find`, `verify`, `gh attestation verify`에 충분하다[INFERENCE: 러너 이미지 README]. `gh attestation verify`의 JSON에서 증명이 가리키는 실행 번호를 읽는 필드는 구현이 확인한다.
- 가정: `actions: read`의 `GITHUB_TOKEN`으로 같은 저장소의 다른 실행의 아티팩트를 `run-id`로 받을 수 있다(`actions/download-artifact`의 설명: `github-token`은 다른 실행의 아티팩트에 필요하다). 처음 실행에서 안 되면 구현자는 오류를 보고하고 멈춘다. 토큰을 넓히는 말없는 대체는 하지 않는다.
- 가정: 0.44의 CI 전체를 다루는 다른 변경이 `build-native.yml`의 캐시 단계를 고치면 이 변경과 줄이 겹치지 않는다(I18). 겹치면 뒤에 오는 쪽이 리베이스한다. `scripts/check-release-workflow-safety.py`와 시험은 둘 다 고치므로 병합 순서는 오케스트레이터가 정한다.
- 가정: 구현은 커밋 둘이다. 첫째는 `build-native.yml`(`manifest`, 보존, 이름, 범위)과 `scripts/release-build.py`와 시험이고, 둘째는 `publish.yml`과 검사기, 그 시험, 문서다. 첫째만으로도 검사기는 통과한다(`manifest` 잡은 `scope` 규칙만 받는다). 되돌림은 둘째를 되돌리면 `publish.yml`이 옛 모양으로 돌아와 다시 빌드한다.

Approved: journey 2026-11-18T07:53:29Z — "오너가 채팅에서 '통합하자.. 최적화좀 하자 CI 좀'으로 이 변경을 요청하고 질문 4개를 ask로 답함(2026-11-18). 승인은 0.42/0.43처럼 오케스트레이터에게 위임. 독립 계획 리뷰 1라운드 approve(P0/P1 없음), P2/P3 반영. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 93bd33395620d15d38198ac8e92c5749c2369b57) (brief sha256:b5b91edf89e4f932aff510db5fa5926a9ea974fc375181749f250ae4b5c81a69)
