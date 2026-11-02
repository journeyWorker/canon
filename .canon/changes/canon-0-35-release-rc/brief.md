---
kind: chore
waive:
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "태그와 명령 이름은 릴리스 담당이 쓰는 철자 자체다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "이름과 예시 값을 괄호로 붙이는 표기라 문장으로 풀면 길이만 는다"
---

# 0.35.0-rc.1을 사전 릴리스로 게시한다

Source: .github/workflows/publish.yml, scripts/check-release-workflow-safety.py, 0.35 계획 (issue 134)

## Intent

0.35의 crate와 검사기(wave A와 B)는 canon 자신이 1.2와 L2를 선언하기 전에 다른 도구가 쓸 수 있다. canon의 0.35.0 전체는 wave C가 끝나야 한다. 그 사이에 `v0.35.0-rc.1` 태그로 먼저 내보내려면 게시 워크플로가 사전 릴리스 버전을 안전하게 다뤄야 한다. 지금 워크플로의 네 `npm publish`는 `--tag` 없이 돌고, 사전 릴리스 버전을 `latest`에 올리거나 npm이 거절한다.

## Category

ci

## Invariants

- ==**정식 버전의 게시 경로는 한 글자도 바뀌지 않는다.**== `vX.Y.Z` 태그는 `latest`로 올라가고 `--provenance --access public`을 쓴다.
- 버전에 하이픈이 든 사전 릴리스(`0.35.0-rc.1`)는 `--tag next`로 올라간다. 어떤 경우에도 `latest`로 올라가지 않는다.
- 태그가 작업 공간 버전(`Cargo.toml`)과 같은지 확인하는 기존 단계는 사전 릴리스 이름도 받는다. 같지 않으면 아무것도 게시하지 않는다.
- GitHub 릴리스가 있다면 사전 릴리스로 표시한다. 없으면 이 항목은 해당 없다.
- 플랫폼 패키지 셋과 launcher의 네 `npm publish`가 같은 태그 규칙을 쓴다. 한 패키지만 `latest`에 올라가는 일이 없다.
- 워크플로 안전 검사(`just release-check`)가 사전 릴리스 규칙을 알고 통과한다. 새 규칙을 빼도 실패한다.

## Checks

- ==**`just release-check`가 통과한다.**== 사전 릴리스 규칙을 지운 시험용 워크플로는 안전 검사에서 실패한다.
- 워크플로 시험이 `0.35.0-rc.1`은 `--tag next`, `0.35.0`은 태그 없음(latest)을 확인한다.
- 실제 태그는 이 변경에서 밀지 않는다. 게시는 오케스트레이터가 wave A와 B가 끝난 뒤에 한다.
- `just lint`와 `cargo test --workspace`가 같은 커밋에서 통과한다.

## Out of scope

- 0.35.0 정식 릴리스와 CHANGELOG 항목의 작성, 버전 올리기.
- crate의 crates.io 게시와 launcher의 다른 개선.
- 사전 릴리스를 소비하는 쪽(npm launcher)의 동작 변경.
