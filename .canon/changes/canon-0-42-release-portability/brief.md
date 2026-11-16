---
kind: chore
waive:
  - rule: "prose.contrast"
    section: "intent"
    reason: "빌드 이미지의 glibc와 바이너리가 쓰는 심볼의 값은 서로 다른 사실이고, 그 대비가 바닥이 오르는 이유다"
  - rule: "prose.contrast"
    section: "invariants"
    reason: "IP가 아니라 호스트 이름으로 연결하는 것과 인증 거절이 통과이고 인증서 오류가 실패인 것은 시험이 가르는 대비 자체다"
  - rule: "prose.key-sentence"
    section: "checks"
    reason: "인수의 시험 실행이 버전 올리기 커밋에서 도는 것이 그 확인의 핵심이다"
  - rule: "prose.paragraph-length"
    section: "checks"
    reason: "항목 하나가 한 스모크의 두 확인 곳과 부정 대조를 함께 말한다"
  - rule: "prose.parenthetical"
    section: "intent"
    reason: "괄호는 이미지 이름과 glibc 버전의 풀이다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 시험 파일 이름이다"
  - rule: "prose.sentence-length"
    section: "checks"
    reason: "행 목록과 로그에서 읽을 항목을 나열한 문장이다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "러너 라벨, 심볼 이름, DLL 이름은 바이너리와 워크플로에 적힌 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "라벨, 환경변수, 패키지 이름, 타깃 이름, 컴파일러 플래그는 검사가 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "명령과 DLL, 타깃 이름은 실행해서 확인하는 철자 자체다"
  - rule: "prose.key-sentence"
    section: "invariants"
    reason: "동작 불변과 게시 차단 두 줄이 이 변경의 핵심이라 둘 다 표시한다"
  - rule: "prose.length"
    section: "brief"
    reason: "세 플랫폼 측정값, 벤치마크, 버린 방식의 이유, 패키지 일관성 목록을 빼면 구현자가 바닥과 검사를 다시 추측해야 한다"
  - rule: "prose.length"
    section: "intent"
    reason: "측정표와 벤치마크, owner가 준 호환 사실이 모두 필요하다"
  - rule: "prose.length"
    section: "invariants"
    reason: "플랫폼마다 불변 하나와 그 검사가 필요하다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변마다 확인 방법과 실패를 보이는 길이 있어야 한다"
  - rule: "prose.paragraph-length"
    section: "invariants"
    reason: "항목 하나가 한 플랫폼의 바닥, 검사, 실패 메시지를 함께 말한다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 옵션 이름과 값의 풀이다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "나열된 검사 항목을 한 문장으로 묶은 것이다"
---

# 릴리스 바이너리가 선언한 바닥을 지킨다: 고정 러너, 정적 Linux(musl), macOS 15.0, 정적 Windows CRT, linux-arm64

Source: .github/workflows/publish.yml, .github/workflows/build-native.yml, scripts/check-release-workflow-safety.py, scripts/test-release-workflow-safety.py, packages/cli/src/index.ts, packages/core-*, crates/canon-cli/src/main.rs, .cargo/config.toml, 0.41.0 npm 바이너리 세 개를 직접 잰 값, musl-bench.md(2026-11-14, 0.41 계획 검토 폴더)

## Intent

릴리스 바이너리가 어떤 OS에서 도는지는 지금 빌드 러너가 우연히 정한다. 0.41.0으로 `npm pack`한 세 바이너리를 잰 값은 이렇다.

| 플랫폼 | 빌드 러너 | 잰 값 |
| --- | --- | --- |
| linux x64 | `ubuntu-latest`(빌드 로그의 이미지는 `ubuntu-26.04`) | 필요한 GLIBC 심볼 버전의 최댓값 `GLIBC_2.39`. 공유 라이브러리는 `libc.so.6`, `libm.so.6`, `libgcc_s.so.1`. |
| macOS arm64 | `macos-15` | `minos 11.0`(SDK 15.5). `Security`, `CoreFoundation`, `libiconv`, `libSystem`에 링크한다. |
| Windows x64 | `windows-latest` | `VCRUNTIME140.dll`와 UCRT `api-ms-win-crt-*` 열 개를 import한다. |

- ==**linux 바닥은 코드 변경 없이 오른다.**== 0.41.0의 Linux 빌드는 `ubuntu-latest`로 돌았고 빌드 로그의 이미지는 `ubuntu-26.04`였다. 그런데 바이너리의 최댓값은 2.39다. 이 값은 빌드 이미지가 보장하는 것이 아니라 링크된 코드가 우연히 쓴 심볼에서 나온 것이어서, 의존이나 툴체인이 바뀌면 코드 변경 없이 더 새 심볼을 요구할 수 있다. glibc에는 macOS의 배포 타깃 같은 플래그가 없다.
- 이미 0.41이 못 도는 Linux가 있다. Debian 12 기반 Docker 이미지(`node:*-bookworm`, `rust:*-bookworm`, glibc 2.36), `ubuntu-22.04` 러너, RHEL 9와 Amazon Linux 2023(glibc 2.34)에서 `GLIBC_2.39 not found`로 죽는다. 런처는 지원하지 않는 플랫폼, arch, libc 종류만 걸러 내고 너무 낮은 glibc는 걸러 내지 못해서, 사용자는 로더의 오류를 그대로 본다.
- macOS의 11.0은 지금 rustc의 기본값이다. 어디에도 적히지 않아서 툴체인이나 SDK가 바뀌면 같이 움직인다. 행이 `macos-15`인 것은 `macos-14`가 퇴역해서 한 칸 옮긴 결과이지 요구가 아니다.
- Windows는 `VCRUNTIME140.dll`이 든 VC++ 재배포 패키지가 없는 깨끗한 Windows에서 시작하지 못한다.
- 안전 검사(`just release-check`)는 두 워크플로의 행, 환경변수, 패키지 목록이 서로 같은지만 본다. 러너 라벨도, 만들어진 바이너리도 보지 않는다. 세 행이 모두 `-latest`여도 통과한다.
- 지원 목록은 세 플랫폼이다. linux arm64 사용자는 런처의 `no binary found for linux/arm64`로 종료 1이다.
- postgres와 S3 클라이언트는 모든 빌드에 들어 있다. `canon-store`는 `object_store`(`aws`)와 `sqlx`(postgres, `tls-rustls`)를 항상 컴파일하고 `live-pg`, `live-r2`는 시험 전용 빈 기능이다. CLI는 실행 때 실제로 연결한다. 핫 계층의 postgres는 연결과 스키마 DDL까지 하고(`PgTier::connect`), 콜드 계층의 s3는 `R2Tier::attach`로 붙는다. 이 저장소의 `canon.yaml`도 핫을 postgres로, 콜드를 s3로 보낸다. 정적 musl 바이너리에서 이름 해석은 musl의 해석기를 거치고 NSS가 없다[INFERENCE: tokio와 hyper가 `getaddrinfo`를 쓴다고 본다]. TLS 신뢰 저장소는 두 갈래다. postgres의 `sqlx`는 `tls-rustls`가 고르는 `webpki-roots`로 바이너리 안에 든 루트를 쓴다. S3의 `reqwest`는 `rustls-platform-verifier`로 Linux에서 시스템 CA 파일을 읽고, 번들 루트는 wasm 빌드에만 있다. 그래서 S3 TLS는 호스트에 CA 파일이 있어야 하고 이것은 0.41의 glibc 빌드와 같다. 지금의 스모크 명령(`--version`, `skills show`, `search`, `gate check`)은 이름 해석도 TLS도 연결도 하지 않는다.

Linux libc의 벤치마크(0.41.0 main d7518d159, 릴리스 빌드, aarch64, owner의 Mac 위 Docker, `debian:bookworm` 컨테이너에서 이 저장소 체크아웃에 대해 세 번 교차 실행한 중앙값, 초)다.

| 명령 | glibc | musl | musl + jemalloc |
| --- | --- | --- | --- |
| `status --json` | 1.69 | 1.93 | 1.66 |
| `gate check --json` | 0.76 | 1.00 | 0.64 |
| `query --kind evidence --json` | 0.163 | 0.225 | 0.146 |
| `search approval --json` | 1.77 | 2.98 | 1.58 |

- musl 빌드는 코드 변경 없이 된다(`aws-lc-sys`, SQLite, zstd, `ring` 모두 그대로). 정적이고 26.3 MB이며 jemalloc을 더하면 26.8 MB다. musl만 쓰면 느리고, ==**musl과 jemalloc을 함께 쓰면 glibc와 같거나 빠르다.**== 잰 것은 aarch64 하나이고 x86_64 musl은 재지 않았다.
- jemalloc은 빌드 때 페이지 크기를 정한다. arm64 Linux 커널은 4K, 16K(Asahi, Raspberry Pi 5), 64K(RHEL)가 있고, 가장 큰 64K로 빌드하면 셋 모두에서 돈다. 비용은 할당 단위가 커지는 것이다. 0.41.0을 mac(16K 페이지)에서 잰 최대 상주 메모리는 `status` 132 MB, `gate check` 126 MB, `search` 725 MB여서 64K의 추가분은 무시할 만하다[INFERENCE: 64K 빌드로 직접 재지 않았다].

owner가 정한 것: macOS는 arm64만이고 배포 타깃 15.0이며 `macos-26`에서 빌드한다(11~14는 지원을 버린다). Windows는 x64만이다. Intel mac과 Windows arm64는 하지 않는다. Linux는 musl 정적(x64와 arm64)이고 musl에서만 jemalloc을 쓰며, aarch64 musl은 64K 페이지로 빌드한다. 사전 릴리스 없이 0.42.0을 곧바로 `latest`로 낸다. glibc는 버린 방식으로 적는다.

## Category

build

## Invariants

- I1 ==**canon의 명령, 출력, 레저는 바뀌지 않는다.**== 같은 소스에서 나온 바이너리의 동작이 같다. 바뀌는 것은 빌드 환경, 링크 방식, musl 빌드의 메모리 할당기, 어떤 패키지가 나가는지, 지원하는 OS의 바닥이다.
- I2 릴리스 두 워크플로(`publish.yml`, `build-native.yml`)의 모든 잡이 `-latest`가 아닌 고정 러너 라벨을 쓴다. 가장 새 이미지를 쓰되 라벨을 적는다. 허용 라벨은 `macos-26`, `ubuntu-26.04`, `ubuntu-26.04-arm`, `windows-2025` 넷이다. 두 워크플로의 행은 호스트까지 같다.
- I3 Linux 행은 타깃 `x86_64-unknown-linux-musl`(러너 `ubuntu-26.04`)과 `aarch64-unknown-linux-musl`(러너 `ubuntu-26.04-arm`)이다. 컨테이너 없이 호스트에서 링크한다. 바이너리가 정적이지 않으면, 곧 인터프리터(PT_INTERP)나 필요한 공유 라이브러리(DT_NEEDED)가 하나라도 있으면 릴리스 빌드가 실패하고 그 이름을 말한다. 이 바이너리에는 glibc 바닥이 없다.
- I4 musl 타깃에서만 jemalloc이 전역 할당기다. `tikv-jemallocator`는 `cfg(target_env = "musl")`에 걸린 의존이고, 다른 타깃(macOS, Windows)의 의존 트리와 바이너리에는 들어가지 않는다. 릴리스 빌드는 musl 바이너리에 jemalloc이 실제로 들어 있는지 확인하고, 없으면 실패한다. 할당기를 말없이 잃으면 위 표의 `musl` 열(느린 쪽)이 된다. ==**`aarch64-unknown-linux-musl` 빌드는 `JEMALLOC_SYS_WITH_LG_PAGE=16`(64K)으로 빌드한다.**== 이 값이 빌드에서 실제로 먹었는지 릴리스 검사가 확인하고(바이너리나 빌드 출력에서 읽는다), 먹지 않았으면 실패한다. x86_64 행은 이 변수를 쓰지 않는다(x86_64는 4K 하나다).
- I5 `MACOSX_DEPLOYMENT_TARGET=15.0`이 두 워크플로에 같은 값으로 명시된다. macOS 바이너리의 `minos`가 15.0이 아니면 릴리스 빌드가 실패한다. 15.0 미만은 고정이 먹지 않았다는 뜻이고 15.0 초과는 바닥이 말없이 올랐다는 뜻이다. macOS 행은 arm64 하나다.
- I6 msvc 타깃은 C 런타임을 정적으로 링크한다(`+crt-static`). 설정은 cargo 설정 파일 한 곳에 있어 두 워크플로가 같은 것을 읽고, 워크플로의 `RUSTFLAGS`가 그것을 덮지 않는다. Windows 바이너리의 import에 VC 런타임 DLL(`VCRUNTIME*`, `MSVCP*`, `CONCRT*`)이 하나라도 있으면 릴리스 빌드가 실패한다. UCRT는 Windows의 구성요소이므로 이 검사가 막지 않는다. Windows 행은 x64 하나이고 최소 버전은 새로 정하지 않는다(rustc가 정한 것을 따른다).
- I7 linux-arm64 패키지 `@journeykit/canon-core-linux-arm64`(`os: linux`, `cpu: arm64`)를 더하고, 기존 `@journeykit/canon-core-linux-x64`는 이름 그대로 musl 정적 바이너리를 담는다. 두 패키지에 `libc` 필터를 달지 않는다. 런처는 linux/x64와 linux/arm64를 호스트 libc와 상관없이 이 둘로 풀고 libc 감지(`CANON_LIBC` 환경변수 포함)를 없앤다. 그 밖의 플랫폼과 arch는 지금처럼 "패키지 없음" 오류다. 작업 공간 개발 빌드의 탐색 경로는 Linux에서 musl 타깃 디렉터리를 먼저 보고 `target/release`로 내려간다.
- I8 네 패키지의 빌드, 다운로드, 체크섬 검증, 실행 권한, 버전 적용, 게시 전 점검, 게시 단계, `@journeykit/canon`의 `optionalDependencies`, `bun.lock`이 서로 같은 집합이다. 빠진 곳이 있으면 안전 검사가 이름을 대며 실패한다.
- I9 각 Linux 바이너리는 낡은 glibc 이미지(`debian:12`, glibc 2.36: 0.41이 죽던 이미지)와 musl 이미지(`alpine`)의 컨테이너에서 돈다. 이미지는 digest로 고정한다. 명령은 `--version`(워크스페이스 버전과 같아야 한다), `skills show canon`, 이 저장소 체크아웃에 대한 `search`와 `gate check`(SQLite FTS와 할당기를 쓴다)이고 모두 종료 0이다. git은 `-c safe.directory=*`로 부른다. 컨테이너의 root가 러너 uid 소유의 체크아웃을 거절하지 않게 하기 위해서다[INFERENCE: git의 safe.directory 검사가 거절할 수 있다]. x64는 x64 러너, arm64는 arm 러너에서 네이티브로 돈다. arm 러너의 커널은 4K 페이지라 64K 빌드가 4K에서 도는 것을 덮고, 16K와 64K 커널은 CI가 덮지 않는다[INFERENCE: 더 큰 페이지로 빌드한 jemalloc이 더 작은 페이지의 커널에서 돈다는 jemalloc의 규칙에 기댄다]. ==**이 스모크는 릴리스 워크플로(`publish.yml`)에서만, 게시 단계 앞에서 돈다. PR과 매 푸시에서는 돌지 않는다. 하나라도 실패하면 게시가 돌지 않는다.**==
- I10 바이너리 검사는 설치 없이 도는 스크립트 하나다(표준 라이브러리만). 플랫폼별로 ELF(정적 여부), Mach-O(`minos`), PE(import)를 읽는다. 로컬에서 npm 패키지의 바이너리에 같은 명령으로 돌릴 수 있고, 기대값을 옵션으로 바꿔 실패하는 모습을 볼 수 있다.
- I11 안전 검사가 I2~I9, I13~I16을 읽는다: 워크플로의 `-latest`와 허용 밖 라벨, Linux 행의 타깃이 musl인지, aarch64 musl 행에만 `JEMALLOC_SYS_WITH_LG_PAGE=16`이 있는지, `MACOSX_DEPLOYMENT_TARGET`의 불일치와 값, cargo 설정의 `crt-static` 누락과 워크플로의 `RUSTFLAGS`, 행마다 바이너리 검사 단계 누락, 매트릭스의 `package_dir`를 런처가 풀지 못하는 경우, 세 스모크 잡(I9, I13, I15)의 누락, `publish`가 그 잡을 기다리지 않는 경우, 스모크가 `build-native.yml`에 있는 경우, 스모크 이미지와 서비스 컨테이너 이미지의 digest 누락, `dry_run` 건너뛰기 조건이 게시 단계 밖에 있거나 게시 단계에 없는 경우와 태그 푸시를 건너뛰게 하는 조건, 게시 단계의 순서(`core-linux-arm64` 첫째, 런처 마지막), 게시 전 점검의 `npm whoami`가 패키지 부재 조건 없이 도는 경우, `timeout-minutes`가 없는 잡(네 워크플로 모두). 타르볼 모양으로 바뀌는 사전 릴리스 규칙과 패키지 집합 규칙도 이 변경이 다시 쓴다(체크 참조). 규칙마다 통과 시험과 거절 시험이 있다.
- I12 지원 바닥과 패키지 변경이 사용자가 읽는 곳에 적힌다. CHANGELOG에 한 줄씩 명시한다: ==**macOS 15.0 미만(11~14)은 0.42부터 지원하지 않는다.**== Linux는 libc와 무관한 정적 바이너리이고 낡은 glibc에서도 돈다. linux-arm64가 추가됐다. Windows는 재배포 패키지가 필요 없다. 웹 문서의 아키텍처 쪽 플랫폼 패키지 목록(영어와 한국어)과 `repo-scaffold` 스킬의 "Three ship today"도 고친다. 스킬은 원본(`canon/skills-dev`)을 고치고 설치본을 다시 만든다. npm 페이지에 보이는 패키지 설명도 고친다. `core-linux-x64`의 설명 "canon CLI binary for linux x64 (glibc)"는 거짓이 되므로 정적 musl이라고 바꾸고, `core-linux-arm64`도 같은 모양으로 쓴다.
- I13 설치 경로 스모크: 게시할 바로 그 타르볼로 한다. 런처와 플랫폼 패키지를 `npm pack`하고(버전 적용과 런처 빌드가 끝난 뒤), 깨끗한 접두 디렉터리에 타르볼을 설치하고, 런처가 낸 `canon --version`이 워크스페이스 버전과 같아야 한다. 컨테이너는 `debian:12` 계열(glibc 2.36)과 `alpine` 계열이고 digest로 고정한다. 런처가 Node를 요구하므로 Node가 든 이미지(`node:*-bookworm-slim`과 `node:*-alpine`)를 쓴다. x64와 arm64 러너 각각에서 돈다. 이 스모크도 릴리스 워크플로에서만, 게시 단계 앞에서 돌고, 게시 단계는 스모크가 쓴 그 타르볼을 게시한다(다시 `npm pack`하지 않는다). 하나라도 실패하면 게시가 돌지 않는다.
- I14 시험 실행: 릴리스 워크플로(`publish.yml`)의 `workflow_dispatch`에 불리언 입력 `dry_run`이 있고 수동 실행의 기본값은 true다. 태그 푸시는 입력이 없고 언제나 진짜 릴리스다. `dry_run`이 true인 실행은 모든 빌드, 검사, 스모크(I9, I13, I15), 패키징을 돌리고 ==**게시 단계(네 `npm publish`)만 건너뛴다.**== 건너뛰는 조건은 게시 단계에만 붙고 다른 단계나 잡에는 붙지 않는다. 조건은 입력이 없는 태그 푸시를 건너뛰지 않는 모양이어야 한다. 읽기 전용인 게시 전 점검은 시험 실행에서도 돈다. 점검은 `npm view`로 패키지마다 npm에 있는지 본다(`E404`는 한 번도 게시된 적 없는 패키지다). `npm whoami`는 ==**세트의 패키지 중 npm에 없는 것이 하나라도 있는 동안에만**== 돈다. 이때만 신뢰 게시(OIDC)로 첫 게시를 할 수 없어 `NODE_AUTH_TOKEN`에 기대기 때문이다. 모든 패키지가 이미 npm에 있으면 `whoami`를 돌리지 않으므로, 이후 릴리스가 신뢰 게시로 넘어간 뒤에는 토큰이 없거나 만료돼도 점검이 막지 않는다. `whoami`는 토큰의 유효만 보고 `@journeykit` 범위의 쓰기 권한이나 2FA 우회는 보지 못한다[INFERENCE]. 이미 게시된 버전이라는 점검 실패의 안내문은 `recovery=true`와 `dry_run=false`를 함께 말한다. `recovery=true`에 `dry_run=true`를 같이 준 수동 실행은 게시 없이 초록으로 끝나지 않고 일찍 실패한다. `dry_run`을 false로 준 수동 실행은 진짜 게시이다. 게시 순서는 새 `core-linux-arm64`가 플랫폼 패키지 중 첫째이고 런처가 마지막이다. 인증 문제가 있으면 다른 0.42.0 패키지가 나가기 전에 멈춘다.
- I15 네트워크 경로 스모크: 정적 musl 바이너리가 이름 해석과 TLS와 postgres 연결을 실제로 한다는 것을 게시 전에 본다. `publish.yml`에서만 돌고 x64와 arm64 러너 각각에서 `debian:12`와 `alpine` 컨테이너로 돈다. 진짜 비밀은 쓰지 않는다. (a) 사용자 정의 docker 네트워크의 `postgres` 서비스 컨테이너(digest 고정)에 ==**IP가 아니라 호스트 이름으로**== 연결한다(musl 해석기를 타게 하려는 것이다). 핫 계층이 postgres인 `canon.yaml`과 스모크가 만든 임시 비밀번호의 `CANON_PG_DSN`으로 핫 계층을 붙이는 명령 하나가 종료 0이다(연결과 스키마 생성까지 한다). (b) 공개 HTTPS S3 호환 엔드포인트(예: `https://s3.us-east-1.amazonaws.com`)를 `CANON_S3_ENDPOINT`로 주고 가짜 키와 존재하지 않는 버킷으로 콜드 계층을 읽는 명령 하나를 돌린다. 인증 거절(예: `InvalidAccessKeyId`, 서명 불일치)이 통과이고 이름 해석 실패, 연결 실패, 인증서 오류는 실패다. 이 경로는 시스템 CA 파일을 읽으므로 이미지에 없으면 스모크가 `ca-certificates`를 설치한다. 어떤 명령이 계층을 붙이고 읽는지, 인증 거절과 인증서 오류를 오류 문구로 가르는 방법은 구현이 한 번 돌려 정한다. 하나라도 실패하면 게시가 돌지 않는다.
- I16 병합 순서와 시간 제한: 이 변경은 `canon-0-42-ci-refresh`가 병합된 다음에 병합한다(이 변경이 그 위로 옮긴다). 이 변경이 더하거나 바꾸는 모든 잡에 `timeout-minutes`를 둔다. 값의 규칙은 ci-refresh와 같다. 측정한 가장 긴 실행의 세 배를 5분 단위로 올린다. 측정 전 초기값은 바이너리 스모크 15분, 설치 경로 스모크 15분, 네트워크 스모크 15분이다[INFERENCE: 재 본 적 없는 추정]. 기존 build 행의 제한(build-native 45분, publish 50분)은 musl 행과 arm 행을 넣어 다시 재서 확인한다. 새 `uses:` 줄은 ci-refresh가 고정한 같은 버전과 SHA를 쓴다.

## Checks

- ==**`just release-check`가 통과한다.**== 안전 검사와 그 시험(`scripts/test-release-workflow-safety.py`)이 I2~I9의 규칙마다 통과 시험과 거절 시험을 가진다. 규칙 하나를 지운 시험용 워크플로나 시험용 매니페스트는 검사에서 실패한다.
- 실제 바이너리로 하는 거절 확인 셋(0.41.0 npm 패키지를 외장 디스크 임시 폴더에 풀어서 돌린다).
  - Linux: 0.41.0의 `canon`은 정적 검사(I3)에서 실패하고 인터프리터와 `libc.so.6`을 말한다.
  - macOS: 0.41.0의 `canon`(`minos 11.0`)은 I5의 검사에서 실패하고 11.0을 말한다.
  - Windows: 0.41.0의 `canon.exe`는 I6의 검사에서 실패하고 `VCRUNTIME140.dll`을 말한다.
- `build-native.yml`이 이 변경의 PR에서 네 행(macOS arm64, Linux x64 musl, Linux arm64 musl, Windows x64) 모두 초록이다. 로그에서 확인한다: Linux 행의 바이너리가 정적이고 jemalloc이 들어 있다, aarch64 행의 64K 페이지 설정이 먹었다, macOS 행의 `minos`가 15.0이다, Windows 행의 import 목록에 VC 런타임 DLL이 없다. 새 바이너리가 I3~I6를 통과한다는 증거는 이 로그뿐이다.
- 스모크(I9, I13, I15)는 PR에서 돌지 않으므로 게시 전에 두 곳으로 확인한다. 첫째, 스모크 스크립트를 owner의 Mac(arm64 Docker)에서 알파인으로 빌드한 aarch64 musl 바이너리에 돌려 `debian:12`와 `alpine`에서 모두 종료 0임을 본다. 이때 네트워크 스모크의 부정 대조를 한 번 한다. 인증서가 믿을 수 없는 호스트(예: `https://untrusted-root.badssl.com`)를 `CANON_S3_ENDPOINT`로 주면 인증서 오류로 실패해야 한다. 둘째, 릴리스 워크플로를 `dry_run`(기본값 true)으로 수동 실행해 빌드, 검사, 스모크 잡이 x64와 arm64 각각 두 이미지에서 초록이고 게시 단계가 모두 건너뛰어졌음을 본다. 로그에서 이미지가 digest로 고정돼 있고 glibc 이미지의 glibc가 2.36임을 확인하고, 네트워크 스모크 로그에 `postgres` 호스트 이름 연결과 S3 인증 거절이 있음을 확인한다.
- 시험 실행(I14): 인수의 시험 실행은 ==**0.42.0 버전 올리기 커밋에서 태그 전에**== 돈다. 그 커밋의 작업 공간 버전 0.42.0은 아직 npm에 없어서 게시 전 점검이 `recovery` 없이 통과한다. 올리기 전(0.41.0이 이미 npm에 있다)의 시험 실행은 점검에서 실패하므로 인수가 아니다. 증거에는 시험 실행의 커밋을 적고 올린 뒤에 기록한다. 같은 실행에서 `npm view`로 0.42.0이 새로 생기지 않았음을 확인한다. 이 릴리스에서는 `core-linux-arm64`가 npm에 없으므로 로그에 `npm whoami` 결과가 있어야 한다. 실패 경로: `recovery=true`와 `dry_run=true`의 수동 실행은 일찍 실패하고, 이미 게시된 버전의 점검 실패 안내문에 `recovery=true`와 `dry_run=false`가 모두 있다. 안전 검사의 시험은 게시 단계에서 조건을 뺀 워크플로, 게시가 아닌 단계에 조건을 단 워크플로, 태그 푸시도 건너뛰게 하는 조건의 워크플로, 게시 순서가 어긋난 워크플로, 패키지 부재 조건 없이 `npm whoami`를 돌리는 워크플로를 실패시키고, 부재 조건 아래의 `whoami`는 통과시킨다. 모든 패키지가 npm에 있는 상태에서 `whoami`를 건너뛴다는 것은 점검 단계의 조건을 읽는 이 시험이 보이고, 이 릴리스의 실제 실행으로는 볼 수 없다.
- 같은 PR에서 `bun install --frozen-lockfile`과 `cd packages/cli && bun test`가 통과한다(새 워크스페이스 패키지가 `bun.lock`에, 런처에서 libc 감지가 빠진 것).
- `cargo test --workspace`가 같은 커밋에서 통과하고 `cargo build --locked`가 `Cargo.lock`의 변경(`tikv-jemallocator`) 뒤에도 된다. macOS의 개발 빌드 의존 트리에 jemalloc이 없다.
- 사전 릴리스의 `--tag next`와 정식 릴리스의 `latest` 분기는 뜻이 같고, 명령은 타르볼 경로를 갖는 모양으로 바뀐다(`npm publish "$TARBALL" --provenance --access public`, 사전 릴리스는 `--tag next` 추가). 안전 검사의 사전 릴리스 규칙(`prerelease_publish_errors`가 줄을 `npm publish --provenance --access public`과 같음으로 비교한다)과 그 시험, 패키지 집합 규칙(게시 단계와 매트릭스의 일치)을 이 모양에 맞게 다시 쓴다. 다시 쓴 규칙마다 통과 시험과 거절 시험이 있고, 사전 릴리스를 `latest`로 올리는 워크플로를 거절하는 시험이 남는다. 기존 시험은 새 모양으로 고쳐 통과한다.
- 시간 제한(I16): `timeout-minutes`가 없는 잡이 네 워크플로에 0개다(안전 검사 규칙과 그 시험). 리뷰에 새 잡의 초기값과 시험 실행 뒤 재측정 값, build 행의 재측정 값을 적는다.
- `just lint`, `just atip-deep`, `canon gate check`가 같은 커밋에서 통과한다.
- 이 변경은 워크플로와 스크립트와 `canon-cli`의 `main.rs`를 바꾸므로 코드 변경으로 센다. 이 변경이 adopt된 출시된 subject는 다시 열어 새 evidence를 기록하고 출시한다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

## Out of scope

- Intel mac(x86_64 macOS)과 Windows arm64 빌드(owner 결정: 하지 않는다).
- Windows 최소 버전을 새로 정하는 일.
- `test.yml`과 `pr-guard.yml`의 `ubuntu-latest`. 거기서 만든 바이너리는 나가지 않는다.
- 성능 회귀 하네스. 위 표는 한 번 잰 값이고 CI가 다시 재지 않는다. 지키는 것은 I4의 jemalloc 존재 확인까지다.
- 정적 musl 바이너리가 쓰지 못하는 동적 로더 기능(`dlopen`, NSS 기반 이름 해석). 이 저장소의 기본 빌드가 이것을 쓰는지는 확인하지 않았다. 확인한 것은 postgres와 S3 경로가 모든 빌드에 있다는 것이고 I15가 덮는다.
- glibc를 골랐다면 필요했을 런처의 "glibc 바닥" 오류 메시지(너무 낮은 glibc에서 로더 오류 대신 바닥을 말하는 일). musl 정적으로 확정돼 필요 없다.
- 바이너리 서명, 공증, SBOM.
- 릴리스 절차 자체와 버전 올리기. 사전 릴리스(`v0.42.0-rc.1`)는 하지 않고 0.42.0이 `latest`로 곧바로 나간다. 운영자 선행 작업은 아래 결정에 있고 사람이 하며 에이전트가 하지 않는다.

## Questions and assumptions

- 결정: 변경 종류는 chore다. canon의 명령과 출력과 레저가 바뀌지 않는다. 다만 사용자가 겪는 것이 둘 바뀐다: macOS 11~14 사용자는 owner 결정으로 지원 밖이 되고(0.42 CHANGELOG에 싣는다), linux arm64와 낡은 glibc의 Linux 사용자는 새로 쓸 수 있게 된다. 리뷰어가 이것을 사용자가 보는 새 동작으로 읽으면 지원 바닥 변경만 feature 변경으로 나눈다.
- 결정: 주제는 `atip-contract`다. 릴리스와 패키징을 가진 주제는 아직 없고, 같은 파일을 바꾼 0.35의 사전 릴리스 chore가 여기에 adopt됐다.
- 결정: 러너는 가장 새 이미지의 고정 라벨이다. owner가 `macos-26`(`macos-15`는 `macos-14` 퇴역 때 한 칸 옮긴 것일 뿐), `ubuntu-26.04`와 `ubuntu-26.04-arm`을 정했다. Windows `windows-2025`는 `windows-latest`가 지금 가리키는 이미지의 고정 이름이다[INFERENCE: 이 날짜의 매핑은 구현 때 러너 이미지 문서로 확인한다]. CMake와 NASM이 같은 이미지에 든다는 전제도 그때 확인한다.
- 결정: macOS 검사는 `minos`가 15.0이 아니면 실패한다. owner의 말은 15.0 미만을 거절하는 것이었다. 15.0 초과는 말없이 바닥이 오른 것이고 이 변경이 막으려는 사고여서 양쪽을 막는다. 15.0 미만만 막기를 원하면 I5의 문장만 바뀐다.
- 결정: Linux는 musl 정적과 musl 한정 jemalloc이 기본이다. 이유는 측정이다. musl+jemalloc이 glibc와 같거나 빠르고(`gate check` 0.64 초 대 0.76 초) 정적이라 바닥이 없다. musl만 쓰면 1.00 초로 느려서 jemalloc 없는 musl은 고르지 않는다. 같은 코드로 모든 타깃이 빌드되고 할당기만 musl에 걸린다.
- 결정: Linux 행은 컨테이너 없이 호스트에서 `musl-tools`(apt)와 `rustup target add`로 빌드한다. 이 저장소의 C 빌드(`aws-lc-sys`, `ring`, `libsqlite3-sys`, `zstd-sys`, `tikv-jemalloc-sys`)는 musl용 C 컴파일러를 찾아야 한다. arm 러너에서 `cc` 크레이트가 `aarch64-linux-musl-gcc` 대신 `musl-gcc`를 쓰게 하려면 `CC_aarch64_unknown_linux_musl` 같은 환경변수가 필요할 수 있다. owner의 벤치마크는 Alpine 이미지에서 돌렸고 이 러너 구성은 돌려 보지 않았으므로, 구현이 첫 실행으로 필요한 패키지와 변수를 확정한다. 막히면 같은 스모크의 방식으로 내려간다. 호스트 잡이 JS 액션(checkout, cache, artifact)을 돌리고 cargo 빌드는 한 단계 안에서 `docker run`으로, 작업 공간을 마운트한 digest 고정 `rust:alpine`(arm 러너에서는 arm64 이미지)에서 한다. 잡 `container:`로 Alpine을 쓰지 않는 이유는 Alpine 컨테이너 안의 JS 액션이 arm64 러너에서 지원되지 않는다고 알고 있어서다[INFERENCE: 러너 문서이고 오늘 다시 확인하지 않았다].
- 결정: aarch64 musl 빌드의 페이지 크기는 `JEMALLOC_SYS_WITH_LG_PAGE=16`이다(owner). 이유는 Asahi와 Raspberry Pi 5(16K), RHEL(64K) 커널에서도 도는 것이다. 설정이 먹었는지는 릴리스 검사가 읽는다. 읽는 방법은 구현이 정한다(후보: jemalloc의 통계 출력이 페이지 크기를 말하는지[INFERENCE: 확인하지 못했다], 빌드 출력). 읽을 방법을 찾지 못하면 안전 검사가 워크플로에 변수가 있는 것을 보는 데서 멈추고 그 사실을 리뷰에 적는다.
- 결정: 스모크는 호스트 러너에서 아티팩트를 받아 `docker run`으로 컨테이너에 마운트한다. 이미지에 도구를 더하지 않는 것이 목적이므로 잡 `container:`를 쓰지 않는다. canon이 git을 부르므로 `search`와 `gate check` 단계에서만 이미지에 git을 설치한다(`apk add`, `apt-get install`). 이미지는 멀티 아키텍처 인덱스 digest로 고정하고 구현이 그 digest를 정한다.
- 결정: Windows 검사는 VC 런타임 계열 DLL 이름의 거부 목록이다. 허용 목록은 OS가 import를 하나 더할 때마다 릴리스를 깨고, 막으려는 것은 재배포 패키지가 필요한 DLL이다.
- 가정: 세 OS의 러너에 표준 라이브러리 python이 있다. Windows 행에서 없으면 구현이 SHA로 고정한 설치 액션을 쓴다.
- 버린 방식(glibc 2.39 바닥): ① `ubuntu-26.04` 호스트의 잡이 `container: ubuntu:24.04`(digest 고정)에서 링크하고 GLIBC 심볼 2.39 초과를 거부한다. ② `cargo zigbuild --target x86_64-unknown-linux-gnu.2.39`. ③ `runs-on: ubuntu-24.04`. 버린 이유는 glibc 바닥 자체다. 2.39는 Debian 12(2.36), RHEL 9와 AL2023(2.34), `ubuntu-22.04`를 계속 막고, 사용자가 `GLIBC_2.39 not found`를 보지 않게 하려면 런처 오류 메시지 feature가 더 필요하다. 속도는 musl+jemalloc이 낫다. ②는 `aws-lc-sys`의 cmake 빌드가 zig cc에서 되는지 확인하지 못했고 zig와 cargo-zigbuild가 고정할 설치물 둘이 더 생기며 `build-native.yml` 머리 주석의 "zig를 쓰지 않는다" 결정을 뒤집는다. ③은 가장 단순하지만 owner가 최신 러너를 원했고 이미지 수명이 바닥의 수명이 된다. glibc로 가면 ①이 기본이다.
- 결정: Linux libc는 musl 정적(x64와 arm64, musl에서만 jemalloc)으로 확정이다(owner). 대가는 `tikv-jemallocator` 의존 하나, musl용 C 컴파일러 설정, 정적 링크가 쓰지 못하는 동적 로더 기능이다.
- 결정: 사전 릴리스 없이 0.42.0을 `latest`로 곧바로 낸다(owner). 첫 실행이 진짜 릴리스가 되므로 스모크와 검사는 게시 전에 한 번 돌려 볼 길이 필요하고, 그 길이 시험 실행(`dry_run`, I14)이다.
- 결정: 설치 경로 스모크가 게시할 타르볼을 쓰므로 게시 단계의 모양이 바뀐다. 패키지를 한 번 `npm pack`하고 스모크가 그 타르볼을 설치하고 게시가 같은 타르볼(`npm publish "$TARBALL" --provenance --access public [--tag next]`)을 게시한다. 지금의 `working-directory: packages/<dir>`에서 `npm publish`를 하는 단계는 없어지므로, 그 모양에 기대는 안전 검사의 두 규칙(사전 릴리스 규칙과 패키지 집합 규칙)은 이 변경이 다시 쓴다. 타르볼 설치 때 npm이 아직 게시되지 않은 플랫폼 패키지를 레지스트리에서 찾지 않게 하는 방법(`--offline`이나 두 타르볼을 함께 설치)은 구현이 정한다.
- 결정: 릴리스 워크플로에 시험 실행 입력 `dry_run`을 더한다(owner). 수동 실행의 기본값은 true이고 태그 푸시는 진짜 릴리스다. 사전 릴리스가 없어서 스모크가 처음 도는 때가 진짜 0.42.0 릴리스가 되는 것을 막는다. 지금의 `workflow_dispatch`는 게시까지 하므로 기본값을 true로 두어 실수로 게시하지 않게 한다. 시험 실행은 게시 단계만 건너뛰고 나머지는 같은 경로로 돈다. 안전 검사는 게시 단계에만 그 조건이 있는지 확인한다. 점검(`npm view`, 조건부 `npm whoami`)은 읽기 전용이라 시험 실행에서도 돈다.
- 결정: 운영자 선행 작업(사람이 하는 일이고 에이전트가 하지 않는다)은 태그 전에 owner가 `NPM_TOKEN`을 새로 만드는 것이다(세분화 토큰, 2FA 우회, `@journeykit` 범위). 새 패키지 `@journeykit/canon-core-linux-arm64`는 npm에 아직 없어서(`npm view`가 E404) 신뢰 게시(OIDC)로는 첫 게시가 안 되고 토큰이 필요하다. 리뷰가 잰 저장소 비밀 `NPM_TOKEN`의 마지막 갱신은 2026-07-20(119일 전)이다. npm이 쓰기 가능한 세분화 토큰의 수명을 90일로 제한하므로 만료됐을 가능성이 크다[INFERENCE: 그 제한이 이 토큰을 만들 때 적용됐다고 본다]. 시험 실행의 점검이 이 토큰의 유효를 `npm whoami`로 한 번 본다. 이 점검은 세트의 패키지가 npm에 없는 동안(첫 게시)에만 돌므로, 첫 게시 뒤 모든 패키지가 npm에 있고 신뢰 게시가 토큰을 대신하면 토큰 없이도 릴리스가 막히지 않는다. 첫 게시 뒤 npm에서 trusted publisher를 설정하는 것도 owner의 일이다.
- 결정: 게시 순서는 새 `core-linux-arm64`를 플랫폼 패키지 중 첫째로 한다. 이 패키지의 첫 게시가 토큰에 기대므로, 인증이 실패하면 다른 0.42.0 패키지가 나가기 전에 멈추어 부분 릴리스가 생기지 않는다.
- 결정: 네트워크 경로 스모크(I15)가 있는 이유는 이 릴리스가 owner 팀이 매일 쓰는 postgres와 S3 경로의 libc를 바꾸고 사전 릴리스도 없기 때문이다. postgres 서비스 연결은 TLS 없이 이름 해석과 연결과 DDL을 덮고, TLS는 S3 경로(시스템 CA 파일)가 덮는다. postgres의 번들 루트(`webpki-roots`) 경로는 CI가 덮지 않는다.

Approved: journey 2026-11-16T05:49:14Z — "오너가 채팅에서 오케스트레이터에게 승인을 위임함 (2026-11-16, '일단 한번에 가자 기본안대로'). 독립 계획 리뷰 2라운드 approve(1라운드 P1 3건 수정 확인, P2 2_1 반영). 오너는 브리프 본문을 읽지 않음. NPM_TOKEN 교체는 태그 전 오너 몫." (relayed by planner planner-oco via chat) (code tree f41bb88d9c43e1d43b2e67ad37c1cc46ab96a8e3) (brief sha256:18c5fa204184d318915735309c101d31e7d7164c77464c1022268b044304b17a)
