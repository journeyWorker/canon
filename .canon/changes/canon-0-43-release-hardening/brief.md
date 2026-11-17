---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "네 구멍마다 실행 번호와 잰 값이 근거이고, 빼면 구현자가 다시 재야 한다"
  - rule: "prose.length"
    section: "intent"
    reason: "구멍 넷의 근거(실행, 주석, 파일 수, 대기 시간 표)가 모두 필요하다"
  - rule: "prose.length"
    section: "invariants"
    reason: "구멍 하나에 불변이 둘씩 있고 검사기 규칙의 입력 모양을 열거해야 한다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변마다 확인과 거절 시험의 입력을 열거해야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "워크플로 파일, 작업 이름, 명령과 환경변수는 이 변경이 바꾸는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "워크플로 파일, 환경변수, 경로 목록, 명령 형태는 검사기가 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "거절 시험의 입력은 명령과 대입의 철자 그대로여야 한다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 실행 번호와 값의 출처다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 경로 목록, 옵션과 명령 형태의 예다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 시험이 읽는 입력의 예다"
  - rule: "prose.contrast"
    section: "intent"
    reason: "바이너리와 워크플로는 서로 다른 대상이고 그 구분이 변경의 종류(chore)를 정한다"
---

# 릴리스 하드닝: 큰 릴리스 PR에서도 네이티브 빌드가 돌고, 시험 실행이 게시 명령을 그대로 실행하고, 검사기의 빈틈을 막는다

Source: .github/workflows/build-native.yml, .github/workflows/publish.yml, .github/workflows/pr-guard.yml, scripts/check-release-workflow-safety.py, scripts/test-release-workflow-safety.py, 실행 43889537213 · 43903307313 · 43894868331 · 43887186622 · 43882184784(`gh run view`, 작업 주석 `gh api …/check-runs/<id>/annotations`), 풀 리퀘스트 150(`gh pr view`), GitHub 문서 "Workflow syntax"의 "Git diff comparisons"

## Intent

0.42.0 릴리스에서 릴리스 워크플로와 그 검사기의 구멍 넷이 드러났다. canon의 바이너리는 관계가 없다.

- ==**릴리스 PR에서 네이티브 빌드가 돌지 않았다.**== `build-native.yml`은 풀 리퀘스트에서 경로 필터(`paths:`)로만 돈다. 릴리스 PR 150은 4,043개 파일을 바꿨고 이 워크플로는 돌지 않았다. GitHub 문서의 한도는 3,000개다(의뢰서의 300개는 추정이었다). 변경이 3,000개를 넘고 필터에 맞는 파일이 처음 3,000개 안에 없으면 워크플로는 돌지 않는다. 이 PR에서 맞는 첫 파일은 경로 순으로 3,638번째(`.cargo/config.toml`)였고, 앞은 `.canon/archive` 2,358개와 `.canon/ledger` 1,261개 등이었다[INFERENCE: GitHub의 정렬이 경로 순이라고 본다]. `test.yml`은 필터에 `.canon/**`가 있어 돌았다.
- 최근 병합된 풀 리퀘스트 40개 중 21개가 이 워크플로를 건너뛰었다[INFERENCE: 최근 실행 100개의 head 브랜치 이름과 맞춰 셌다]. 13개는 파일 1~32개의 문서·chore PR이라 건너뛰는 것이 맞다. 8개는 파일 3,705~11,322개의 큰 PR이다(릴리스 PR 0.33, 0.35, 0.36, 0.37, 0.39, 0.40, 0.42와 `feat/0.30`). 0.42에서는 릴리스 브랜치에서 손으로 돌린 실행(43882184784)이 aarch64 musl 행의 jemalloc 빌드 실패를 태그 전에 잡았다.
- ==**macOS arm64 잡이 러너를 못 얻어 두 번 취소됐다.**== 실행 43889537213과 43903307313에서 `Build - aarch64-apple-darwin`이 만들어진 지 15분(901초) 뒤 취소됐다. 작업 주석은 `The job was not acquired by Runner of type hosted even after multiple attempts`와 macOS arm64의 용량 제약 안내다. 단계는 하나도 시작하지 않았고 `runner_name`은 비어 있다. 잡의 `timeout-minutes: 50`은 러너가 잡을 시작한 때부터 센다[INFERENCE: 문서가 아니라 이 취소에서 읽었다].
- 같은 실행의 다른 세 행은 2~4초에 러너를 얻었다. 실패한 잡만 다시 돌리자(시도 2) 두 실행 모두 5초와 2초 만에 러너를 얻어 10분 37초와 13분 19초에 끝났다. 취소된 시도에서 스모크, 팩, 게시 잡은 모두 `skipped`였고 아무것도 게시되지 않았다. 첫 시도의 기록은 이렇다. `not acquired` 주석이 있는 취소만 센다(`macos-14`의 취소 둘은 이미지 퇴역 시험 정전, `macos-15`의 하나는 새 푸시의 동시성 취소였다).

| 러너 라벨 | 첫 시도 수 | 러너를 못 얻어 취소 | 300초 이상 대기 |
| --- | --- | --- | --- |
| `macos-26` | 6 | 2 | 1(603초) |
| `macos-15` | 26 | 0 | 2(303초, 308초) |
| `macos-14` | 16 | 0 | 2(303초, 304초) |

  대기가 300초의 배수(303, 304, 308, 603)로 몰리고 취소가 901초라서 플랫폼이 5분마다 세 번 시도하고 포기하는 모양이다[INFERENCE: 문서화되지 않은 동작이다]. 표본이 작고 `macos-26`이 가장 새 이미지다.
- ==**검사기에 통과하는 빈틈이 있다.**== 변형 입력으로 검사기를 돌려 확인했다. 게시 전 점검은 `npm whoami`를 `if [ -n "$ABSENT" ]` 안에서만 허용한다. 같은 단계에 보호 없는 `npm --loglevel=error whoami`, `env FOO=1 npm whoami`, `FOO=1 npm whoami`, `command npm whoami`, `npm --registry <주소> whoami`를 더해도 통과한다. 호출을 세는 정규식이 `npm whoami`가 줄 앞이나 구분자 뒤에 있기를 요구하기 때문이다. `TARBALL`의 `./` 검사는 큰따옴표 대입만 읽는다. `TARBALL='tarballs/x.tgz'`, 따옴표 없는 대입, `export TARBALL=…`, `TARBALL=$(…)`는 통과한다.
- ==**시험 실행은 실패한 그 명령을 돌리지 않는다.**== v0.42.0 태그 실행(43894868331)은 첫 게시에서 멈췄다. npm이 `npm publish tarballs/<파일>.tgz`를 GitHub 저장소 약식 표기로 읽어 `git ls-remote ssh://git@github.com/tarballs/….tgz.git`를 불렀고(종료 128) 아무것도 게시되지 않았다. 시험 실행(`dry_run`)은 다섯 `npm publish` 단계를 통째로 건너뛰어서 이 결함을 볼 수 없었다. 이 계획에서 npm 11.20.0으로 확인했다. `npm publish --dry-run`은 레지스트리에 쓰지 않는다. `npm_config_dry_run=true`도 같은 효과다. 약식 경로는 시험 실행에서도 같은 `ls-remote` 오류로 거절되고 `./tarballs/x.tgz`는 통과한다.

## Category

ci

## Invariants

- I1 ==**canon의 바이너리, 명령, 출력, 레저는 바뀌지 않는다.**== 바뀌는 것은 `build-native.yml`, `publish.yml`, 릴리스 검사기와 그 시험, 범위 스크립트 하나, CHANGELOG다. 소스 코드, `test.yml`, `pr-guard.yml`은 그대로다.
- I2 `build-native.yml`의 `pull_request` 트리거에 `paths`와 `paths-ignore`가 없다. 첫 잡(`scope`)이 빌드를 돌릴지 정하고 빌드 계열의 모든 잡이 그 결과를 기다린다. `scope`는 `timeout-minutes: 5`다.
- I3 `scope`는 수동 실행에서 돌린다. head 브랜치가 `release/`로 시작하는 풀 리퀘스트도 돌린다. 그 밖의 풀 리퀘스트는 지금 필터의 경로(`crates/**`, `packages/dashboard/**`, `.cargo/**`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.github/workflows/**`, `.github/actions/**`, `scripts/**`)에 맞는 파일이 하나라도 있으면 돌리고 없으면 건너뛴다. 건너뛴 실행은 빌드 잡이 모두 `skipped`이고 워크플로는 초록이다.
- I4 변경 파일 목록은 `git diff --no-renames --name-only`로 구한다. 이름 바꿈은 옛 경로와 새 경로를 모두 싣는다(기본 `--name-only`는 새 경로만 싣는다). 풀 리퀘스트 파일 API도 3,000개에서 멈춘다[INFERENCE]. 목록을 구하지 못하면 건너뛰지 않고 돈다. 돌 때의 빌드 행, 환경변수, 아티팩트는 지금과 같다.
- I5 빌드 계열 잡 `dashboard`, `man`, `build`는 `scope`를 기다리고 `scope`의 출력이 명시적으로 `skip`일 때만 건너뛴다(`if: needs.scope.outputs.skip != 'true'` 꼴). 출력이 없거나 이름이 틀리면 빌드가 돈다. `scope` 잡 자체가 실패하면 실행은 빨갛다. `safety`는 `scope`를 기다리지 않는다.
- I5b 릴리스 검사기는 `build-native.yml`의 `pull_request`에 `paths`나 `paths-ignore`가 있는 것을 거절한다. `scope` 잡이 없는 것, `scope`를 기다리지 않는 빌드 계열 잡, 양의 값(`== 'true'`, `== 'run'` 꼴)이 있어야 도는 `if:`도 거절한다. 두 워크플로의 행이 같아야 한다는 규칙과 스모크가 `publish.yml`에만 있어야 한다는 규칙은 그대로다.
- I6 macOS arm64 행의 러너 라벨(`macos-26`), 시간 제한, 행 구성은 바뀌지 않는다. 자동 재시도 워크플로도 더하지 않는다.
- I7 `publish.yml`의 머리 주석에 복구 절차를 적는다. 징후는 잡이 `cancelled`이고 단계가 없고 주석이 `not acquired by Runner`인 것이다. 처방은 `gh run rerun <실행 번호> --failed`다. 취소된 잡과 시작도 못 한 뒤 잡(스모크, 팩, 게시)만 다시 돌고 다른 행의 아티팩트는 그대로 쓴다. 태그를 다시 달지 않는다.
- I8 그 주석은 두 가지를 덧붙인다. 태그 실행을 다시 돌리면 진짜 게시까지 가는데, 게시 잡이 한 번도 시작하지 않았으므로 부분 게시가 없고 게시 전 점검이 처음처럼 돈다. `timeout-minutes`는 대기에 걸리지 않으므로 늘려도 소용없다.
- I9 시험 실행(`dry_run`)은 다섯 `npm publish` 단계를 건너뛰지 않고 같은 명령으로 돌린다. 게시 단계마다 환경변수 `NPM_CONFIG_DRY_RUN`을 둔다. 값은 태그 푸시에서 `false`, `dry_run=false`인 수동 실행에서 `false`, `dry_run=true`인 수동 실행에서 `true`다. 명령 줄은 진짜 릴리스와 글자까지 같다.
- I10 환경변수는 다섯 게시 단계에만 둔다. 잡이나 워크플로에 두면 같은 잡의 `npm install -g npm@…`가 시험 실행에서 아무것도 설치하지 않는다(이 계획에서 확인했다). 게시 단계에는 `dry_run`을 읽는 `if:`가 없다. 시험 실행과 `recovery=true`의 조합은 지금처럼 일찍 실패한다. 게시 전 점검은 지금처럼 시험 실행에서도 돈다.
- I11 검사기는 게시 단계마다 환경변수의 식을 지금의 조건 읽기로 세 경우에 평가한다. 나온 문자열이 태그 푸시 `false`, `dry_run=true` `true`, `dry_run=false` `false`가 아니면 거절한다. 문자열 `false`는 참으로 읽히므로 값을 비교한다.
- I12 검사기는 게시 단계의 `if:`가 `dry_run`을 읽는 것을 거절한다(건너뛰면 명령이 시험되지 않는다). 명령에 `--dry-run`이 직접 적힌 것도 거절한다(진짜 릴리스도 아무것도 게시하지 않는다). 사전 릴리스 규칙과 패키지 집합 규칙이 읽는 `npm publish` 줄의 모양은 그대로다.
- I12b 검사기는 시험 실행 설정이 게시 단계 밖에 있는 것을 거절한다. 설정은 대소문자와 상관없이 `npm_config_dry_run`이나 `npm_config_dry-run`이다(npm은 `npm_config_*`를 대소문자 없이 읽는다). 길은 잡, 워크플로, 다른 단계의 `env:`, `$GITHUB_ENV` 쓰기, `npm config set dry-run`이다.
- I13 검사기는 `npm whoami`를 호출의 모양과 상관없이 센다. 따옴표 안의 글은 지우고(`$( )`와 역따옴표 안은 남긴다) `\` 이어쓰기는 합친다. 같은 단순 명령에서 따옴표 없는 `npm` 낱말 다음에 `whoami` 낱말이 오면 호출이다. 앞의 `FOO=1`, `env`, `command`, `timeout 30`, `time`, `exec`와 `npm` 뒤의 옵션(`--loglevel=error`, `--registry <주소>`)은 판정을 바꾸지 않는다. 보호(`if [ -n "$ABSENT" ]` 안) 없는 호출이 하나라도 있으면 거절한다.
- I14 검사기는 `npm publish "$TARBALL"`을 돌리는 게시 단계마다 그 단계의 `run` 스크립트에서 `TARBALL`에 읽을 수 있는 값을 대입하는지 본다. 큰따옴표, 작은따옴표, 따옴표 없음, `export`·`declare`(옵션 포함)·`readonly`·`local`이 앞선 형태가 같은 판정을 받는다. 따옴표를 벗긴 값이 `./`, `../`, `/`로 시작하지 않으면 거절하고, 값이 명령 치환뿐이어서 앞부분을 읽을 수 없는 대입도 거절한다. 다른 출처는 모두 거절한다: 단계의 YAML `env:`, `$GITHUB_ENV`, `for`·`read`·`printf -v`.
- I15 CHANGELOG에 세 가지를 적는다. 큰 릴리스 PR에서도 네이티브 빌드가 돈다. 시험 실행이 게시 명령을 `--dry-run`으로 그대로 돌린다. macOS 러너를 못 얻었을 때의 복구 절차가 있다.
- I16 병합 순서는 atip → `canon-0-43-test-cleanup` → 이 변경이다. 이 변경은 `test.yml`과 `pr-guard.yml`을 만지지 않는다.
- I17 이 변경이 만지는 `build-native.yml`의 곳은 셋이다. `pull_request:`와 `paths:` 블록(28~38줄), 첫 잡 `scope`의 추가(`safety` 앞, 55줄 위), `needs:`와 `if:` 세 곳(`dashboard` 74줄, `man` 104줄, `build` 136줄).
- I17b `publish.yml`의 곳은 머리 주석(23~26줄 부근), 게시 단계 앞 주석(701줄), 게시 단계 다섯이다. 단계마다 `if:`(703, 721, 739, 757, 775줄)를 `env:`로 바꾼다. 701줄의 주석은 "시험 실행은 이 다섯 단계만 건너뛴다"라서 거짓이 된다. 시험 실행이 같은 명령을 `NPM_CONFIG_DRY_RUN=true`로 돌린다고 고친다.
- I18 test-cleanup이 먼저 병합된다. 그 변경은 `test.yml`의 `grammar-mutation` 작업(380줄부터)과 세 호출자 단계(191줄부터)를 고치고, 두 릴리스 워크플로는 만지지 않는다. 검사기는 `grammar-mutation`을 이름으로 읽지 않고 잡마다 `timeout-minutes`를 본다. 그 작업의 삭제에 영향받지 않는다.
- I18b test-cleanup은 `scripts/check-release-workflow-safety.py`와 `scripts/test-release-workflow-safety.py`도 고친다(오류 문구 단언을 판정으로, `recovery=true and dry_run=false` 글자를 요구하는 규칙과 그 시험의 삭제). 이 변경은 두 파일을 같은 줄 단위로 만지므로 그 위에서 다시 맞춘다.
- I19 새로 쓰거나 고치는 검사기 시험은 판정만 단언한다. 검사기가 입력을 통과시키는가 거절하는가를 보고, 오류 문구, 메시지 글자, 문장 조각, 워크플로 소스 글자의 개수나 포함은 단언하지 않는다. 이 변경이 고치는 규칙(I5b, I11~I14)의 기존 시험이 그런 단언을 하면 판정으로 고치거나 지운다. `test-release-workflow-safety.py`의 `job.count("npm whoami") == 3`(825줄)은 지운다. 바로 다음 줄의 shipped 워크플로 통과 판정이 같은 계약을 덮는다.

## Checks

- ==**`just release-check`가 통과한다.**== I5b, I11~I14의 규칙마다 통과 시험과 거절 시험이 있고 모두 판정만 단언한다(I19).
- 거절 시험은 이 계획이 확인한 빈틈을 입력으로 쓴다. 보호 없는 `npm --loglevel=error whoami`, `env FOO=1 npm whoami`, `command npm whoami`, `timeout 30 npm whoami`, `\`로 이어 쓴 호출. `TARBALL='tarballs/x.tgz'`, 따옴표 없는 대입, `export TARBALL=…`, 단계의 YAML `env: TARBALL`. 게시 단계의 `if:`로 되돌아간 모양, `--dry-run`이 박힌 모양, 잡 수준의 `NPM_CONFIG_DRY_RUN`과 소문자 `npm_config_dry_run`, `$GITHUB_ENV` 쓰기. `paths:`가 되돌아온 `build-native.yml`, 양의 값이 있어야 도는 빌드 `if:`.
- 지금 게시 단계의 `if:` 모양에 묶인 시험은 I9의 계약으로 고쳐 쓴다. 계약은 태그 푸시가 진짜, `dry_run=true`가 시험, `dry_run=false`가 진짜다. 옛 조건 글자에만 묶인 시험은 지운다. 이 규칙들의 시험에서 `any("… in error")`처럼 문구를 찾는 단언이 하나도 남지 않는다.
- 범위 스크립트의 시험은 `just release-check`에 든다. 변경 4,000개 중 필터에 맞는 파일이 맨 끝 하나뿐이면 돈다(이 결함의 모양). `scripts/x`를 `tools/x`로 옮기는 이름 바꿈은 돈다. 문서 파일만 바꾸면 건너뛴다.
- 범위 스크립트의 시험은 두 경우도 본다. head가 `release/0.43.0`이면 파일과 상관없이 돈다. 변경 목록을 구하다 실패하면 돈다.
- 이 변경의 PR 실행(`.github/workflows/**`를 바꾸므로 `scope`가 돌리기로 정한다)에서 `scope`의 결정이 보이고 네 빌드 행이 건너뛰지 않고 실행돼 초록이다. 실행 번호를 증거에 적는다. 0.43 릴리스 PR(head `release/0.43.0`)에서도 같은 확인을 하고, 그 PR이 `release/` 규칙의 첫 실제 확인이다.
- 0.43 버전 올리기 커밋에서 `publish.yml`을 시험 실행(`dry_run` 기본값)으로 돌린다. 다섯 게시 단계의 로그에 `Publishing to https://registry.npmjs.org/ with tag latest and public access (dry-run)`와 `+ <패키지>@0.43.0`이 있다. `npm view`로 0.43.0이 새로 생기지 않았다. 이 로그가 `--provenance`가 시험 실행을 깨지 않는다는 확인이다.
- `just lint`와 `canon gate check`가 같은 커밋에서 통과한다. Rust와 시나리오가 묶인 코드는 바뀌지 않으므로 새 evidence는 없고 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Out of scope

- macOS 행을 `macos-15`나 유료 대형 러너로 옮기는 일과 러너를 못 얻었을 때 자동으로 다시 돌리는 워크플로. 복구 절차만 적고, 옮김은 owner 결정의 조건(Questions)이 오면 한다.
- 러너 대기 시간 자체를 줄이는 일(15분 포기는 GitHub의 동작이고 설정이 없다[INFERENCE])과 `timeout-minutes` 값 변경.
- `test.yml`의 경로 필터. 같은 3,000개 한도를 갖지만 필터에 `.canon/**`가 있어 릴리스 PR에서 돈다.
- 시험 실행이 보증하지 못하는 것: 토큰의 쓰기 권한과 2FA 우회, 신뢰 게시(OIDC) 설정, 출처 증명의 서명, 이미 있는 버전과의 충돌. `npm publish --dry-run`은 레지스트리에 쓰지 않으므로 이것들은 첫 진짜 게시에서 드러난다. 게시 전 점검의 `npm whoami`가 토큰의 유효만 본다.
- 검사기를 속이려는 쓰기(`bash -c`, `eval`, 변수로 조립한 명령). 검사기는 실수를 막는 가드이고 적대적 입력은 막지 않는다.
- 이 변경이 고치지 않는 검사기 규칙의 시험에 남은 문구 단언. test-cleanup이 오류 문구 단언을 고치고 `recovery=true and dry_run=false`를 요구하는 규칙과 시험을 지운다. 그 뒤에 남는 것은 시험 감사의 다음 라운드 후속 목록에 둔다.

## Questions and assumptions

- 결정: 변경 종류는 chore다. 릴리스 워크플로와 검사기만 바뀌고 canon의 명령, 출력, 레저는 바뀌지 않는다.
- 결정: 주제는 `atip-contract`다. 같은 파일을 바꾼 0.42 릴리스 chore(`canon-0-42-release-portability`)가 여기에 adopt됐다.
- 결정(owner, 2026-11-17): macOS 행은 `macos-26`을 그대로 두고 복구 절차를 적는다(I6~I8). 0.43 릴리스의 실행(수동 실행과 태그 실행)에서 `macos-26`이 한 번이라도 러너를 또 못 얻으면 행을 `macos-15`로 옮긴다. 근거: 표본이 작고(`macos-26` 6번 중 2번) 둘 다 재실행 한 번으로 풀렸으며 아무것도 게시되지 않았다. `macos-15`는 26번 중 0번 취소였다. 바이너리의 바닥은 배포 타깃 15.0이 정하므로 이미지가 바뀌어도 `minos`는 같다[INFERENCE: 0.41은 `macos-15`에서 SDK 15.5로 빌드했다]. 옮기는 변경은 행 라벨 두 곳과 검사기의 허용 목록만 바꾼다.
- 결정(owner, 2026-11-17): 빌드 범위는 `scope` 잡이다(I2~I4). 필터만 없애는 안은 문서·chore PR(최근 40개 중 13개)까지 macOS를 포함한 네 빌드를 돌려 용량이 부족한 macOS 풀에 부담을 더하므로 고르지 않았다. `release/` 접두사 규칙은 릴리스 브랜치 이름(`release/0.42.0`)에 기대고, 파일 규칙이 어긋나도 릴리스 PR이 도는 두 번째 안전망이다.
- 결정(owner, 2026-11-17): 시험 실행은 환경변수 하나로 같은 명령을 시험 실행으로 바꾼다(I9). 시험 실행용 단계를 따로 두면 명령이 두 벌이 되어 어긋날 수 있다. 환경변수가 잘못 `true`가 되면 진짜 릴리스가 초록인 채 아무것도 게시하지 않으므로 I11이 세 경우를 평가한다.
- 결정(owner, 2026-11-17): 검사기 시험은 판정만 단언한다(I19). owner의 규칙은 문구를 바이트 수준으로 고정하는 시험을 만들지 않는 것이다.
- 가정: CI의 npm 11.5.1에서도 `NPM_CONFIG_DRY_RUN`과 `--dry-run`이 로컬 11.20.0과 같게 동작하고, 시험 실행의 `--provenance`가 Actions에서 서명을 시도하지 않는다[INFERENCE: 로컬에서만 확인했다]. 첫 시험 실행(0.43 올리기 커밋)에서 깨지면 구현자는 발견한 오류를 보고하고 멈춘다. `--provenance`만 빼는 식의 말없는 대체는 하지 않는다.
- 가정: 범위 스크립트가 변경 목록을 얻으려면 기준 브랜치의 이력이 필요하다. 얻는 방법(체크아웃 깊이, 병합 커밋의 첫 부모와의 비교)은 구현이 정하고, 못 얻으면 I4에 따라 돈다.
- 가정: 변경 파일 목록의 경로 목록은 범위 스크립트 한 곳에만 둔다.

Approved: journey 2026-11-17T09:49:36Z — "오너가 채팅에서 오케스트레이터에게 0.43 승인을 위임함 (2026-11-17 ask: '응'). 독립 계획 리뷰 1라운드 approve(P0/P1 없음), P2/P3 반영. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 0c6fb013d928439f3157a3b1ad6c0ef35fe8d5eb) (brief sha256:6656f57ca4af5dab32cbe4d4ef8c0529be29027b792dafb8045b695afe356b7a)
