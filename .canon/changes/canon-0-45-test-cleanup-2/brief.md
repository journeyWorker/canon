---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "고칠 줄의 처분은 표 한 장이 정하고, 삭제할 시험 여덟과 flake의 수정이 구현자가 지켜야 하는 조건이다"
  - rule: "prose.length"
    section: "invariants"
    reason: "불변식마다 처분하는 시험과 줄을 열거해야 지운 것과 남긴 것이 한 목록으로 정해진다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변식마다 점검이 하나씩 있어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "시험과 파일 이름은 이 변경이 바꾸는 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "시험 이름, 파일, 시나리오 id는 JUnit 묶기가 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "점검은 파일과 명령의 철자를 그대로 말해야 구현자와 리뷰어가 같은 것을 돌린다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "시험 이름과 파일 열거가 한 문장이다"
  - rule: "prose.paragraph-length"
    section: "invariants"
    reason: "한 불변식이 한 항목이고 열거가 문장으로 센다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 시험 이름, 줄 번호, 값의 출처다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 점검이 읽는 값과 명령의 출처다"
---

# 시험이 문장을 단언하지 않는다: 남은 message·hint 문구 줄을 판정으로 고치거나 지우고, 문구를 못 박는 시험을 지운다

Source: 줄마다의 처분표 `/Volumes/Shelter/storage/journey-temp/projects/canon/2026-11-19-0.45/test-cleanup-2.md`(main `ccebdf1c1`, 0.44.0), 앞의 변경 `canon-0-43-test-cleanup`의 brief와 감사 `/Volumes/Shelter/storage/journey-temp/projects/canon/2026-11-17-0.43-plan/test-audit.md`

## Intent

0.43은 "어떤 시험도 사람이 읽는 문구를 바이트 단위로 고정하지 않는다"를 규칙으로 두고 스위트를 한 번 훑었다. 남는 시험 가운데 오류 `message`, `hint`, `effectNote`를 읽는 줄은 "기계 토큰 같다"며 하나씩 확인하지 않았다. 이 변경은 그 줄을 하나씩 읽은 결과를 적용한다.

- 범위: `crates/*/tests`와 `src`의 `#[cfg(test)]`에서 `message`, `hint`, `effectNote`를 읽는 단언 모양의 줄 331개(88개 파일)와 같은 시험의 줄 다섯. 0.43 메모의 "약 187줄"은 세는 규칙이 기록에 없어 재현되지 않는다. 331은 표의 규칙이 지금 찾는 값이다.
- 결과(표): 그대로 둘 기계 토큰 230줄(옵션, 명령 줄, id, 코드, 경로, 설정 키, 숫자, 필드의 존재), 단언이 아닌 줄 55(추출 도우미, 실패 메시지 인수), 기계 단언으로 고칠 줄 27, 지울 줄 13, 시험째 지울 줄 11(시험 8개).
- 시험 하나가 흔들린다: `write_lock`의 `a_call_that_found_the_lock_file_leaves_it_and_a_second_guard_keeps_what_the_first_made`가 부하에서 `Busy`로 실패한다. 같은 잠금 파일을 `drop` 직후 `Duration::ZERO`로 다시 잡는 가정이 원인이다. `flock`은 열린 파일 기술자에 속하고, 같은 프로세스의 다른 스레드가 `git`을 띄우면 fork된 자식이 exec 전까지 기술자 사본을 쥐어 잠금이 부모의 `drop`보다 오래 간다. 같은 파일의 `a_second_guard_in_one_thread_does_not_wait_and_the_last_drop_releases`는 이미 5초 대기로 같은 가정을 고쳤다. [추정] 코드를 읽어 얻은 원인이고 재현하지는 않았다.

## Category

refactor

## Invariants

- I1: canon의 동작은 바뀌지 않는다. 출력, 종료 값, 기록 형식, 오류 코드, 선언(`contractVersion` 1.6)이 같다. 바뀌는 것은 시험 코드, 시험 도우미, 두 시나리오의 문장, CHANGELOG 항목 한 줄이다. `src/`의 제품 문구는 바꾸지 않는다.
- I2: 표의 R 27줄은 표가 적은 대로 기계 단언으로 고친다: 경고는 `code`(`LEGACY_GUIDE_STALE`, `LEGACY_STATE_FOUND`), 비밀 검사 거절은 파일·id, 규칙 id, `line N`을 따로 `contains`로 보며 괄호와 쉼표는 단언하지 않는다, staleness 안내는 사유 이름(`base_missing` 등)과 시나리오 id만 보고 개수 서식은 보지 않는다(개수는 `canon-gate`의 구조체 단언이 맡는다). `starts_with`는 `contains`로, 이름 + 경로를 이은 서식은 두 토큰으로, `atip_l0.rs`의 직렬화 문자열 전체는 `hint`·`field`·`suggestions` 키가 없다는 단언으로 바꾼다.
- I3: 표의 D 13줄은 줄만 지운다. 시험은 다른 단언을 그대로 가진다. 줄은 문구, 메아리, 같은 계약의 중복, 옛 이름 훑기, 제3자(`serde_yaml`) 오류 문구, `Debug` 글자 고정, 바이트 고정이다.
- I4: 시험 여덟을 지운다. `hints.rs`의 둘(파일째), `hint_forms.rs`의 `atip_contract11_10_every_hint_in_the_sources_…`와 `…_the_rule_catches_prose_after_a_command_…`, `canon-gate/src/staleness.rs`의 `the_items_count_the_cells_by_reason_and_a_shallow_clone_is_pointed_at_unshallow`, `canon-ingest/src/plan_adapter.rs`의 `record_malformed_names_the_path_and_reason`와 `record_malformed_with_hint_carries_the_actionable_hint`, `safety.rs`의 `removed_destructive_spellings_exit_2_and_name_the_commit_form`이다. 이유는 소스 글자 훑기, 시험의 시험, 방금 넣은 인수의 되읽기, 같은 힌트를 `removed_028`과 `paging`이 이미 단언함이다. 이 시험만 쓰던 도우미도 지운다. `atip_contract11_10`의 나머지 셋(말뭉치 훑기, 바이너리 훑기, `atip_contract11.rs`의 시험)은 힌트 형태 계약(7.1.2)을 계속 지킨다.
- I5: ==**퇴역하는 시나리오는 없다.**== 지우는 시험 여덟 가운데 시나리오의 마지막 시험은 없고 모든 (주제, surface) 쌍이 `@case:failure` 시나리오를 그대로 가진다. 문장을 고치는 시나리오는 둘이다. `atip.contract11.10`은 "소스"를 읽는다는 말을 빼고, `context.capability.54`는 "옛 철자가 없다"는 절을 뺀다. 표의 R·D 줄이 속한 다른 시나리오의 문장은 구현자가 다시 읽고 지운 점검을 말하면 고친다. 찾은 것은 위 둘이다.
- I6: `write_lock`의 그 시험은 지우지 않고 고친다. `drop` 뒤 같은 잠금 파일을 다시 잡는 호출 하나(`hold_within(dir.path(), Duration::ZERO)`, `leave_no_trace` 앞)가 `Duration::from_secs(5)`를 기다린다. 파일의 다른 `ZERO` 호출은 새 임시 폴더의 첫 호출, 같은 스레드의 재진입, 파일을 지운 뒤의 호출이라 그대로 둔다. 시험이 지키는 것(거절된 호출이 앞선 호출이 만든 잠금 파일을 지우지 않는다, 둘째 보호자가 첫째가 만든 것을 지킨다)은 소비자에게 보이는 동작이다.
- I7: 이 변경은 시험, 스크립트, 도구를 더하지 않는다. 표에 없는 줄은 건드리지 않는다. 구현자가 읽다 새 문구 단언을 찾으면 같은 규칙으로 처분하고 표에 적는다.
- I8: 이 계획은 cargo를 돌리지 않았다. 표의 줄 번호는 main `ccebdf1c1` 기준이고 구현자가 시험 이름으로 다시 찾는다.

## Checks

- C1(I2, I3, I4): `rg -n 'contains\("\(line |starts_with\(&?format!\("parsing|"base_missing 1"|old_spellings\(&format|\.hint\(\)\.starts_with\("canon "\)' crates` 결과에 표가 R·D로 정한 줄이 하나도 없다.
- C2(I4): `crates/canon-cli/tests/hints.rs`가 없고 `rg -n 'fn (every_refusal_carries_a_hint|the_scanner_follows|the_items_count_the_cells_by_reason|record_malformed_names|record_malformed_with_hint|removed_destructive_spellings)' crates`가 비어 있다. `hint_forms.rs`는 시험 둘(말뭉치, 바이너리)을 가진다.
- C3(I5): `.feature`에서 `@atip.contract11.10`과 `@context.capability.54`의 문장에 위 두 표현이 없고, `canon gate check`가 위반 0이다. 시나리오 목록의 수는 그대로다.
- C4(I6): `a_call_that_found_the_lock_file_leaves_it_and_a_second_guard_keeps_what_the_first_made`가 canon-cli lib 시험을 `--test-threads` 기본값으로 연속 20회 돌려 모두 통과한다(샌드박스에서, 맥에서 돌리지 않는다).
- C5(I1): 샌드박스에서 `cargo clippy --workspace --all-targets -- -D warnings`와 `cargo test --workspace`가 통과하고, `canon schema --json`, `docs/cli/canon.cli.json`, 생성 문서의 diff가 없다.
- C6(I3): 시험 수는 이전보다 8개 줄고(R·D 줄은 시험을 줄이지 않는다) CHANGELOG의 Unreleased에 숫자(고친 줄, 지운 줄, 지운 시험, 고친 시나리오)와 flake의 원인 한 줄을 싣는다.

## Out of scope

- `stderr`·`stdout` 글을 문장 조각으로 `contains` 하는 줄(0.43 메모로 417줄). 이 변경은 `message`·`hint`·`effectNote`를 읽는 줄이다. 따로 계획한다.
- 제품 문구와 비밀 검사 거절에 `line`·`rule` 필드를 더하는 일(제품 변경이다).
- 봉투 모양(`message`는 문자열, `retryable`은 불리언)을 세 군데 이상이 보는 중복. 각 시험이 그것을 시나리오의 주제로 삼는다.
- 리뷰용 하네스, 변이 실행, 새 시험.

## Questions and assumptions

- owner 결정(Q1, 2026-11-19 대화): 주제 열한 곳에 이 변경을 채택한다(`atip-contract`, `atip-schema`, `context-capability`, `dispatch-manifest`, `evidence-attestation`, `finding-lifecycle`, `gate-trust-spine`, `review-attestation`, `skills-install`, `subject-lifecycle`, `tier-storage`). 줄이 고쳐지는 시험의 시나리오가 속한 주제다. 출시된 주제를 다시 출시할 때 변경 수준 리뷰 하나와 증거가 필요하다.
- owner 결정(Q2, 2026-11-19 대화): `(line N, rule)`의 `line N`은 기계 토큰(위치 표지)으로 보고 파일·id, 규칙 id, `line N`을 따로 단언한다. 괄호와 쉼표는 단언하지 않는다. 필드로 승격하는 것은 제품 변경이라 이 변경 밖이다.
- owner 결정(Q3, 2026-11-19 대화): `hints.rs`를 지운다. 모든 거절이 힌트를 가진다는 계약은 ATIP의 오류 등록부가 코드마다 `fix`를 요구하고 atip-deep이 그것을 검사하므로 거기서 지킨다. 거절마다의 힌트 존재 시험(`atip_reasons_eio`, `atip_codestored`, `one_name`, `spec028_c`, `reverify`)은 남는다.
- owner 결정(Q4, 2026-11-19 대화): `safety.rs`의 첫 시험은 `removed_028` 표의 행과 `paging`의 단언이 같은 토큰을 이미 보므로 지운다. `evidence vault` 같은 이름 바꿈을 보는 둘째 시험은 남긴다.

Approved: journey 2026-11-19T08:46:58Z — "오너가 채팅에서 질문 4개의 설명(비밀값 거절 메시지 테스트, hints.rs와 ATIP 오류 목록, subject 11개, safety.rs 중복)을 보고 '승인하고'로 직접 승인함(2026-11-19). 오너 지시로 에이전트 계획 리뷰는 돌리지 않음. 브리프 본문 전체는 읽지 않음." (relayed by planner planner-oco via chat) (code tree b4649db847f8df6a851d8d89b55fefb91e105373) (brief sha256:07313c7e8d2d40edef923c62f11e6c4ecfddd06e8c253d1335bfa7ffecd4f456)
