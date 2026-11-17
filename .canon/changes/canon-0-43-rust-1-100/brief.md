---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "버전이 적힌 자리, 새 lint의 수, 이미지 다이제스트, 병합 순서가 모두 구현자의 입력이라 빼면 다시 재야 한다"
  - rule: "prose.length"
    section: "intent"
    reason: "버전이 적힌 자리와 1.100의 변화 다섯이 근거다"
  - rule: "prose.length"
    section: "invariants"
    reason: "불변식마다 바뀌는 자리를 열거해야 한다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변식마다 실행할 명령이 하나씩 필요하다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "파일, 잡, lint, 이미지 이름은 이 변경이 바꾸는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "파일, 키, 이미지 이름은 이 변경이 바꾸는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "확인은 명령의 철자를 그대로 말해야 구현자와 리뷰어가 같은 것을 돌린다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 파일 이름과 측정의 출처다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 파일 목록과 명령 형태의 예다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 명령 형태의 예다"
---

# Rust 툴체인을 1.99.1에서 1.100.0으로 올린다: 핀, 선언, CI, Alpine 이미지, 새 lint를 한 PR에서

Source: `rust-toolchain.toml`, `Cargo.toml`, `.github/workflows/test.yml`·`build-native.yml`·`publish.yml`, `scripts/test-release-workflow-safety.py`, `scripts/ci-strip-workspace-build.sh`, `docs/design/rust-coding-rules.md`, `canon-0-42-ci-refresh`의 brief(I4와 Out of scope), Rust 1.100.0 릴리스 노트와 clippy 1.100 변경 기록(2026-11-17에 읽음), `rust-dev-guide`의 LINT-5, Docker Hub의 `rust:1.100.0-alpine3.24`(2026-11-17), main `5c4f46fab`(0.42.0)

## Intent

canon은 Rust 1.99.1로 빌드하고 검사한다. 1.100.0이 stable이 됐고 owner가 Rust 프로젝트 넷의 툴체인을 함께 옮기기로 했다. 집 규칙(LINT-5)은 핀, `rust-version`, CI 이미지를 한 PR에서 같이 올리고 같은 PR에서 lint 목록을 다시 확인하라고 한다. 지금 상태는 다음과 같다.

- ==**버전이 적힌 곳은 여섯 파일이다.**== `rg -n '1\.99'`에서 역사 기록을 뺀 결과다. `rust-toolchain.toml`의 `channel`. `test.yml`의 `clippy` 잡 이름(`clippy 1.99.1 and visibility rule`)과 그 위 주석. `build-native.yml`과 `publish.yml`의 aarch64 musl 행(`alpine_image: rust:1.99.1-alpine3.24@sha256:44819b27…`). `docs/design/rust-coding-rules.md`의 핀 설명. `scripts/test-release-workflow-safety.py`의 시험 입력 셋(같은 이미지 문자열을 그대로 적었다). 나머지 `1.99` 철자는 출시된 CHANGELOG 항목, 같은 문서의 0.28.0 측정 기록, 지난 brief와 레저의 기록이라 바꾸지 않는다.
- ==**`rust-version`이 어디에도 없다.**== 워크스페이스 `Cargo.toml`과 열한 크레이트 모두 선언하지 않는다. LINT-5의 둘째 자리가 비어 있고 핀 하나만 버전을 말한다.
- ==**새 lint는 한 곳이 아니다.**== 1.100의 clippy는 켜 둔 `map_unwrap_or`를 `.map(f).unwrap_or_default()`까지 넓혀 `map_or_default(f)`를 권한다. 0.42 계획은 `canon-model`의 `handoff.rs` 한 곳을 쟀다. 이 트리에서 그 모양을 글자로 세면 한 줄짜리가 137줄(73개 파일), 여러 줄에 걸친 것이 15곳이고 절반쯤이 `tests/`에 있다. 같은 lint가 atip에서는 64곳이었다. 진짜 수는 구현자가 1.100.0의 clippy로 센다. `Option::map_or_default`는 1.98.0부터 stable이라 고친 코드는 1.99.1에서도 컴파일된다.
- ==**lint 밖에서도 1.100이 바꾸는 것이 있다.**== clippy의 새 lint(`option_zip_none`, `needless_nonzero_get`)와 기존 lint의 확장(`unnecessary_map_or`, `unnecessary_fold`, `needless_bool`, `used_underscore_binding`, `redundant_clone`)은 `-D warnings`에서 실패가 된다. cargo는 중간 산출물 디렉터리 배치를 바꿨고 자체 lint 체계(`cargo::unused_dependencies`, `cargo::unused_workspace_package_fields`)를 들였다. 뒤의 것은 `[workspace.package]`의 `repository`(어느 크레이트도 상속하지 않는다)에서 반드시 울린다. CI 캐시를 준비하는 `scripts/ci-strip-workspace-build.sh`는 `cargo clean -p` 뒤에 `target/debug/deps`, `.fingerprint`, `build` 아래의 파일을 이름으로도 지우고 지울 것이 없어도 실패하지 않는다(`|| true`). 이름으로 지우는 반복은 다른 경로의 워크스페이스 사본을 만드는 `grammar-mutation` 잡 때문에 있었고, `canon-0-43-test-cleanup`이 그 잡을 지운다. 배치가 달라지면 아무것도 지우지 않은 채 캐시만 커질 수 있다.
- ==**옛 툴체인의 `target/`가 복원될 길이 남아 있다.**== `test.yml`의 캐시 잡 여섯 곳은 `restore-keys`의 마지막 줄 `canon-test-<잡>-cargo-`로 툴체인 해시가 없던 때의 `target/` 전체를 복원한다. 0.42 변경이 이것을 첫 실행용 다리로 두고, 툴체인을 올리는 변경이 지운다고 적었다.
- ==**aarch64 musl 릴리스 행은 새 이미지 다이제스트가 필요하다.**== 그 행은 `channel`과 같은 rustc가 든 이미지로 빌드하고(행의 빌드 단계가 둘을 비교해 다르면 실패한다) 이미지를 다이제스트로 고정한다. Docker Hub의 `rust:1.100.0-alpine3.24` 인덱스 다이제스트는 `sha256:de1e0058543a81ed7b046629167db587c67a94bdf37cce14eb27f0ec83baaffb`다. 태그가 다시 빌드될 수 있어 구현자가 다시 읽는다[INFERENCE]. 릴리스 안전 검사기는 `rust:<버전>-alpine<버전>@sha256:` 모양만 읽으므로 검사기 코드는 그대로고, 그 시험의 이미지 문자열 넷만 낡는다.
- ==**`clippy` 잡 이름을 필수 검사로 거는 곳이 없다.**== `gh api`로 읽은 `main`의 브랜치 보호와 룰셋이 0개다. 이름을 바꿔도 아무것도 깨지지 않는다.

## Category

ci

## Invariants

- I1 ==**canon의 동작은 바뀌지 않는다.**== 명령, 출력, 종료 값, 기록 형식, 선언(`contractVersion` 1.6)이 같다. 바뀌는 것은 `rust-toolchain.toml`, 워크스페이스와 열한 크레이트의 `Cargo.toml`, `test.yml`, `build-native.yml`, `publish.yml`, `scripts/test-release-workflow-safety.py`, 규칙 문서, `canon/skills-dev/repo-scaffold/SKILL.md`와 그 사본(`.agents/skills/repo-scaffold/`, `.claude/skills/repo-scaffold/`), CHANGELOG, lint 자리를 고친 소스, 이 변경의 `.canon/` 기록(brief, evidence, review)이다. `scripts/ci-strip-workspace-build.sh`는 I5가 요구할 때만 바뀐다. 소스의 변경은 값이 같은 다시 쓰기뿐이다(`.map(f).unwrap_or_default()`를 `.map_or_default(f)`로). 예외는 둘이고 자리마다 증거에 적는다. 하나는 1.100.0에서 컴파일되게 하는 데 호환 변경이 강제하는 `.rs` 수정과 의존성 올림이다. 다른 하나는 `cargo::unused_dependencies`가 가리킨 의존성을 지울 때 `Cargo.lock`이 바뀌는 것이다. 그 밖에 `Cargo.lock`은 같다.
- I2 버전은 하나다. `rust-toolchain.toml`의 `channel`이 `1.100.0`이고, 워크스페이스 `[workspace.package]`가 `rust-version = "1.100.0"`을 선언하고 열한 크레이트가 `rust-version.workspace = true`를 쓴다. 두 워크플로의 aarch64 musl 행이 같은 줄 `rust:1.100.0-alpine3.24@sha256:<새 다이제스트>`를 갖는다(검사기가 두 워크플로의 행이 같다는 것을 이미 강제한다).
- I3 이름에 버전이 없다. `clippy` 잡의 이름이 `clippy and visibility rule`이고 그 위 주석은 버전 대신 "핀이 정한 툴체인"을 말한다. 규칙 문서의 핀 설명도 버전 없이 `rust-toolchain.toml`을 가리킨다. 다음 올림은 `rust-toolchain.toml`, `rust-version`, 이미지 셋만 건드린다.
- I4 lint는 고치고 덮지 않는다. `just lint`의 clippy 세 실행이 1.100.0에서 경고 0이다. 새 `#[allow]`가 없다(`allow_attributes`가 이미 막는다). `#[expect(lint, reason = "…")]`는 다시 쓸 수 없는 자리에만 쓰고 자리마다 이유가 있다.
  켠 lint 목록(`Cargo.toml`의 `[workspace.lints]`, `clippy.toml`, `test.yml`의 `-D` 플래그)은 그대로다. 1.100이 더한 lint 중 목록에 넣을 것이 있는지 본 결과는 증거에 남기고 넣지는 않는다. `cargo check --workspace --all-targets --locked`가 cargo lint 경고를 내지 않고, 나오면 이 PR에서 고친다. 나올 경고는 `repository`다. 기본 고침은 `[workspace.package]`에서 그 줄을 지우는 것이다(크레이트를 crates.io에 게시하지 않는다). 대안은 열한 크레이트에 `repository.workspace = true`를 쓰는 것이다.
- I5 옛 캐시는 복원되지 않는다. `test.yml`의 `restore-keys`는 잡마다 한 줄 `canon-test-<잡>-deps-<툴체인 해시>-`뿐이고 `-cargo-`로 끝나는 줄이 없다. 맨 위 캐시 설명 주석도 다리 이야기를 뺀다. 첫 PR 실행은 캐시를 복원하지 못하는 차가운 실행이고 잡마다 `timeout-minutes` 안에 끝난다.
  1.100.0의 디버그 빌드 뒤에 `scripts/ci-strip-workspace-build.sh`를 돌리면 `$CARGO_TARGET_DIR` 아래에 워크스페이스 크레이트 이름이 붙은 파일이 남지 않는다. `grammar-mutation`이 없어진 뒤에 `cargo clean -p`만으로 1.100 배치에서 남는 것이 없으면 이름으로 지우는 반복과 그 설명 주석을 지운다. `cargo clean -p`가 워크스페이스 산출물을 남길 때만 스크립트를 새 배치에 맞춘다.
- I6 시험이 문구나 버전을 고정하지 않는다. 이 변경은 시험을 더하지 않는다. `scripts/test-release-workflow-safety.py`의 이미지 문자열 넷(359, 362, 366, 370줄)은 워크플로 본문에서 읽은 값으로 바꿔서, 다음 올림이 시험을 건드리지 않는다. CHANGELOG나 워크플로 주석의 글자를 읽는 시험은 없다.
- I7 문서가 새 상태를 말한다. `docs/design/rust-coding-rules.md`의 핀 설명은 버전을 적지 않고 `rust-toolchain.toml`을 가리키며, 새 lint의 전후 수는 날짜가 있는 측정으로 한 줄 적는다(0.28.0 측정 기록은 그대로 둔다). 크레이트 만드는 규칙(`canon/skills-dev/repo-scaffold/SKILL.md`의 셋째 단계)이 `rust-version.workspace = true`를 더하고, 두 사본은 `canon skills install --source canon/skills-dev --target .`로 다시 만든다. `rust-toolchain.toml`의 머리 주석이 올릴 때 바꿀 셋(`channel`, `rust-version`, 이미지)을 말한다.
  CHANGELOG의 `Unreleased`에 `Changed` 항목을 쓴다. 전후 표에 `channel`, `rust-version`, aarch64 이미지와 다이제스트, `clippy` 잡 이름, lint 자리의 수를 싣는다. 한 줄 설명은 소스에서 빌드하는 사람이 1.100.0을 받는다는 것, 검사기 설치는 미리 빌드한 npm 바이너리 그대로라는 것, 캐시 다리를 지웠다는 것이다.

## Checks

- C1(I2) `rg -n --hidden '1\.99' . -g '!.git' -g '!.canon/archive' -g '!.canon/ledger' -g '!.canon/artifacts' -g '!.canon/changes'`가 남기는 파일은 `CHANGELOG.md`(출시된 항목과 이 변경의 전후 표)와 `docs/design/rust-coding-rules.md`의 0.28.0 측정 줄뿐이다.
  `cargo metadata --no-deps --format-version 1 | jq '[.packages[].rust_version] | unique'`가 `["1.100.0"]`이다.
- C2(I2) ==**새 다이제스트를 릴리스 전에 실제로 돌리는 곳은 이 PR의 `build-native.yml` 실행뿐이다.**== 경로 필터에 `rust-toolchain.toml`과 `.github/workflows/**`가 있어 이 PR에서 돈다. 네 행이 초록이고 aarch64 musl 행의 빌드 단계 로그가 이미지 안에서 `rustc 1.100.0`을 인쇄한다. 모든 Rust 잡 로그도 `rustc 1.100.0`을 인쇄한다(`setup-rust` 액션이 핀과 다르면 실패한다). 샌드박스에서 `docker buildx imagetools inspect rust:1.100.0-alpine3.24`의 다이제스트가 핀과 같다.
- C3(I3) `rg -n 'clippy 1\.' .github`가 비어 있다.
- C4(I4) 샌드박스에서, 고치기 전에 `scripts/lint-counts.sh`를 1.100.0으로 돌려 lint별 개수와 파일 목록을 증거에 적고(전), 고친 뒤 0이다(후). `just lint`가 통과한다. `rg -c '#\[allow' crates`가 전과 같고 새 `#[expect(`마다 `reason`이 있다. `cargo check --workspace --all-targets --locked`의 출력에 `warning:`이 없다.
- C5(I1) `cargo test --workspace`, `just release-check`, `canon gate check`, `just atip-deep`가 통과한다. `cargo xtask check-generated`가 깨끗하다. `Cargo.lock`의 차이는 비어 있거나 I1의 예외마다 증거에 적힌 것이다. `git diff main --stat`이 I1의 목록 밖의 파일을 담지 않는다(`.canon/changes/canon-0-43-rust-1-100/`와 이 변경의 레저 기록은 목록 안이다). `.rs`의 차이를 읽어 lint 자리의 다시 쓰기와 I1의 예외 말고는 없음을 리뷰에 적는다.
- C6(I5) `rg -n -- '-cargo-$' .github/workflows/test.yml`이 비어 있다. PR 실행의 각 캐시 잡 로그가 `Cache not found`다(차가운 실행). 잡별 시간과 한계를 리뷰에 표로 남긴다: 기준은 `canon-0-42-ci-refresh`가 잰 시간이다. 샌드박스에서 `CARGO_INCREMENTAL=0 cargo test --workspace --no-run --locked` 뒤 `scripts/ci-strip-workspace-build.sh`를 돌리고 `find "$CARGO_TARGET_DIR" -iname '*canon*'`가 비어 있다(`CARGO_INCREMENTAL=0`은 CI와 같은 조건이다). 병합한 세션이 첫 main push의 `Save cargo cache`가 새 키로 저장됐는지 확인한다.
- C7(I6) `rg -n 'rust:1\.' scripts`가 비어 있다. `just release-check`가 통과하고, 이미지 줄을 지운 복사본과 `ubuntu:26.04@sha256:`로 바꾼 복사본에서는 "must set `alpine_image:`"로 실패한다(시험이 이 둘을 이미 본다).
- C8(I7) CHANGELOG에 전후 표와 한 줄 설명이 있고 `docs/design/rust-coding-rules.md`의 핀 설명에 버전이 없다. `.agents/skills/repo-scaffold/SKILL.md`와 `.claude/skills/repo-scaffold/SKILL.md`가 원본과 같고 `rust-version.workspace = true`를 담는다. 읽는 시험은 없다. 사람이 리뷰에서 읽는다.

## Out of scope

- edition 2024 전환. 규칙 문서가 레포가 정할 것으로 두었다.
- 1.100이 더한 lint로 `[workspace.lints]`나 `-D` 목록을 늘리는 일. 이 PR은 목록을 그대로 두고 결과만 증거에 남긴다.
- 검사기를 `cargo install atip-check`로 되돌리는 일. `canon-0-43-atip-0-3-2`가 미리 빌드한 npm 검사기를 쓰고 owner가 그대로 두기로 했다. 이 변경이 병합돼도 바꾸지 않는다.
- `build-native.yml`, `publish.yml`, `pr-guard.yml`의 캐시 키(`canon-cli-<target>-cargo-<Cargo.lock 해시>`)에 툴체인 해시를 더하는 일(질문 3).
- `rust-version`과 `channel`이 같은지 따로 검사하는 일. `rust-version`은 바닥값이고 canon은 라이브러리가 아니다. 이미지와 CI의 rustc는 이미 핀과 같지 않으면 실패한다.
- Alpine을 3.24에서 올리는 일, 다른 Rust 프로젝트(atip, forca, lute)의 툴체인, cargo의 의존성 쿨다운 설정.
- 시험 추가, 시험 이름 변경, CHANGELOG 문구를 읽는 시험.

## Questions and assumptions

- 질문 1: 열한 크레이트에 `rust-version`을 새로 선언할지. 기본: ==**선언한다**== (`"1.100.0"`, 워크스페이스에서 상속). 이유: 집 규칙이 핀과 함께 요구하고, 낮은 rustc로 빌드하면 cargo가 이유를 말하며 멈춘다. 대안은 선언하지 않고 핀만 두는 것이다(파일 열둘이 줄지만 규칙의 둘째 자리가 비어 있다).
- 질문 2: `clippy` 잡 이름에서 버전을 뺄지. 기본: ==**뺀다**== (`clippy and visibility rule`). 이유: 이름의 버전은 올릴 때마다 낡고, 필수 검사로 거는 곳이 없어 이름을 바꿔도 깨지는 것이 없다. 버전은 잡 로그의 `rustc` 줄과 핀에 있다. 대안은 `clippy 1.100.0 and visibility rule`이다.
- 질문 3: `build-native.yml`, `publish.yml`, `pr-guard.yml`의 캐시 키에 툴체인 해시를 더할지. 기본: ==**더하지 않는다.**== 이유: 이 키는 `Cargo.lock` 해시만 담아 옛 툴체인의 `target/`가 복원될 수 있지만 cargo가 rustc 지문이 다른 산출물을 버리므로 결과는 맞고 느릴 뿐이다. `Cargo.lock`은 0.43 버전 올림에서 바뀌어 새 키가 생긴다. 이 키들은 `canon-0-42-ci-refresh`가 릴리스 쪽 캐시 정책을 범위 밖으로 두었고 `canon-0-43-release-hardening`이 같은 파일을 고친다.
- 가정: 새 다이제스트는 인덱스 다이제스트이고(옛 핀과 같은 종류) 행의 빌드 단계가 그것을 쓴다. 구현이 다시 읽었을 때 값이 달라졌으면 새 값을 쓰고 증거에 적는다.
- 가정: `map_or_default`로 바꾸면 값이 같다(`unwrap_or_default`와 같은 `Default::default()`). 타입 추론이 안 되는 자리는 타입을 적어 고치고 `#[expect]`를 쓰지 않는다.
- 가정: lint의 수는 구현이 1.100.0 clippy로 센다. 위 글자 세기(137줄과 15곳)는 위쪽 경계이며 `.map(`과 `.unwrap_or_default()`가 같은 `Option`이나 `Result` 위에 있는지는 clippy만 안다. 0.42 계획의 한 곳이 전부라는 읽기는 이 글자 세기와 맞지 않으므로 [INFERENCE: 그 측정은 한 크레이트만 돌았다] 구현이 먼저 센다.
- 결정: 구현은 커밋 둘이다. 첫째는 핀, 선언, 이미지, 잡 이름, 캐시 다리, 시험 입력, 문서, 크레이트 만드는 규칙이고 둘째는 lint 자리의 다시 쓰기다. 둘째는 1.99.1에서도 컴파일되므로(`map_or_default`는 1.98.0부터 stable) 되돌릴 때 둘째는 남겨도 된다. ==**되돌림은 첫째 커밋 전체를 되돌리는 것이다.**== 핀만 되돌리면 `rust-version = "1.100.0"`을 선언한 크레이트를 cargo가 거절하고, aarch64 행의 빌드 단계가 이미지의 rustc와 `channel`을 비교해 실패한다. 측정과 빌드는 샌드박스에서 하고 맥에서 cargo를 돌리지 않는다.
- 결정: 병합 순서는 ==**`canon-0-43-atip-0-3-2` → `canon-0-43-test-cleanup` → `canon-0-43-release-hardening` → 이 변경**==이다. 이유 다섯.
  1. lint 자리는 73개 파일에 흩어져 있고 한 줄짜리 137곳 중 71곳이 `tests/`다. `test-cleanup`이 시험 파일을 지우거나 고치므로 먼저 병합되면 지워질 줄을 고치지 않고 충돌도 없다.
  2. lint 재확인(LINT-5)은 마지막 트리에서 한 번 한다. 다른 셋이 더하는 코드는 1.99.1의 clippy로 썼고 새 모양(`.map(f).unwrap_or_default()`)은 거기서 깨끗하다. 이 변경이 먼저 들어가면 나머지 셋이 리베이스할 때마다 자기 새 코드로 빨개진다.
  3. `test.yml`에서 `restore-keys`가 있는 잡은 여섯이다(`offline`, `clippy`, `atip`, `grammar-mutation`, `bench-shape`, `live-tier`). `test-cleanup`이 `grammar-mutation`을 잡째 지우므로 뒤에 오면 다섯만 고친다. `atip` 변경은 `atip` 잡의 검사기 캐시 단계를 지우고 이웃 줄을 고치므로 줄 충돌이 나지 않게 이 변경이 그 위로 옮긴다.
  4. `build-native.yml`과 `publish.yml`에서 이 변경이 고치는 곳은 행의 `alpine_image` 한 줄씩이고 `release-hardening`은 `pull_request:`·`paths:` 블록, 새 `scope` 잡, `needs:`와 게시 단계를 고친다. 줄이 겹치지 않는다. `scripts/test-release-workflow-safety.py`는 둘 다 고치므로 뒤에 오는 쪽이 리베이스한다.
  5. 툴체인은 이 넷 중 diff 밖의 이유로 CI를 빨갛게 만들 수 있는 유일한 변경이다. 마지막에 두면 앞의 셋의 초록이 기준선이 되고 되돌릴 때 이 변경의 첫째 커밋만 되돌린다. 이 PR의 `build-native.yml` 실행이 새 다이제스트를 0.43 릴리스 PR 전에 한 번 돌린다.
  `release-hardening`이 늦어지면 이 변경이 그 앞에 와도 된다. 둘은 겹치는 줄이 없고 `scope` 잡도 같은 경로 목록(`rust-toolchain.toml`, `.github/workflows/**`)으로 이 PR에서 `build-native.yml`을 돌린다.
- 결정: `canon-0-43-atip-0-3-2` brief의 "canon의 툴체인(1.99.1)에서는 `cargo install atip-check`가 되지 않는다"는 문장은 그 변경이 병합되는 시점에는 맞고 이 변경 뒤에는 낡는다. 그 brief는 건드리지 않고, 이 변경의 CHANGELOG 항목이 검사기 설치는 미리 빌드한 npm 바이너리 그대로라고 한 줄로 적는다.

Approved: journey 2026-11-17T09:53:19Z — "오너가 채팅에서 오케스트레이터에게 0.43 승인을 위임함 (2026-11-17 ask: '응'). 툴체인 1.100 상향은 오너 요청(Senna 경유 Buzz f274eaa0). 독립 계획 리뷰 1라운드 approve(P0/P1 없음), P2/P3 반영. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 0c6fb013d928439f3157a3b1ad6c0ef35fe8d5eb) (brief sha256:2d1d80a1a24533a93bfff1f7d9c9c4a0f0d29c552b9aec2428a37e5090647071)
