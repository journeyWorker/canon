---
waive:
  - rule: "prose.code-citation"
    section: "detail"
    reason: "옵션 이름, 경고 코드, 경로 패턴, 이유 이름은 호출자가 읽는 철자 자체다"
  - rule: "prose.code-citation"
    section: "acceptance"
    reason: "검사에 쓰는 경로 패턴은 호출자가 쓰는 철자 자체다"
---

# 디렉터리와 글롭 표면 참조도 파일처럼 낡고, 낡음 검사는 못 본 것을 센다

Source: 벤더 감사 보고서 D13·D14·R11a·R12(0.37.0으로 재현, docs/research/vendor-audits/00-cross-vendor-patterns-rust-2026-11.md)와 채택 브리프의 질문 1·10, canon-0-38-code-scopes(같은 글롭 문법), canon-0-39-git-boundary(앞선 변경)

## Intent

`--surface-ref`는 증거가 덮는 파일을 선언하고, 게이트는 선언한 파일이 기준 커밋 뒤에 바뀌면 `stale-evidence`로 낮춘다. 0.37.0으로 잰 값은 이렇다. `src/`, `src`, `src/**`, `src/*.rs`는 경고 없이 저장되고 그 아래 파일을 고쳐도 `gate check`는 clean이다. 기준 커밋을 읽지 못하면(얕은 복제, `gc`로 사라진 커밋) 검사는 말없이 건너뛰어 clean이다. 조상이 아닌 기준 커밋은 신호가 없다. `status`는 신선도를 말하지 않는다.

## Desired outcome

- 디렉터리와 글롭 `--surface-ref`가 코드 제외 글롭과 같은 문법으로 바뀐 경로에 맞는다. ==**`src`를 선언한 증거는 src 아래 파일이 바뀌면 stale이 된다.**==
- 정확한 파일과 `effect:` 참조의 답은 그대로다.
- 아무 파일에도 맞지 않는 값은 저장할 때 경고한다.
- 기준 커밋을 읽지 못한 셀의 수를 `gate check`와 `status`가 같은 이유 셋으로 말하고, 판정과 종료 값은 그대로다.

## User flow

1. F1 에이전트는 디렉터리나 글롭으로 증거를 기록한다 → 값이 저장된다 → 그 아래 파일이 바뀌어 커밋되면 `gate check`가 `stale-evidence`를 낸다.
2. F2 에이전트는 어떤 파일에도 맞지 않는 값을 준다 → 값이 저장되고 경고가 나온다.
3. F3 CI는 얕은 복제에서 `gate check`를 돌린다 → 기준을 읽지 못한 셀의 수와 이유가 나온다 → ==**위반 목록과 종료 값은 그대로다.**==
4. F4 에이전트는 `canon status`를 읽는다 → 같은 수와 이유를 경고로 본다.
5. F5 사람은 CHANGELOG를 읽는다 → 새 일치 규칙과 더 요구하는 경우가 있고, 위험 등급 `paths`의 문법은 아직 다르다.

## Step detail

### F1 일치

- `effect:`가 아닌 값은 저장소 루트에서 시작하는 경로 패턴이다. 바뀐 경로와 문자열이 같거나 패턴에 맞으면 맞는다. ==**문법은 코드 제외 글롭과 같다.**==
- `code.exclude` 문법에서 `*`와 `?`는 `/`를 넘지 않고 `**`는 넘으며, 디렉터리 이름은 그 아래 전부에 맞는다. 값 끝의 `/`는 떼고 읽는다.
- `code.exclude`와 달리 `/`가 없는 패턴도 루트 기준이다. `Cargo.toml`은 루트의 파일 하나다.
- 바뀐 경로는 지금과 같다(이름 변경은 옛 경로와 새 경로 모두). `effect:`는 경로와 비교하지 않는다.
- 이유는 맞은 선언과 맞은 경로를 처음 3개와 총수로 말한다.
- `code.exclude`, `code_scopes`, 표면 참조는 한 매처를 쓴다. 매처는 패턴과 경로 길이의 곱에 비례하는 시간 안에 답한다.

### F2 쓸 때 경고

- `effect:`가 아닌 값이 작업 트리의 어떤 파일에도 맞지 않으면 경고 `SURFACE_REF_UNMATCHED`가 그 값을 말한다. 값은 저장되고 종료 값은 그대로다. 파일 목록을 얻지 못하면 경고하지 않는다.

### F3 기준을 읽지 못한 셀

- 대상은 git까지 가는, 곧 코드 트리 단축으로 건너뛰지 못한 통과 증거의 (주제, 역할) 셀이다. 이유는 셋이다.
  - `base_missing`: 기준 커밋이 이 저장소에 없다.
  - `base_not_ancestor`: 있으나 HEAD의 조상이 아니다.
  - `git_unavailable`: git이 답하지 못한다(실행 불가, 시간 초과, 읽을 수 없는 출력).
- 판정은 그대로다. `base_missing`과 `git_unavailable` 셀은 지금처럼 아무것도 보태지 않고, `base_not_ancestor` 셀은 지금처럼 `<기준>..HEAD`로 계산한다.
- `gate check`는 `staleness_unresolvable` advisory 하나를 낸다. 총 셀 수, 이유별 셀 수, 처음 3개 주제를 말하고, 얕은 복제이면 `git fetch --unshallow`를 안내한다. 셀이 없으면 아무것도 내지 않는다.

### F4 status

- `status`는 같은 수와 이유를 경고 `STALENESS_UNRESOLVABLE` 하나로 말한다. `data` 모양은 그대로다.

### F5 안내

- CHANGELOG의 Changed에 새 일치 규칙과 게이트가 더 요구하는 경우를, Added에 advisory와 경고를 싣는다. 위험 등급 `paths`는 `*`가 `/`를 넘고 다음 호환을 깨는 릴리스에서 맞춘다고 적는다. 도움말과 안내문도 같다.

## Acceptance

- F1:
  - `src/`, `src`, `src/**`, `src/*.rs`를 각각 선언한 증거 넷은 `src/a.rs`를 고쳐 커밋하면 모두 stale이고 이유가 맞은 선언과 경로를 말한다.
  - 실패 경로: ==**`src/x/b.rs`만 고치면 `src/*.rs` 증거는 stale이 아니다.**== 루트 `Cargo.toml` 선언은 `crates/x/Cargo.toml`을 고쳐도 stale이 아니고, 정확한 파일과 `effect:`는 0.38과 같은 판정을 낸다.
  - `/`가 든 패턴 표에서 세 곳이 같은 답을 내고, `*` 20개 패턴과 200자 경로가 일반 시험 시간 안에 끝난다.
- F2:
  - `src/nope/**`는 경고와 함께 저장되고 종료 값은 0이다.
  - 실패 경로: `../x`는 지금처럼 `USAGE`로 거절된다.
- F3:
  - 얕은 복제, `gc --prune=now`한 저장소, 다른 브랜치 장부를 옮긴 저장소가 `base_missing`, `base_missing`, `base_not_ancestor`를 말하고 위반 목록과 종료 값은 전과 같다. git을 PATH에서 뺀 저장소는 `git_unavailable`이다.
  - 실패 경로: 전체 복제에서는 advisory가 없다.
- F4:
  - 같은 저장소에서 `status`와 `gate check`의 수와 이유가 같다.
  - 실패 경로: 한 곳만 다르면 시험이 실패한다.
- F5:
  - CHANGELOG 항목이 있다.
  - 실패 경로: 항목이 없으면 릴리스 확인 시험이 실패한다.
- 생성 문서 drift 시험, `cargo test --workspace`, `canon gate check`가 통과한다.

## Questions and assumptions

- 결정: 읽을 때 맞추고 쓸 때는 맞는 것이 없는 값만 경고해 저장한다(채택 질문 10의 (c)와 (b)). 거절하지 않는다.
- 결정: `/`가 없는 값도 루트 기준이다. 어느 깊이에나 맞추면 루트 매니페스트 증거가 모든 크레이트의 매니페스트에 stale이 된다.
- 결정: 못 읽은 셀은 advisory만 낸다(질문 1의 (a)). 엄격한 등급의 stale은 CI에서 이 줄이 얼마나 뜨는지 본 뒤 따로 정한다. 조상이 아닌 기준도 판정을 그대로 둔다. 스쿼시 머지 뒤에 흔하고, 건너뜀으로 바꾸면 지금 stale인 셀이 clean이 된다.
- 결정: 위험 등급 `paths`는 이번에 맞추지 않는다. 승인된 정책의 뜻이 말없이 바뀐다.
- 결정: 이 변경은 canon-0-39-git-boundary 뒤에 선다. git 호출은 그 한 문을 지나고 경로는 `-z`로 읽는다. 그 문의 시간 초과는 git이 답하지 못한 것으로 센다.
- 결정: `?`는 지금처럼 바이트 하나를 센다.
- 실패 방식: 장부에 디렉터리 값이 이미 있는 저장소는 새로 stale이 된다. `status`는 기준 커밋마다 git 호출이 둘 늘어난다.

## Out of scope

- D12의 `-z` 경로 읽기와 D15의 git 시간 제한과 출력 한도(canon-0-39-git-boundary).
- 위험 등급 `paths`의 문법을 `code.exclude`에 맞추는 일(다음 호환을 깨는 릴리스).
- 줄과 구간 단위 앵커, 그 판정, 발견 기록의 줄 신선도.
- 새 경로 타입, 범위 우선순위 해석기, 외부 글롭 라이브러리와의 비교 시험.
- 기준 커밋이 없는 증거 기록의 침묵(이 저장소에 193건). git이 못 답한 것이 아니라 기록에 기준이 없는 것이다.
- 못 읽은 셀을 엄격한 등급에서 stale로 읽는 정책.
- 커밋하지 않은 작업 트리의 변경을 낡음으로 보는 일(검사는 지금도 `<기준>..HEAD`만 본다).
- `canon evidence approval-payload`의 `--surface-ref`.
