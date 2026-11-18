---
kind: chore
waive:
  - rule: "prose.length"
    section: "brief"
    reason: "시나리오 119개와 시험 범주 여섯의 처분, 병합 순서, 측정 방법이 모두 구현자가 지켜야 하는 조건이라 줄이면 구현자가 감사를 다시 읽어야 한다"
  - rule: "prose.length"
    section: "intent"
    reason: "시험 규칙과 현재 비용의 측정값이 이 변경의 근거다"
  - rule: "prose.length"
    section: "invariants"
    reason: "범주마다 시험과 시나리오의 처분을 id로 열거해야 지운 것과 남긴 것이 한 목록으로 정해진다"
  - rule: "prose.length"
    section: "checks"
    reason: "불변식마다 점검이 하나씩 있어야 하고 전후 측정의 조건을 적어야 한다"
  - rule: "prose.code-citation"
    section: "intent"
    reason: "시험 바이너리와 파일 이름은 측정이 붙은 대상의 철자 자체다"
  - rule: "prose.code-citation"
    section: "invariants"
    reason: "시험 이름, 파일, 시나리오 id는 JUnit 묶기가 대조하는 철자 자체다"
  - rule: "prose.code-citation"
    section: "checks"
    reason: "점검은 파일과 명령의 철자를 그대로 말해야 구현자와 리뷰어가 같은 것을 돌린다"
  - rule: "prose.sentence-length"
    section: "invariants"
    reason: "시나리오 id 목록과 파일 열거가 한 문장이다"
  - rule: "prose.sentence-length"
    section: "checks"
    reason: "점검 명령과 기대 결과의 열거가 한 문장이다"
  - rule: "prose.paragraph-length"
    section: "invariants"
    reason: "한 불변식이 한 항목이고 열거한 시험과 시나리오가 문장으로 센다"
  - rule: "prose.paragraph-length"
    section: "checks"
    reason: "한 점검이 한 항목이고 열거가 문장으로 센다"
  - rule: "prose.parenthetical"
    section: "invariants"
    reason: "괄호는 시험 이름과 시나리오 id, 값의 출처다"
  - rule: "prose.parenthetical"
    section: "checks"
    reason: "괄호는 점검이 읽는 값과 명령의 출처다"
---

# 시험이 소비자에게 보이는 버그만 잡는다: 문서·기록 문구, CI 배선, 시험의 시험, 같은 계약의 중복을 지우고 그 시나리오를 같은 변경에서 퇴역시킨다

Source: 시험 감사 `/Volumes/MoeLive/storage/journey-temp/projects/canon/2026-11-17-0.43-plan/test-audit.md`(main `5c4f46fab`, 0.42.0), CI 실행 43894845013(main 푸시, head `f76ab7e8`, Test 27분 26초), 승인된 `canon-0-42-test-cost`와 같은 0.43의 `canon-0-43-atip-0-3-2`, `canon-0-43-release-hardening`의 브리프

## Intent

CI의 offline 작업은 27.4분이고 `cargo test --workspace`가 20분 50초를 쓴다. 바이너리 240개의 실행 시간 합 1,245초 가운데 canon-cli의 시험 바이너리가 1,209초(시험 2,453개)다. 전체 시험은 약 4,100개이고 canon-cli의 시험 코드만 85.7k줄이다. `atip_*` 바이너리 35개(시험 475개)가 395초, 32%를 쓴다.

| 항목 | 값(CI 실행 43894845013) |
|---|---|
| offline 작업 | 27.4분: 빌드 2분 21초, `cargo test --workspace` 20분 50초, 세 호출자 단계 1분 50초, witness 32초, gate 23초 |
| `arg_rules` | 135초(시험 12개), 가장 큰 바이너리 |
| `session_kinds_removal` | 90초(시험 25개). 0.39에 없앤 기능이다 |
| 세 호출자 단계 | 110초. `canon-store`의 lib 시험(225개, 31초)이 셋 더 돈다(93초) |
| `grammar-mutation` 작업 | 별도 러너에서 2분 28초 |

이 시간과 줄의 상당 부분은 소비자에게 보이는 동작을 지키지 않는 시험에 간다. 릴리스마다 시험이 하나씩 쌓였다. CHANGELOG 항목에 표와 문장이 있는지 읽는 시험, 그 점검이 행을 빼면 실패하는지 보는 둘째 시험, 안내문과 웹사이트에 옛 이름이 없는지 훑는 시험, 워크플로와 `justfile` 글자가 같은 명령을 말하는지 읽는 시험, 선언된 값이 늘 때마다 고치는 개수 단언, 같은 계약을 바이너리 넷이 따로 단언하는 시험이다. 문장을 고쳐 쓰면 이런 시험이 실패하고 틀린 주장은 통과한다. atip 마이너를 올릴 때마다 시험 파일 열두 곳의 `"1.6"` 57줄을 고친다. 0.42의 시험 비용 변경도 심은 변경 열한 가지와 전후 측정 장치를 요구했고, 둘 다 사용자에게 보이는 버그를 잡지 않는다.

### 이 변경이 적용하고 리뷰어가 판단하는 규칙

owner의 시험 규칙이다. 이 변경은 스위트 전체에 한 번 적용하고, 리뷰어는 남기거나 더하는 시험마다 같은 규칙으로 판단한다.

- 시험은 소비자에게 보이는 그럴듯한 버그를 잡아야 한다. 리뷰어는 시험마다 "어떤 소비자 버그를 잡는가"를 한 문장으로 답할 수 있어야 한다. 답이 문구, 파일 글자, 개수, 다른 시험이면 지운다.
- 배선, 메아리, 소스 글자, 우연한 기본값, 동어반복, 던지지 않음만 보는 단언, 길이가 늘었음만 보는 단언을 시험하지 않는다.
- 문구나 구현을 지키는 시험은 지운다. 새 문구로 다시 못 박지 않는다.
- 계약은 한 번만 단언한다. 명세 동등은 `cli_spec`, 선언은 `atip_l0`, 바이트는 conformance에 두고 다른 시험은 프로세스 안의 선언된 문서를 읽는다.
- 시험의 시험을 두지 않는다. 검사기가 제품 기능인 경우(`brief check`, `atip check`)만 예외다.
- 리뷰만을 위한 하네스, 심은 변경 실행, 변이 실행을 두지 않는다. 이 변경도 그런 것을 더하지 않는다.
- owner의 말: "제발 바이트레벨 문구 고정같은건 하지말자. 진짜." 어떤 시험도 사람이 읽는 문구를 바이트 단위로 고정하지 않는다. 시험이 단언하는 것은 판정(종료 값, 오류 코드, 필드, 종류, 이름)이고 오류 `message`, `hint`, 도움말, 설명, CHANGELOG, 안내문의 문장이 아니다. 기계 필드와 문장이 섞인 바이트 비교는 기계 필드만 비교하도록 줄이고, 남는 시험 전체에서 문장을 단언하는 줄은 판정으로 고치거나 지운다(I14, I15).

시나리오 id는 JUnit 케이스 이름에 묶는 이름표이고 사양이 아니다. 시험을 지우면 그 시험에만 묶인 시나리오는 같은 변경에서 `.feature` 파일에서 지운다. `canon inventory sync`가 지운 시나리오에 묘비를 쓰고 게이트는 그것을 없는 것으로 읽는다.

## Category

refactor

## Invariants

- I1: canon의 동작은 바뀌지 않는다. 출력, 종료 값, 기록 형식, 생성 문서, 오류 코드, 선언(`contractVersion` 1.6)이 같다. 바뀌는 것은 시험 코드, 시험 데이터, `.feature` 시나리오, `test.yml`, `justfile`, `scripts/`, CHANGELOG, 지운 시험을 가리키던 주석 한 줄씩이다. 시나리오의 편집에는 퇴역(블록 삭제) 말고 시험을 건드리지 않는 태그 한 줄이 있다. `atip.optionvalues.16`의 `@subject:atip-contract`를 `@subject:atip-schema`로 바꾼다(I2). 예외가 둘이다. 하나는 `canon/skills/reference/canon-report-dashboard.md`의 한 절을 고치는 것(I11)이고 `canon skills show canon-report-dashboard`의 출력이 그 한 절만큼 달라진다. 다른 하나는 릴리스 검사기 `scripts/check-release-workflow-safety.py`의 규칙 하나(워크플로 단계의 안내 문장을 요구하는 규칙)를 지우는 것(I15)이다. 검사기는 CI의 점검이고 canon의 바이너리가 아니다. `crates/*/src`는 `#[cfg(test)]` 시험 코드(`selftest`의 개수 단언)와 주석만 바뀐다. `Cargo.lock`, `conformance/`의 사례와 기대 파일, 생성 명세는 바뀌지 않는다.
- I2: ==**시험을 지우면 그 시험에만 묶인 시나리오를 같은 변경에서 퇴역시킨다.**== 시나리오의 시험은 JUnit 묶기를 따라 이름이 시나리오 id(점은 밑줄)로 시작하는 시험이다. 시험이 하나라도 남으면 시나리오는 남고, 그 문구가 지운 점검을 말하면 문구를 고친다. 퇴역은 시나리오 블록(`# canon:` 출처 줄 포함)을 `.feature`에서 지우는 것이다. 정책 `require_cases: [failure]`는 주제가 출시될 때 그 주제가 가진 surface마다 실패 경로 시나리오 하나를 요구한다(`subject move <주제> --to shipped`). 그래서 퇴역 뒤에도 모든 (주제, surface) 쌍이 `@case:failure` 시나리오를 하나 이상 가져야 한다. 퇴역 목록을 그대로 지우면 세 곳이 실패 경로를 잃는다. `atip.check`와 `atip.codestable`은 `atip.check.40`과 `atip.codestable.06`을 남겨서 막는다(I3 D). 셋째, 주제 `atip-contract`의 `atip.optionvalues`는 `.14`(happy)와 `.15`(failure)를 지우면 edge인 `.16`만 남는다. `.15`는 CHANGELOG 점검의 점검이어서 남기지 않고 `atip.optionvalues.16`의 주제를 그 surface의 나머지 시나리오가 속한 `atip-schema`로 옮겨(태그 한 줄, 시험은 그대로) `atip-contract`가 그 surface의 시나리오를 갖지 않게 한다. `canon-0-43-atip-0-3-2`가 먼저 병합되어 `.16`의 문구를 고치므로 이 태그 줄은 그 위에서 적용한다. 지운 뒤 `.feature` 파일이 비지 않고, `canon format`이 깨끗하고, 퇴역 시나리오가 걸친 열두 주제마다 시나리오가 남는다. 게이트가 초록이 되는 조건은 I16이 정한다.
- I3: 범주별 삭제와 퇴역은 아래 목록과 같다. 시험 수와 시나리오 수는 main `5c4f46fab`에서 시험 이름과 파일 읽기로 센 값이고, 구현자는 같은 규칙으로 다시 센다. 목록에 없는 시험이 규칙에 걸리면 리뷰어가 짚는다. 합계 141과 119는 문장 단언 훑기(I14)를 더하기 전의 값이고 `canon-0-43-atip-0-3-2`가 맡은 시험과 시나리오(I9)를 뺀 값이다. 훑기로 단언이 하나도 남지 않아 시험이 통째로 사라지면 그 시험과 시나리오(I2)를 증거에 적고 합계에 더한다. 훑기가 끝난 합계를 CHANGELOG에 싣는다.

  | 범주 | 규칙 | 시험 | 퇴역 시나리오 |
  |---|---|---:|---:|
  | A | CHANGELOG 문구를 읽는 시험 | 39 | 38 |
  | B | 안내문, README, 스킬, 웹사이트 문구를 읽는 시험, 소스 글자를 읽는 시험 | 10 | 8 |
  | C | 옛 이름과 "없어졌다" 훑기, 0.35 골든 표, 퇴역 기능의 잔재 | 21 | 19 |
  | D | 위 시험의 점검을 시험하는 시험 | 52 | 40 |
  | E | CI·justfile 배선 메아리 | 1 | 1 |
  | F | 같은 계약을 두 번 단언하는 시험의 병합 | 15 | 13 |
  | G | 선언된 값이 늘었음만 따라가는 개수 | 0(단언만 고침) | 0 |
  | 합계 | | 138 | 119 |

  그 밖에 0.42 장치의 시험 둘(`arg_rules`), 변이 시험 하나(`grammar_t15_mutation`)를 지운다(I7, I8). 총 141개이고 시나리오에 묶이지 않는다. 시험 총수 약 4,100개의 3.4%다.
- I3 A: CHANGELOG 문구 시험은 `atip_claim14.rs`, `atip_claim15.rs`, `atip_claim16.rs`, `atip_canoncursor.rs`, `atip_planjcs.rs`, `atip_optionforms.rs`, `atip_optionvalues.rs`, `atip_provideroption.rs`, `atip_schemashape.rs`, `atip_codestored.rs`, `atip_codesplit.rs`, `atip_codenames.rs`, `atip_reasons.rs`, `atip_incomplete.rs`, `atip_codeexit.rs`, `atip_codestable.rs`, `atip_claiml2.rs`(0.35 절의 표와 증거 목록을 읽고 소스에서 `#[test]`를 찾는 `_11`, `_13`)와 `approval_tree.rs`, `artifacts_prune.rs`, `artifacts_zstd.rs`, `code_scopes.rs`, `finding_closer.rs`, `git_boundary.rs`, `git_env_allowlist.rs`, `staleness_scope.rs`, `write_durability.rs`, `secret_scan_changelog.rs`, `change_state_docs.rs`, `session_ingest_removed.rs`, `session_kinds_removal.rs`, `ci_evidence_docs.rs`, `brief_multi_approve.rs`에 있다. 시나리오: artifacts.object-zstd.13, artifacts.prune.14, artifacts.prune.26, atip.canoncursor.13, atip.claim14.15, atip.claim15.14, atip.claim16.13, atip.claiml2.11, atip.claiml2.13, atip.codeexit.10, atip.codenames.08, atip.codesplit.10, atip.codestable.01, atip.codestable.02, atip.codestable.04, atip.codestable.11, atip.codestable.12, atip.codestored.10, atip.daterefusals.04, atip.incomplete.12, atip.optionforms.13, atip.optionvalues.14, atip.planjcs.12, atip.provideroption.14, atip.reasons.09, atip.schemashape.11, dispatch.secret-scan.28, finding.closer.11, gate.approval-tree.12, gate.ci-run.13, gate.code-scopes.12, gate.git-boundary.15, gate.git-env.08, gate.staleness-scope.13, ingest.removal.13, skills.install.73, tier.durability.15, tier.retired.18.
- I3 B: skills.install.11(스킬 설명의 낱말과 길이, 가이드 첫 절 이름), atip.codestable.09, atip.codestable.10, atip.codestable.13, atip.helpjson.09, atip.provideroption.11, ingest.removal.12, tier.retired.19가 퇴역한다. 시나리오가 없는 시험은 `finding.rs`의 `no_shipped_finding_help_or_doc_asserts_a_hand_typed_count`(소스 두 곳을 `include_str!`로 읽는다)다. `ingest.removal.02`는 시험 둘 가운데 "생성 문서에 옛 줄이 없다" 하나만 지운다. `selftest --suite session-ingest`를 거절하는 시험이 남아 시나리오가 남는다.
- I3 C: atip.claim14.11, atip.claim15.12, atip.claiml2.02, atip.codeexit.11, atip.codenames.02, atip.codenames.05, atip.codenames.06, atip.codestable.07, atip.codestored.09, atip.contract11.13, atip.crate12.13, atip.l0.27, atip.midway.09, atip.midway.11, atip.skillsshow.09, tier.retired.09, tier.retired.10, tier.retired.11, tier.retired.16. `tier.retired.15`, `.17`, `.20`은 업그레이드된 저장소의 동작 시험이어서 C에서 뺀다(I5). `old_codes/mod.rs`와 `fixtures/codes-0.35.tsv`(0.35의 코드 179개)는 사용처가 모두 사라지므로 함께 지운다. 남는 시험이 아직 쓰는 줄(`atip_codestable_05`의 `ADDED_AFTER_0_36`)은 그 시험에서 걷어낸다. `old_names.rs`는 시험이 모두 사라지므로 파일을 지운다. `atip_names.rs`도 같다.
- I3 D: artifacts.object-zstd.14, artifacts.prune.15, artifacts.prune.27, atip.claim14.12, atip.claim14.16, atip.claim15.13, atip.claim15.15, atip.claim16.14, atip.claiml2.12, atip.claiml2.14, atip.codenames.07, atip.codenames.09, atip.codesplit.11, atip.codestable.03, atip.codestable.08, atip.codestable.14, atip.codestable.15, atip.codestored.14, atip.crate12.15, atip.daterefusals.05, atip.helpjson.10, atip.incomplete.13, atip.midway.14, atip.optionforms.14, atip.optionvalues.15, atip.planjcs.16, atip.provideroption.15, atip.schemadoc.14, atip.schemashape.12, atip.skillsshow.11, change.drop.25, dispatch.secret-scan.29, finding.closer.12, gate.approval-tree.13, gate.ci-run.14, gate.code-scopes.13, gate.git-boundary.16, gate.git-env.09, gate.staleness-scope.14, tier.durability.16. 시나리오가 없는 시험은 `grammar_t10_typing.rs`의 `the_walk_recognises_an_untyped_member`와 `finding.rs`의 `the_count_guard_catches_the_class_and_spares_the_indices`다. 일부만 지우고 시나리오가 남는 것은 atip.schemadoc.13(옛 이름 훑기 둘), atip.l0.23(소스 훑기가 제대로 읽는지 보는 시험 하나, 훑기 자체는 I3 F), atip.reasons.08(소스 훑기 시험은 I3 F대로 합쳐 지운다), atip.contract11.04(소스를 훑는 시험 하나)이다. 이 넷은 남는 시험을 이름으로 증거에 적고 문구에서 지운 시험의 말을 뺀다. `change.drop.24`도 같다(A의 시험 하나, 다른 시험이 남는다).
- I3 D에서 남기는 것: 검사기가 계약의 검증기인 경우는 지우지 않는다. `arg_rules.rs`의 문서 `Value`에 규칙을 심는 시험(421, 443, 459줄)과 `atip_claim14_05-10`, `atip_claim15_03-09`, `atip_claim16_03-10`은 남는다. `atip_check_40`과 `atip_codestable_06`은 시험의 시험 모양이지만 남긴다. 정책이 surface마다 실패 경로 시나리오를 요구하고 이 둘이 `atip.check`와 `atip.codestable`의 유일한 실패 경로 시험이어서다(I2). 둘 다 판정만 단언한다. `atip_check_40`은 `drift`가 심은 낡은 등록부를 알아채는가를, `atip_codestable_06`은 이름 규칙이 어긴 코드를 거절하는가를 본다. 새 시나리오와 새 시험은 더하지 않는다. `atip_check_39`도 남는다. 감사의 D 목록은 `atip_check.rs:115`(40)를 지우라고 적었으나 이 변경은 정책 때문에 지우지 않는다.
- I3 E: gate.git-boundary.14(`just lint`와 clippy 작업과 코딩 규칙 파일의 글자를 읽는 시험) 하나다. CI 작업과 레시피의 명령 줄, `--contract`, 태그를 읽는 배선 시험(`atip_check_43`, `atip_claim14_13`, `atip_claim14_14`, `atip_claim15_11`, `atip_claiml2_07`)과 `atip_optionforms_15`, `atip_optionvalues_16`의 `--contract` 읽기는 `canon-0-43-atip-0-3-2`가 맡는다(I9). 이 변경의 E 목록에 없다.
- I3 F: 합치는 계약은 다섯이다.
  - 선언: `atip_l0_26`이 하나의 선언 시험이다. 선언한 마이너 == `DECLARED_MINOR` == 픽스처의 `contractVersion` == 명세의 `contractVersion`을 프로세스 안의 `canon_cli::spec`에서 읽는다. 명세 동등 줄(`on_disk == render`)과 `envelopes > 30`은 뺀다(G). atip.claim14.01, atip.claim15.01, atip.claim16.01, atip.claiml2.01, atip.crate12.12가 퇴역하고, 이 시험 중 선언 밖의 단언(공유 옵션 등)이 있으면 그 단언만 남은 시험이 이미 다른 시험(`atip_schemashape`, `atip_schemadoc`)에 있는지 확인하고, 없으면 그 한 줄만 남긴다. 남은 `atip_claim14/15/16`의 시험은 명령마다 `canon schema <명령> --json`을 띄우는 표면(`SURFACE`) 대신 프로세스 안의 문서를 읽는다. 남은 `"1.6"` 글자 줄은 `DECLARED_MINOR`로 바꾼다. 릴리스가 시험에서 고치는 것은 상수 하나와 픽스처 하나다.
  - 명세 동등: `cli_spec.rs`가 한 곳이다. `spec028_c.rs`의 같은 비교(`context.capability.28`)를 퇴역시킨다. `schema_doc.rs`의 명령마다 바이트 대조(`atip_schemadoc_13` 첫 시험)는 한 번 남기고 `commands.len() == 81`만 뺀다(G).
  - 안내문 명령이 파싱된다: `docs_lint.rs`가 한 곳이다. atip.skillsshow.07, atip.skillsshow.08이 퇴역한다.
  - 등록부와 방출 소스: `schema_codes.rs`(소비자가 `canon schema --errors`를 읽는다)와 `atip_reasons_02`가 남는다. 종료 값 소스 훑기는 하나를 남긴다. `exit_registry.rs`의 `atip_l0_23_no_call_site_overrides_an_exit_and_no_literal_exit_is_outside_0_1_2`는 리터럴 `ExitCode::from(3)`, `process::exit(n)`(n이 2보다 큼), 시험이 닿지 않는 경로의 `.exit =`와 `.retryable =` 덮어쓰기를 막는 유일한 점검이다. `schema_codes`는 코드와 명령의 귀속을, `grammar_t5_errors`는 등록 안 된 코드를 볼 뿐이다. `atip_reasons_08`의 `.exit =`와 `.retryable =` 훑기는 이 시험에 합치고 그 시험은 지운다. 이 소스 글자 시험은 규칙의 예외로 남기고 오류 문구는 단언하지 않는다. 훑기가 제대로 읽는지 보는 `atip_l0_23_the_scan_reads_literals_and_leaves_variables_alone`는 시험의 시험이어서 지운다. 지우는 나머지는 `cli_error.rs`의 `the_code_registry_is_unique_sorted_and_matches_the_codes_the_sources_construct`, `atip_contract11_04`의 소스 훑기, `atip_codenames_02`(I3의 C와 D)다.
  - 픽스처 모양: atip.claiml2.09, atip.claiml2.19는 `atip` 작업이 같은 픽스처를 읽어 실패하므로 지운다.
  - conformance와 Rust의 중복: 말뭉치의 사례와 기대 파일은 건드리지 않는다. `atip_batchpointers`의 `_07`, `_08`, `_09`는 같은 파일 안의 `_03`, `_04`와 같은 호출의 단언이어서 지우고(atip.batchpointers.07, .08, .09), `atip_helpjson`, `atip_inputorder`, `atip_readwrites`는 말뭉치 사례가 같은 호출의 종료 값, 코드, 필드를 이미 기계 값으로 얼린 단언을 줄에서 뺀다. 말뭉치가 대신 얼렸다고 인정하는 것은 JSON 단계의 산문이 아닌 값과 글 모드에서 남는 기계 토큰 줄뿐이다(I14). 오류 `message`나 `hint`의 글을 단언하던 줄은 말뭉치로 옮기지 않고 지운다. 남기는 것은 말뭉치가 말할 수 없는 것(쓰지 않았다, 홈과 임시 폴더가 그대로다, 파일 차이, 한 상태에서 이어지는 호출)이다. 단언이 하나도 안 남는 시험만 지우고 그 시나리오를 퇴역시킨다. 후보 시나리오는 atip.helpjson.01, .02, .04, .05, .06이고 최대 다섯이다. 빼고 남은 시험이 있는 시나리오는 남는다.
- I3 G: 지우는 시험은 없고 단언을 고친다. 정확한 개수는 `!is_empty()`로 바꾸고, 빈 훑기를 막는 `> 60`, `> 300`류는 `!is_empty()`로 줄인다. 남는 시험의 위치는 `schema_doc.rs`(81), `removed_028.rs`("67곳", `rows.len() == 14`), `codes_names.rs`(`RENAMED.len() == 50`), `atip_codeexit_01.rs`(22, 21), `atip_schemashape.rs`(`ROLES_0_37.len() == 78`), `atip_crate12.rs`(12, 13), `atip_optionforms.rs`(12), `ledger_migrate.rs`(34), `ledger_naming.rs`(14), `src/selftest.rs`의 시험(10)이고, 그 밖의 `atip_*`의 열쇠 훑기 한계도 같다. 시나리오 문구가 개수를 말하면(예: 81개 명령 문서) 개수를 빼고, 그 시나리오는 I16의 새 리뷰 목록에 든다.
- I4: 남기는 시험의 목록은 감사의 남길 목록이다. `canon schema` 출력(`schema_doc` 규칙·모양, `schema_examples`, `atip_schemashape`, `atip_check_39`, `cli_spec`, `docs_generated`), atip 계약(`atip_l0`, `atip_canoncursor_01-12,14-17`, `atip_claim15_03-09`, `atip_claim16_03-10`, `atip_claim14_05-10`, `atip_helpjson`, `atip_skillsshow_03,05,06`, `atip_check_order`, `atip_constraints`, `atip_validate`, `atip_codeexit_01`, `atip_reasons_01-08`, `grammar_t12_refusal`), 힌트와 철거된 철자(`removed_028` 1-4, 8, `docs_lint` 500-575, `atip_l0_12/14`), conformance와 그 사례 117개, 릴리스 공급망 파이썬 시험(`scripts/test-release-workflow-safety.py`, `scripts/test-check-release-binary.py`), `packages/cli`와 platformer 시험, `brief_prose`, `brief_format`, `hermetic_env`, `grammar_t9_witness*`. 이 변경은 이것들을 지우거나 줄이지 않는다. 예외는 문장을 단언하는 줄이다. 바이트 비교의 산문 부분을 빼는 것, 남는 시험 전체의 문장 단언을 판정으로 고치는 것(I14), 릴리스 검사기 시험의 문구 단언을 판정으로 고치는 것(I15)이다.
- I5: `session_kinds_removal.rs`는 소비자가 겪는 동작의 시험으로 줄인다. 남기는 것은 퇴역 키가 읽히고 경고하는 시험(`tier_retired_01` 둘, `02`, `03`), `canon migrate`가 퇴역 키 줄만 지우고 되돌리는 시험(`04`, `05`, `06`), `canon purge --kind session|event`(`07` 둘, `08`), purge 미리보기(`tier_purge_preview_01-03`과 `01`의 둘째), 퇴역 키를 이름으로 대는 거절(`tier_retired_keys_01-02`), 업그레이드된 저장소의 시험 셋이다. 업그레이드된 저장소의 시험은 `kind=session/`과 `kind=event/` 디렉터리의 읽을 수 있는 파일, 깨진 파일, 빈 파일, 읽을 수 없는 본문이 있어도 `status`, `gate check`, `inventory sync`, `ledger compact`가 읽고 그 파일을 한 바이트도 바꾸지 않는다는 `tier_retired_15`와 `tier_retired_20`, 옛 틀에서 `init --check`가 종료 0과 경고로 답한다는 `tier_retired_17`이다. 이것들을 지우는 다른 시험은 없다. 모두 19개다. 지우는 것은 I3의 A(`18`), B(`19`), C(`09`, `10`, `11`, `16`)의 여섯이다. 감사는 남길 것을 "약 8개"로 적었다. 그 수는 `tier_retired_01-08`만 센 것이고 purge 미리보기와 퇴역 키 거절과 업그레이드된 저장소의 시험은 소비자 계약이라 남긴다.
- I6: "unit tests under three callers" 단계와 `just test-callers`는 `cargo test -p canon-cli --lib`만 돈다(`--workspace` 대신). `CANON_ROLE`, `CANON_ACTOR`를 읽는 시험은 canon-cli에만 있다(canon-model에는 주석 한 줄뿐이다). 주 단계가 다른 크레이트의 lib 시험을 호출자 없이 이미 돌린다. 이 말을 단계 위 주석과 레시피 주석에 한 줄로 적는다. 다른 단계는 바뀌지 않는다.
- I7: 0.42의 시험 비용 장치는 표와 시험 하나만 남긴다. 표 `rule_calls.jsonl`, `rule_support/calls.rs`, `just rule-calls`, `just rule-calls-check`(릴리스 전 한 번)은 남는다. `arg_rules.rs`의 `the_stored_rule_calls_are_the_ones_the_search_finds_now`가 신선도 시험 하나다. `a_document_the_table_does_not_describe_is_refused_by_the_table`와 `the_drift_check_fails_on_each_planted_change`는 지운다. 줄의 입력 요약에서 탐색기 소스 글자(`calls.rs`가 `search.rs`와 `samples_*.rs`를 `include_str!`로 읽어 해시하는 부분과 `World`의 `sources`)를 없애고, 요약은 명령의 규칙, 필수 인수 이름, 인수별 후보 값, 예시 단어, `atip` 리비전만 해시한다. 표는 한 번 다시 만들어 커밋한다(`just rule-calls`). `canon-0-43-atip-0-3-2`도 `atip` 리비전이 바뀌어 같은 표를 다시 만든다. atip 변경이 먼저 병합되므로 이 변경은 그 표 위에서 한 번 더 다시 만들고, 요약 값이 바뀌므로 모든 줄의 `digest`가 바뀐다. 호출 내용은 같다. 탐색기 로직이 바뀐 경우는 릴리스 전 `rule-calls-check`가 잡는다. 이 변경은 표의 읽는 쪽과 `atip_check_order`, `arg_rules` 시험 본문을 바꾸지 않는다.
- I8: `grammar_t15_mutation.rs`, `scripts/check-mutations.sh`, `test.yml`의 `grammar-mutation` 작업을 지운다. `scripts/ci-strip-workspace-build.sh`의 머리 주석(그 작업이 만드는 복사본을 말하는 문장)을 고친다. T1-T14 문법 시험은 남는다.
- I9: 같은 0.43의 `canon-0-43-atip-0-3-2`가 맡은 시험과 시나리오는 이 변경이 하지 않는다. 그 변경이 없앤다: `atip_check_43`, `atip_claim14_13`, `atip_claim14_14`, `atip_claim15_11`, `atip_claim16_11`, `atip_claim16_12`, `atip_claiml2_07`과 그 시나리오 atip.check.43, atip.claim14.13, atip.claim14.14, atip.claim15.11, atip.claim16.11, atip.claim16.12, atip.claiml2.07. 그 변경이 고친다: `atip_claim15_10`(픽스처와 트리의 선언 단언만 남긴다)과 `atip_optionforms_15`, `atip_optionvalues_16`(시험과 시나리오는 남기고 `justfile`을 읽는 `--contract` 한 줄씩만 뺀다). 이 시험과 시나리오는 이 변경의 A-F 수와 위 목록에 들어 있지 않고 이 변경은 시험을 손대지 않는다. 이 변경이 하던 `atip_claim14_13`, `atip_claiml2_07`의 삭제와 시나리오 둘이 그 변경으로 넘어갔고(시험 2개, 시나리오 2개), `atip_optionforms_15`, `atip_optionvalues_16`은 두 변경 어디에서도 지우지 않고 남는다. 이 변경이 하는 `atip.optionvalues.16`의 주제 태그 변경은 그 시나리오의 문구를 건드리지 않는다.
- I10: 병합 순서는 atip → 이 변경 → release-hardening이다. 이유는 아래 질문과 가정의 "병합 순서"에 있다.
- I11: 안 돌리는 시험(Q2 기본): `packages/dashboard/test/panel-copy.test.ts`는 지운다. 그 파일을 가리키던 주석(`crates/canon-report/src/{render,marts,snapshot}.rs`, `crates/canon-model/src/records.rs`, `crates/canon-cli/tests/finding.rs`, `packages/dashboard/index.html`, `packages/dashboard/src/panels/review-{rounds,totals}.ts`, `packages/dashboard/test/smoke.test.ts`)와 `canon/skills/reference/canon-report-dashboard.md` 20줄의 한 절은 "시험이 이 문장에 묶는다"를 빼고 고친다. 가이드 문장 하나가 바이너리의 `canon skills show canon-report-dashboard` 출력에서 한 절 줄어든다. 동작이 아니고 그 문서의 문장 하나다. 나머지 안 돌리는 시험(`scripts/test-check-knowledge.py`, `evals/test_*.py`, 다른 `packages/dashboard/test/*`)은 건드리지 않는다.
- I12: 이 변경은 시험, 스크립트, 도구를 더하지 않는다. 측정은 CI 로그에서 읽고, 읽는 한 줄 명령은 PR 설명에 적고 저장소에 두지 않는다.
- I13: 줄어든 시간은 CI의 측정으로 말한다. 로컬 시간은 근거가 아니다. 바이너리마다, 단계마다 전후 값이 있다(Checks).
- I14: 바이트 비교는 기계 계약만 비교하고 사람이 읽는 문장은 비교하지 않는다. 감사의 남길 목록에서 바이트 단위로 비교하는 시험을 하나씩 읽은 결과는 아래 표와 같다. 섞인 셋은 질문과 가정의 결정(owner의 위임)을 따른다. 기대 파일, 명세 파일, 생성 블록은 지금처럼 전체 출력으로 만들고 검토하며(`just conformance`, `just docs`, `CANON_BLESS_CLI_SPEC`), 비교하는 쪽 코드만 산문을 뺀다. 사례와 기대 파일, `docs/cli/canon.cli.json`, 생성 블록의 diff는 없다.

  | 시험 | 지키는 기계 계약 | 산문 | 처분 |
  |---|---|---|---|
  | conformance 117개 사례의 재생(`conformance_corpus_replays_byte_exact`) | 종료 값, JSON의 산문 위치를 뺀 모든 값(필드 이름, 코드, `reason`, `field`, `expected`, `received`, 사용자 데이터 포함), 글 모드의 기계 토큰 | 아래 산문 위치의 문장 | 섞임. 파싱해 산문 위치를 비운 값을 비교하고, 글 모드는 산문을 지운 줄을 비교한다(아래). 이름은 `conformance_corpus_replays`로 고친다(묶는 증거가 없다) |
  | `cli_spec`의 `the_checked_in_spec_is_the_generated_one` | 명령, 옵션, 값 종류, 규칙, 출력 스키마(필드 이름 포함), 예시의 호출과 값 | 아래 산문 위치의 문장 | 섞임. 파싱해 산문 위치를 비운 값을 비교한다 |
  | `docs_generated`의 `every_generated_block_is_what_the_spec_renders` | 블록 종류마다 아래에 적은 기계 내용 | 블록 종류마다 아래에 적은 산문 | 섞임. 블록 종류별로 기계 내용만 비교한다 |
  | `atip_schemadoc_13`(첫 시험) | 바이너리가 찍는 명령 문서가 프로세스 안 선언 문서와 같다 | 없음. 같은 생성기의 두 출력이고 고정 문구가 없다 | 그대로 |
  | `schema_examples`, `atip_schemadoc_05`, `atip_schemashape`, `atip_check_39` | 예시가 실행된다, 기본값이 `--help`와 같다, 모양, 등록부의 code·exit·retryable | 없음 | 그대로 |
  | `atip_helpjson_08` | 종료 0, JSON이 아님, 두 명령 이름이 나온다 | 첫 줄이 상수 `HELP_POINTER`와 같다(문장의 메아리) | 첫 줄 단언만 지운다 |
  | `removed_028` 1-4, 8, `docs_lint`, `atip_l0_12`, `atip_l0_14` | 종료 2, 대체 철자가 `expected`의 첫 항목, 안내문과 `meta.next`의 명령이 파싱된다 | 없음. 힌트는 `canon <대체 명령>`을 담는지만 본다 | 그대로 |

  산문 위치는 JSON 경로로 적는다. `*`는 배열의 모든 원소이고 이 경로의 문자열만 비운다. 같은 이름의 멤버여도 다른 위치는 비교한다. 이 목록은 `cli_spec`과 conformance가 같이 쓴다.
  - 봉투: `error.message`, `error.hint`, `error.details[*].message`, `error.details[*].hint`, `data.items[*].error.message`, `data.items[*].error.hint`, `meta.next[*].reason`, `meta.warnings[*].message`, `meta.warnings[*].hint`.
  - 읽기 답: `data.findings[*].hint`(`brief check`), `data.effectNote`, `data.warning.meaning`, `data.warning.fix`.
  - 스키마 문서(`canon schema …`의 `data`와 `docs/cli/canon.cli.json`): 명령과 트리 항목의 `summary`, `effectNote`, 옵션·피연산자·제약의 `help`, 검증기의 `description`, `removed[*].note`, 등록부와 경고의 `errors[*].meaning`, `errors[*].fix`, `errors[*].reasons[*].meaning`, `errors[*].reasons[*].fix`, `warnings[*].meaning`, `warnings[*].fix`, `exitTable[*].meaning`, JSON 스키마 노드의 설명 주석인 `description`과 `title`의 문자열.
  - 필드 이름과 사용자 데이터는 산문이 아니다. `properties` 사전의 열쇠로 나타나는 `message`, `hint`, `title`, `description`, `summary`와 `data.items[*].data.title`, `data.records[*].title`, `data.records[*].status_override.reason`, `data.input.example.*`, 오류의 `reason`(`UNKNOWN_KEY` 같은 코드)은 그대로 비교한다. 그래서 `ErrorBody`에서 `message` 필드를 빼거나 입력 스키마에서 `title`을 빼면 실패한다.
  글 모드(조건 없는 stdout과 stderr의 글)는 산문을 지운 뒤 나머지를 비교한다. 지우는 것은 `canon: CODE:` 뒤의 문장(코드는 남긴다), `hint:` 줄, `advisory:`와 `warning:` 줄의 문장, ` — ` 뒤의 문장, `next:` 줄의 `# …` 이유다. `field:`, `expected:`, `problem:` 줄의 값, `next:`의 명령 줄, 경로, id, 개수, 위반 종류, 그 밖의 줄은 그대로 비교한다. 산문을 지운 뒤에도 일반 낱말이 섞인 줄(`1 bound file(s) … (0 newly stored)`)은 기계 토큰(숫자, 경로, id, `키=값`, 백틱 조각, 대문자 코드, 하이픈 위반 종류)의 열로 줄여 비교하고 일반 낱말은 비교하지 않는다. 사례 117개 가운데 83개는 `--json` 단계가 없으므로 이 글 모드 비교가 그 사례들의 계약이다.
  `docs_generated`는 블록 종류마다 기계 내용을 적는다.
  - `commands-<역할>`(역할 가이드): 블록 안의 `usage:` 줄, 예시 호출 줄, 펜스와 구분 줄. 뺀다: 앞의 `# …` 요약 주석 줄과 첫 펜스 앞의 도입 문장.
  - `cli-<그룹>`(웹사이트 영어와 한국어): 표의 모든 칸이다. `Command`, `Operands`, `Required options`, `Example` 칸 전부와 `Effect` 칸의 효과 낱말(`read:`, `write:` 앞부분). 뺀다: `Effect` 칸의 효과 낱말 뒤 요약 문장과 표 앞의 도입 문장.
  - `removed`, `removed-031`(CHANGELOG의 철거된 철자 표): 앞의 두 열(옛 철자, 새 철자)과 행의 순서. 뺀다: `What changed` 열.
  남는 시험 전체의 문장 단언도 이 변경에서 판정으로 고치거나 지운다. 시험 코드만 바꾸고 `src/`의 제품 문구는 바꾸지 않는다. 범위는 세 가지다.
  - 오류 `message`, `hint`, `effectNote`, 경고 `message`, `meta.next[].reason`을 읽어 `contains`나 `==`로 문장을 대조하는 줄. 지금 `crates/canon-cli/tests`에 81개 파일 319줄(`brief_format`, `dashboard_briefs`, `spec030_dispatch_demo`, `cli_ids`, `spec_root`, `context_pack_secret_scan`, `brief_prose`, `atip_reasons_eio` 등)이고, `src/`의 `#[cfg(test)]` 모듈에 `.message`나 `.hint`를 `contains`로 대조하는 줄이 30줄 이상(`dispatch`, `removed`, `evidence`, `ci_env` 등) 있다.
  - stdout과 stderr의 글을 문장 조각으로 `contains` 하는 줄. 지금 62개 파일 417줄이다. 이 중 오류 코드, `field: --옵션` 줄, 게이트 위반의 종류와 id, 명령 줄, 경로처럼 기계 토큰을 찾는 줄은 남는다. 문장 조각을 찾는 줄만 고친다.
  - 이 변경이 이미 고치는 파일이 아닌 시험(`brief_prose`, `brief_format` 포함). 두 파일은 제품인 검사기를 시험하므로 규칙 id, 섹션, 줄을 단언하고 `message` 글은 단언하지 않는다.
  고치는 법은 셋이다. 코드, 필드, 종료 값, 종류 단언이 이미 있으면 문장 줄을 지운다. 문장이 유일한 증거였으면 걸린 값을 낱말로(어긋난 옵션 이름, 오류 코드, 경로, 시나리오 id) 단언한다. 기계로 확인할 것이 남지 않으면 그 단언을 지우고, 시험에 단언이 남지 않으면 시험을 지우고 그 시나리오를 퇴역시킨다(I2). 줄 수는 이 계획이 `rg`로 센 값이고 일부는 이 변경이 지우는 시험 안에 있다. 구현자가 다시 센다.
- I15: 릴리스 검사기 시험도 같은 규칙을 따른다. `scripts/test-release-workflow-safety.py`와 `scripts/test-check-release-binary.py`가 오류 문구를 찾는 단언(`any("…" in error)`, `assertIn("…", errors[0])`, 앞 파일 95줄과 뒤 파일 25줄)은 판정으로 고친다. 검사기가 변경 입력을 거절하는가(오류 목록이 비지 않는가, 종료 값이 0이 아닌가)와 깨끗한 입력을 통과시키는가만 단언하고, 어느 입력이 어느 규칙에 걸렸는지의 문장을 단언하지 않는다. 바이너리 검사기 시험이 무엇이 걸렸는지 가려야 하면 걸린 값(`libc.so.6`, `VCRUNTIME140.dll`, `11.0`)만 낱말로 본다. 문장 조각(`machine is aarch64, expected x86_64`, `floor rose`)은 보지 않는다. 검사기 규칙이 워크플로 단계의 안내 문장을 요구하는 것(`recovery=true and dry_run=false`를 담아야 한다는 규칙)은 문구를 고정하는 규칙이므로 규칙과 그 시험(`test_rejects_a_guidance_that_forgets_dry_run_false`)을 지운다. `canon-0-43-release-hardening`이 고치는 규칙(`paths`와 `scope`, 게시 단계의 `if:`와 환경변수, `npm whoami`, `TARBALL`)의 시험은 그 변경이 판정으로 고치므로 이 변경은 건드리지 않는다.
- I16: 남는 시나리오의 문구, 태그, 묶인 시험 집합을 바꾸면 그 시나리오는 증거를 다시 묶은 뒤 새 시나리오 리뷰를 받는다. 시나리오 리뷰는 시나리오의 문구, 태그, 묶인 시험 이름의 집합에 묶이고 그것이 바뀌면 낡는다. 정책 `require_review: {}`는 범위가 `[verifying, shipped]`이므로 `canon gate check`는 `shipped` 주제의 낡은 시나리오 리뷰를 `unreviewed-promotion`(`stale review … its spec changed`)으로 낸다. 변경 단위 리뷰(`canon review change`)는 이것을 풀지 못하고 변경 단위 리뷰가 없다는 위반만 푼다(plan review r3의 scratch 시험). 그래서 이 변경이 `shipped` 주제의 시나리오를 바꾸는데도 게이트가 변경 리뷰와 증거만으로 초록이 되지 않는다.
- I16 목록: 계획이 센, 새 리뷰를 받아야 하는 시나리오 열세 개다. 태그가 바뀌는 하나는 atip.optionvalues.16이다. 시험 일부만 지워 케이스 집합이 줄어드는 여섯은 atip.contract11.04, atip.l0.23, atip.reasons.08, atip.schemadoc.13, change.drop.24, ingest.removal.02이다. 문구에서 개수를 빼는 시나리오(I3 G)는 atip.schemadoc.01, atip.schemadoc.09, atip.claim16.03, atip.claim16.09, atip.constraints.07, atip.daterefusals.01이다(atip.schemadoc.13은 위와 같다). `canon-0-43-atip-0-3-2`가 문구나 태그를 바꾸는 시나리오(atip.optionvalues.16, atip.optionforms.15, atip.claim15.10)도 같은 의무가 있다. 그 변경이 먼저 병합되므로 이 셋은 두 변경의 편집이 모두 들어간 뒤 한 번만 리뷰한다. 구현자는 퇴역 전과 뒤의 JUnit 케이스 집합을 시나리오마다 비교하고, 케이스가 줄었거나 I14 훑기가 문구를 바꾼 시나리오를 이 목록에 더해 증거에 id로 적는다.
- I16 순서: 퇴역과 편집이 병합된 트리에서 (1) `canon inventory sync`, (2) 시험을 돌린 구현자 세션의 `canon evidence add --all-scenarios --report junit:<경로>`, (3) 증명자와 다른 리뷰어 세션의 `canon review change canon-0-43-test-cleanup --project canon`, (4) 증명자와 다른 리뷰어 세션의 `canon review scenario <id>… --project canon`이다. (4)는 목록과 구현자가 더한 시나리오 전부를 (2) 뒤에 한다. (2) 전의 리뷰는 케이스 집합이 바뀌어 다시 낡는다. 리뷰어는 시험을 읽고 문구에 개수나 지운 점검의 말이 남아 있으면 반려한다. 승인 뒤 오케스트레이터가 이 순서를 맡고 구현자의 일이 아니다.

## Checks

- I1: 변경의 diff가 `Cargo.lock`, `conformance/`, `docs/cli/canon.cli.json`을 건드리지 않고 `crates/*/src`는 `#[cfg(test)]`와 주석만 건드린다. `just docs`와 `just conformance`가 아무 파일도 바꾸지 않는다.
- I2, I3: 변경 전 main과 변경 뒤에 `cargo test --workspace -- --list`를 한 번씩 돌려 이름 목록을 `diff`한다. ==**사라진 이름은 I3-I8의 목록과 정확히 같고(141개), 그 밖의 이름은 하나도 사라지지 않는다.**== atip 변경이 먼저 병합된 main을 전으로 삼으므로 그 변경이 없앤 이름은 비교에 들지 않는다. 더해진 이름은 없다. 한 번 읽는 확인이고 저장소에 남기지 않는다.
- I2: `canon inventory sync`가 오류 없이 돈다. 퇴역 119개 id가 `.canon/specs`에 한 곳도 없고(`rg "@<id>$"`), 시험 일부만 지워 남는 시나리오 여섯(atip.contract11.04, atip.l0.23, atip.reasons.08, atip.schemadoc.13, change.drop.24, ingest.removal.02)은 남은 시험 이름이 증거에 있다. `canon format .canon/specs`가 깨끗하다. 열두 주제(atip-contract 61, atip-schema 16, gate-trust-spine 11, tier-storage 8, evidence-attestation 10, atip-input 3, finding-lifecycle 2, dispatch-manifest 2, ingest-pipelines 2, skills-install 2, subject-lifecycle 1, context-capability 1)마다 시나리오가 남고 `.feature` 파일이 빈 채로 남지 않는다. `canon gate check`의 위반 0은 I16의 점검이 보인다.
- I2: (주제, surface) 쌍마다 실패 경로를 세어 퇴역과 태그 변경(`atip.optionvalues.16`) 뒤에 실패 경로를 잃은 쌍이 없는지 보인다. atip 변경이 없애는 `atip.claim15.11`과 주제 `atip-check`(시나리오가 남지 않는 처리는 atip 변경의 몫)의 쌍은 제외한다. scratch 복사에서 건드린 열두 주제 모두를 `subject move <주제> --to verifying`, `--to shipped`로 옮겨 보고 `uncovered-cell`(`require_cases` 규칙)이 하나도 나오지 않는다. 이 점검은 `require_cases` 규칙만 본다. 옛 브리프 38개의 `brief.flow-*` 막힘(질문과 가정의 결정)은 이 변경이 출시를 하지 않으므로 여기서 다루지 않고 증거에 목록만 적는다. 저장소는 건드리지 않는다.
- I2: 변경 뒤 주제의 시나리오 전체를 같은 시험 보고서로 다시 묶는다. `canon evidence add --all-scenarios --report junit:<경로>`다. 이 묶기 뒤에 I16의 리뷰를 한다.
- I3 A-D: `rg -n "CHANGELOG.md" crates/*/tests`에 남는 읽기는 `docs_lint`와 같은 명령 파싱처럼 소비자 계약을 읽는 것뿐이고, 남는 줄의 목록과 이유를 증거에 적는다. `rg -n "include_str!" crates/canon-cli/tests`에 시험 도우미가 읽는 소스 글자가 없다. 남은 시험마다 리뷰어가 "어떤 소비자 버그를 잡는가"에 답한다.
- I3 E: `rg -n "test.yml|justfile|\.github" crates/*/tests`가 가리키는 시험은 `hermetic_env`와 `canon-0-43-atip-0-3-2`가 남기는 시험뿐이다.
- I3 F: `rg -n '"1\.6"' crates/canon-cli/tests --glob '*.rs'`의 남는 줄이 `DECLARED_MINOR` 상수와 픽스처 읽기 밖에 없다. 남는 줄은 증거에 적는다. `atip_claim14/15/16`과 `atip_claiml2`의 바이너리가 명령마다 `canon schema`를 띄우지 않는다(`SURFACE` 제거 또는 프로세스 안 문서). `cli_spec`, `atip_l0_26`, `atip_schemadoc_13`(첫 시험), `docs_lint`가 통과한다.
- I3 F: `atip_helpjson`, `atip_inputorder`, `atip_readwrites`에서 줄을 뺀 시험마다 어떤 사례의 어떤 단계가 그 단언을 대신 얼렸는지를 증거에 한 줄씩 적는다. 대신하는 것으로 인정하는 것은 JSON 단계의 산문이 아닌 값과 글 모드에서 남는 기계 토큰 줄뿐이다. 대신할 사례가 없으면 단언은 남는다.
- I3 F: 종료 값 소스 훑기는 `exit_registry.rs`에 하나 있고 `atip_reasons.rs`에는 소스를 훑는 시험이 없다. 리터럴 `ExitCode::from(3)` 하나와 `.exit =` 덮어쓰기 하나를 소스에 심으면 이 시험이 실패하는 것을 손으로 한 번 보이고 시험으로 남기지 않는다.
- I3 G: 정확한 개수와 `>` 한계를 지운 위치 목록이 증거에 있고, 훑기가 빈 채로 통과하는 것을 `!is_empty()`가 막는다.
- I5: `session_kinds_removal`의 남은 시험이 위 열아홉 개와 같다. `cargo test -p canon-cli --test session_kinds_removal`의 "finished in"이 이전보다 작다(CI 값).
- I6: `test.yml`의 단계와 `justfile`의 `test-callers`가 같은 세 줄(호출자 없음, 구현자와 행위자, 알 수 없는 역할)을 `-p canon-cli --lib`로 돈다. `rg -l "CANON_ROLE|CANON_ACTOR" crates/*/src crates/*/tests`에서 canon-cli 밖은 주석뿐이다. `hermetic_env`가 통과한다. 샌드박스에서 세 호출자의 `cargo test -p canon-cli --lib`가 통과한다.
- I7: `rg -n "include_str!|sources" crates/canon-cli/tests/rule_support/calls.rs`가 소스 글자를 읽지 않는다. atip 변경이 병합된 기준에서 `just rule-calls`로 표를 다시 만들어 커밋하고 `just rule-calls-check`가 깨끗한 트리에서 통과한다. 두 번 만들어 같은 바이트다. 이 변경의 표 diff가 줄의 `digest` 값뿐이다(호출, 규칙, 머리의 `atip` 리비전은 atip 변경이 만든 표와 같다). 규칙 하나를 바꾸면 그 명령의 줄이 낡았다고 읽기가 실패하고 표를 만드는 명령을 말한다는 것을 한 번 손으로 보이고 시험으로 남기지 않는다. `arg_rules`와 `atip_check_order`가 통과한다.
- I8: `rg -n "grammar-mutation|check-mutations|grammar_t15" . --glob '!CHANGELOG.md' --glob '!.canon/**' --glob '!docs/research/**'`가 비어 있다. `just release-check`(워크플로 안전 검사)가 통과한다.
- I9: 이 변경의 diff가 I9의 atip 변경이 맡은 시험(`atip_check_43`, `atip_claim14_13`, `atip_claim14_14`, `atip_claim15_10`, `atip_claim15_11`, `atip_claim16_11`, `atip_claim16_12`, `atip_claiml2_07`, `atip_optionforms_15`, `atip_optionvalues_16`)과 그 시나리오를 건드리지 않는다.
- I11: `packages/dashboard/test/panel-copy.test.ts`가 없고 `rg -n "panel-copy" crates packages canon` 결과가 없다. 남은 대시보드 시험(`bun test`)은 이전과 같은 결과다. `canon skills show canon-report-dashboard`가 한 문장만 달라진 것을 diff로 보인다.
- I12: 이 변경의 diff에 새 `*.rs` 시험 파일, 새 스크립트, 새 레시피가 없다.
- I13: 측정은 CI 로그에서 읽는다. 전(前)은 이 변경의 기준 커밋(atip 변경을 병합한 main)에서 `Test` 워크플로를 `workflow_dispatch`로 두 번 돌려 얻고, 후(後)는 이 변경의 마지막 커밋의 PR 실행과 병합 뒤 main 푸시 실행에서 얻는다. 러너는 모두 `ubuntu-26.04`다. 값은 `offline` 작업의 `cargo test --workspace` 단계 로그에서 읽은 바이너리별 "finished in"과 그 합, 단계별 시간(빌드, 시험, 세 호출자, witness, gate), 작업 전체 시간, `grammar-mutation` 작업의 시간(전에만 있다)이다. 표는 바이너리 상위 20개와 이 변경이 줄인 바이너리(`atip_claim14/15/16`, `atip_claiml2`, `session_kinds_removal`, `arg_rules`, `schema_doc`, `spec028_c`, `atip_readwrites`, `atip_helpjson`)를 담는다. 같은 커밋을 두 번 돌린 값의 차이를 잡음으로 적고, 후가 전보다 그 차이를 넘어 작을 때만 줄었다고 쓴다. 감사의 추정(시험 단계 300-380초, 세 호출자 약 90초, 작업 27.4분에서 약 20분)은 추정으로 나란히 놓는다. PR 설명과 CHANGELOG의 Unreleased에 표를 싣고 증거로 묶는다.
- I14: 손으로 한 번 보인다. 시험으로 남기지 않는다. canon의 오류 `message` 낱말 하나, 글 모드 `hint:` 줄 한 낱말, 명령 `summary` 한 낱말, 옵션 `help` 한 낱말, `next:` 줄의 ` — ` 뒤 문장 한 낱말을 바꾼 바이너리로 `conformance_corpus_replays`(스키마 문서 사례 포함), `cli_spec`, `docs_generated`와 이 변경이 고친 시험이 통과한다. 같은 바이너리에서 오류 `code`, `field`, 종료 값, 옵션 이름, 게이트 위반 종류 하나를 바꾸면, `ErrorBody`의 `message` 필드나 `change new` 입력 스키마의 `title`을 빼면, 글 모드 줄의 개수나 경로나 id 하나(`evidence-add-binds-only-named`에서 앞 첨부를 다시 이어 받는 변경)를 바꾸면, 웹사이트 표의 옵션 칸이나 철거된 철자 표의 새 철자를 바꾸면 각각 해당 시험이 실패한다. `rg -n "byte.exact|byte for byte" crates/canon-cli/tests conformance/README.md`에 산문을 바이트로 비교한다는 말이 없다. `atip_helpjson_08`에 `HELP_POINTER` 비교가 없다.
- I14: `rg -n '\["(message|hint|effectNote)"\]' crates` 결과 어느 줄도 그 값을 문장과 대조하지 않는다. 남는 줄은 값을 읽어 다른 단언에 넘기는 줄이고 목록과 이유를 증거에 적는다. stdout·stderr 글을 `contains`로 대조하는 줄은 남는 줄마다 오류 코드, `field: --옵션`, 위반 종류, id, 명령 줄, 경로 중 하나를 찾는다. 전후 줄 수(319에서, 30줄 이상에서, 417에서)와 이 훑기로 통째로 사라진 시험과 시나리오를 증거에 파일별로 적는다. `cargo test --workspace`가 통과하고 사라진 시험의 시나리오가 I2의 점검을 통과한다.
- I15: `rg -n "in error|assertIn" scripts/test-release-workflow-safety.py scripts/test-check-release-binary.py`에 남는 줄이 문장 조각을 찾지 않고(`canon-0-43-release-hardening`이 고치는 규칙의 시험은 그 변경의 몫이다), `recovery=true and dry_run=false`를 요구하는 규칙과 시험이 없다. `just release-check`가 통과하고 깨끗한 워크플로와 바이너리 입력을 검사기가 통과시킨다.
- I16: 열두 주제가 `shipped`인 채로 I16의 순서가 끝난 뒤 `canon gate check`가 위반 0이다. (3)만 한 상태에서는 `unreviewed-promotion`이 목록의 시나리오(와 구현자가 더한 시나리오)에서만 나오고 다른 시나리오에서는 나오지 않으며, (4) 뒤에 0이 된다. 변경 단위 리뷰만으로 초록이 된다고 쓰지 않는다. 낡은 리뷰가 남는 시나리오는 증거에 id로 적는다.
- 모두: `cargo test --workspace`, `just lint`, `just release-check`, `canon gate check`, `just atip-deep`가 통과한다. CHANGELOG의 Unreleased에 시험만 바뀌었음, 지운 시험 수, 퇴역 시나리오 수, 측정한 전후 값을 싣는다.

## Out of scope

- 감사의 `ARG_RULES_SUBSETS` 기본값을 1,500에서 100으로 낮추는 일(감사의 #6)과 `schema_doc`, `input`, `json_contract`, `read_effects`, `spec028_c`의 `canon` 프로세스 띄우기를 프로세스 안 문서로 바꾸는 일(감사의 #8). 둘 다 시험이 무엇을 단언하는지의 판단이 따로 필요하고 측정이 먼저다.
- witness 기록(`CANON_WITNESS_DIR`, 디버그 바이너리의 기록, `UNWITNESSED` 허용 목록, `grammar_t9_witness`)(Q1).
- 감사의 H(우연한 상수, 동어반복, 하네스 자체 시험): `docs_generated`의 목록 시험, `conformance.rs`의 정규화 규칙 시험, `hermetic_env`, 라이브러리 크레이트의 `round_trip` 이름. 다음 감사에서 하나씩 읽는다.
- `rule-calls-check` 레시피, `ARG_RULES` 탐색 자체, `atip` 크레이트 안의 정규식 재컴파일(0.42와 같음).
- `conformance/`의 사례 추가와 수정. 말뭉치는 얼린 계약이고 이 변경은 Rust 쪽 중복을 줄이고 비교하는 코드에서 산문을 뺄 뿐 사례와 기대 파일은 건드리지 않는다. `src/`의 제품 문구(오류 `message`, `hint`, 도움말)의 수정도 범위 밖이다.
- E의 남은 시험과 `--contract`, 태그, 설치 줄을 읽는 시험의 처분. `canon-0-43-atip-0-3-2`가 맡는다(I9).
- 안 부르는 나머지 시험의 연결과 삭제(Q2 기본은 둔다). `scripts/test-check-knowledge.py`, `evals/test_*.py`(510줄), `packages/dashboard/test/*`(`panel-copy`를 뺀 1,213줄). 후속 목록으로 남긴다.
- 시험 규칙을 저장소 문서(코딩 규칙 등)에 적는 일. 규칙의 자리는 이 브리프와 리뷰어의 판단이다.
- nextest 도입, CI 작업 구조 변경(작업 하나를 지우고 단계 하나를 줄이는 것 말고), 시험 바이너리 통합.
- `build-native.yml` 편집. 이 변경은 `scripts/`를 지우므로 그 워크플로의 `scripts/**` 경로 필터가 한 번 돌 뿐이다.

## Questions and assumptions

- 결정: chore다. 사용자가 볼 수 있는 것은 바뀌지 않는다. 범주는 `refactor`다(시험 코드와 시나리오, CI 단계 하나). 선례는 `canon-0-42-test-cost`, `canon-0-38-test-determinism`이다. 시나리오는 퇴역할 뿐 새로 생기지 않는다. 선례의 "시나리오를 추가하지 않는다"를 따른다.
- 결정: 주제는 `atip-contract`가 대표이고(퇴역 61개) 퇴역 시나리오가 있는 나머지 열한 주제(atip-schema, gate-trust-spine, tier-storage, evidence-attestation, atip-input, finding-lifecycle, dispatch-manifest, ingest-pipelines, skills-install, subject-lifecycle, context-capability)도 채택한다. 채택 경고(`BRIEF_ADOPTION_UNAPPROVED`)는 승인 뒤 사라진다. 이 변경은 주제를 다시 열지도(`subject move --to verifying`) 다시 출시하지도 않는다. 열두 주제는 `shipped`로 남는다. 게이트가 초록이 되려면 변경 단위 리뷰 하나와 `canon evidence add --all-scenarios --report junit:<경로>`로는 모자라고 문구와 태그와 케이스 집합이 바뀐 시나리오마다 증거 뒤에 새 `canon review scenario`가 있어야 한다(I16, plan review r3 3_1, scratch에서 변경 리뷰 뒤에도 `unreviewed-promotion`이 남는 것을 확인했다). 시험 코드는 코드 트리의 일부이므로 `--all-scenarios` 묶기가 모든 주제에 필요하다.
- 결정: 이 변경의 퇴역은 옛 승인 브리프 38개에 `brief.flow-uncovered`나 `brief.flow-no-failure` 막힘을 만든다. 그 브리프의 흐름 단계 가운데 CHANGELOG, 안내문, 옛 이름을 말하던 단계(주로 F5-F8)는 퇴역한 시나리오가 유일한 시나리오이거나 유일한 실패 시나리오였다. scratch 복사에 퇴역을 적용해 `brief check`로 센 값이다. `brief.flow-no-failure` 10개: canon-0-33-atip-l0, canon-0-34-atip-1-1, canon-0-34-atip-schema-doc, canon-0-35-batch-pointers, canon-0-35-crate-1-2, canon-0-36-codes-rename, canon-0-36-help-json, canon-0-37-claim-1-5, canon-0-37-option-forms, canon-0-39-session-kinds-removal. `brief.flow-uncovered` 32개: canon-0-35-batch-pointers, canon-0-35-canon-stopped-midway, canon-0-35-claim-l2, canon-0-35-plan-jcs, canon-0-35-skills-show, canon-0-36-claim-1-4, canon-0-36-code-reasons, canon-0-36-codes-incomplete, canon-0-36-codes-refusals, canon-0-36-codes-rename, canon-0-36-codes-stored, canon-0-36-codes-table, canon-0-37-approval-evidence, canon-0-37-claim-1-5, canon-0-37-option-values, canon-0-38-artifacts-prune, canon-0-38-code-scopes, canon-0-38-date-refusals, canon-0-38-finding-closer, canon-0-38-provider-option, canon-0-38-schema-shape, canon-0-39-git-boundary, canon-0-39-prune-temps, canon-0-39-secret-scan, canon-0-39-session-ingest-removal, canon-0-39-session-kinds-removal, canon-0-39-staleness-scope, canon-0-39-write-durability, canon-0-40-ci-evidence, canon-0-40-git-env-allowlist, canon-0-40-object-store-zstd, canon-0-41-atip-1-6. 두 종류가 합쳐 38개 브리프다. 가이드는 이 둘이 출시를 막지 승인을 막지 않는다고 하고, 열두 주제가 `shipped`인 동안 `status`의 `openBlockers`와 `gate check`는 이 막힘을 세지 않는다(scratch에서 0과 위반 0으로 확인했다). 그래서 이 변경은 출시하지 않고 `shipped`로 둔다. 승인된 브리프를 고치면 사람의 새 답이 필요하고, 가이드(`brief-align`)가 시나리오의 흐름이나 경우 태그를 바꾸는 것을 금지하며, 이 변경은 시나리오를 더하지 않는다. 이 38개 브리프가 막는 것은 그 주제들의 다음 재출시다. 다음 변경이 주제를 다시 열 때 이 막힘을 만나고, 그 처리(옛 브리프의 흐름을 새 변경에서 다시 말하거나 사람이 정하는 닫기)는 그 변경과 사람의 몫이다. 이 변경은 이를 풀지 않는다. 주인이 열두 주제를 지금 재출시 가능하게 하려면 따로 정해야 한다.
- 결정: 시나리오 퇴역에 따로 묘비 커밋이 없다. 묘비는 gitignore된 시나리오 인덱스에 있고 CI의 `canon inventory sync`가 `.feature`에서 다시 만든다(CHANGELOG 0.40). 퇴역은 `.feature` 블록 삭제다.
- 결정(plan review r3 3_2, 오케스트레이터): 옛 브리프 38개의 `brief.flow-*` 막힘은 이 변경이 풀지 않고 미룬다. 열두 주제가 `shipped`인 동안 `gate check`와 `status`가 세지 않고, 다음에 그 주제를 다시 여는 변경이 그 비용을 안는다. 후속 목록에 38개 브리프 이름과 각 브리프가 막는 주제를 적는다.
- 결정: `atip_check_40`과 `atip_codestable_06`은 남긴다(I3 D). 감사는 `atip_check_40`을 시험의 시험으로 보고 지우라고 했으나, 정책 `require_cases: [failure]`가 surface마다 실패 경로 시나리오를 요구하고 퇴역 뒤 이 둘이 각각 `atip.check`와 `atip.codestable`의 마지막 실패 경로다(plan review r1, 1_1). 새 시험과 시나리오는 더하지 않고 둘 다 판정만 단언한다.
- 결정: `session_kinds_removal`은 19개를 남긴다(I5). 감사의 "약 8개"보다 많다. purge 미리보기와 퇴역 키 거절(`tier_purge_preview_*`, `tier_retired_keys_*`)은 이 파일에 있을 뿐 0.40의 소비자 계약이고, `tier_retired_15`, `17`, `20`은 업그레이드된 저장소가 계속 읽히고 그 파일이 그대로라는 유일한 시험이다(plan review r1, 1_2). 시간 절감 추정(90초에서 약 30초)은 그만큼 작아질 수 있고 CI 값으로 보인다.
- 결정(owner, 2026-11-17): 범위는 쓴 대로다. 처음 계획은 시험 151개, 시나리오 128개였다. atip 변경이 `atip_claim14_13`, `atip_claiml2_07`과 그 시나리오를 맡고, `atip_optionforms_15`, `atip_optionvalues_16`은 atip 변경이 고칠 뿐 아무도 지우지 않아 시험 147개, 시나리오 124개가 되었다. plan review r1이 `tier_retired_15`, `17`, `20`, `atip_check_40`, `atip_codestable_06`, 종료 값 소스 훑기 하나를 남기게 하여 이 변경의 몫은 시험 141개, 시나리오 119개다.
- 결정: F의 conformance 중복은 말뭉치를 늘리지 않고 Rust 쪽 줄만 뺀다. 확신 없는 단언은 남긴다.
- 결정(owner의 말, 2026-11-17): "제발 바이트레벨 문구 고정같은건 하지말자. 진짜." 이 말을 규칙으로 쓰고(Intent), 남길 목록의 바이트 비교를 하나씩 읽어 표로 적었다(I14). 기계 계약만 지키는 것은 그대로 두고 섞인 것은 산문을 뺀다. 릴리스 검사기 시험은 I15로 판정만 단언한다. 남는 시험 전체의 문장 단언도 이 변경에서 고친다(I14). 후속으로 미루지 않는다.
- 결정: `skills_install_11`의 스킬 설명 1,024자 한계는 제3자 로더의 한계지만 이 시험은 낱말과 길이와 절 이름을 글자로 읽는다. 규칙대로 지운다. 한계를 지키는 점검이 필요하면 `canon skills check`가 가질 기능이고 후속이다.
- 가정: 시험 수와 시나리오 수는 시험 이름과 파일 읽기로 센 값이고 시나리오 묶기는 이름 앞머리 규칙으로 추정했다. 구현자가 `cargo test --workspace -- --list`와 JUnit 묶기로 다시 세고, 어긋나면 증거에 차이를 적는다. 규칙에 걸리지만 목록에 없는 시험은 리뷰어가 짚는다. 감사의 줄 번호는 main `5c4f46fab` 기준이다.
- 결정: `canon-0-43-release-hardening`은 이 변경과 줄이 겹치지 않는다. 그 브리프는 I17에서 `build-native.yml`(28~38줄, 55줄 위, 74/104/136줄)과 `publish.yml`(머리 주석 23~26줄 부근, 703/721/739/757/775줄)을 만진다고 적고 I1과 I18에서 `test.yml`과 `pr-guard.yml`을 만지지 않는다고 적는다. 이 변경이 고치는 `test.yml`의 곳은 `grammar-mutation` 작업(380줄부터)과 세 호출자 단계(191줄부터)뿐이다. 같은 파일은 `scripts/check-release-workflow-safety.py`와 `scripts/test-release-workflow-safety.py`뿐이고 규칙과 시험이 다르다. 그 변경은 자기 규칙(`paths`와 `scope`, `if:`와 환경변수, `npm whoami`, `TARBALL`)의 시험을 판정으로 고치고, 이 변경은 나머지 규칙의 시험과 `scripts/test-check-release-binary.py`를 고친다(I15). 나머지 `build-native.yml`은 이 변경이 편집하지 않는다.
- 병합 순서: atip → 이 변경 → release-hardening. 그 변경의 I16도 같은 순서를 적는다. 이유 셋. 첫째, atip 변경이 `test.yml`의 `atip` 작업과 `justfile`의 `atip*` 레시피와 같은 시험 파일(`atip_claim14/15/16.rs`, `atip_check.rs`)의 줄을 고친다. atip이 먼저면 이 변경이 고친 파일 위에서 지운다. 둘째, 이 변경은 `test.yml`의 작업 하나와 단계 하나를 지우고 두 검사기 시험 파일의 많은 줄을 판정으로 고친다. 그 위에서 release-hardening이 자기 규칙의 시험을 고치는 편이 충돌이 작다. 같은 시험 파일의 다른 시험이어서 겹침은 줄 단위다. 셋째, 이 변경의 전후 측정은 `offline` 작업의 단계 시간을 비교한다. 그 작업을 바꿀 수 있는 변경이 사이에 병합되면 측정이 섞이므로 측정이 끝난 뒤에 병합한다. 겹치는 곳: `test.yml`(이 변경은 `offline`의 세 호출자 단계와 `grammar-mutation` 작업, atip은 `atip` 작업), `justfile`(이 변경은 `test-callers`와 `rule-calls` 주석, atip은 `atip-install`, `atip`, `atip-deep`).
- 결정(owner, 2026-11-17): witness 기록(`CANON_WITNESS_DIR`)은 남기고 이 변경의 범위 밖이다. 이 단계는 CI에서 32초이고 `grammar_t9_witness_gaps`는 남는다. 지우려면 `src/`의 기록 코드를 바꿔야 해서 chore의 "`src/` 동작 불변"과 부딪친다.
- 결정(owner, 2026-11-17): 안 돌리는 시험은 문구만 보는 `packages/dashboard/test/panel-copy.test.ts`를 지우고(I11, 가리키던 주석과 가이드의 한 절을 고친다) 나머지(`scripts/test-check-knowledge.py`, `evals/test_*.py`, 다른 `packages/dashboard/test/*`)는 연결도 삭제도 하지 않고 후속 목록에 둔다. 이 변경 뒤 마크다운 보고서와 대시보드가 같은 문장을 다르게 말하는 것을 잡는 시험은 없다. 문장 동기화는 문구 고정이라 받아들인다.
- 결정(owner, 2026-11-17): 배선 시험(`atip_check_43`, `atip_claim14_14`, `atip_claim15_11`)은 `canon-0-43-atip-0-3-2`가 없애고 `atip_claim15_10`은 그 변경이 선언 단언만 남긴다. 처음에 이 변경은 이를 그쪽 편집에 맡기고 지우지 않기로 했고(owner), 그 변경이 직접 없애게 되어 남은 일이 없다. 그 변경은 시나리오 atip.claim15.10을 주제 `atip-contract`로 옮기고 atip.claim15.11을 없앤다. 주제 `atip-check`에 시나리오가 남지 않는 처리도 그 변경의 몫이고 이 변경은 `atip-check`를 채택하지도 건드리지도 않는다.
- 결정(owner의 위임으로 오케스트레이터, 2026-11-17): 남길 목록의 섞인 바이트 비교 셋(conformance 재생, `cli_spec`, `docs_generated`)은 기계 필드만 비교한다. 산문은 멤버 이름이 아니라 위치로 정한다(I14의 경로 목록). 같은 이름의 멤버라도 필드 이름이나 사용자 데이터이면 비교한다. 기대 파일과 명세 파일과 생성 블록은 지금처럼 전체 출력으로 만들어 검토에 쓴다(`just conformance`, `just docs`). 산문만 바꾼 변경이 시험을 깨뜨리지 않는다. 비용은 커밋된 명세와 문서의 산문이 CLI의 산문보다 낡아도 시험이 실패하지 않고 `just docs`와 `just conformance`의 diff로만 보인다는 것이다. 대안인 셋의 삭제는 명령·옵션·코드·종료 값의 계약 시험이 함께 사라져 택하지 않는다. 글 모드는 산문을 지우고 나머지를 비교하므로 `--json` 단계가 없는 사례 83개의 계약(개수, 경로, id, 명령 줄)이 남는다.

Approved: journey 2026-11-17T10:20:20Z — "오너가 채팅에서 오케스트레이터에게 0.43 승인을 위임함 (2026-11-17 ask: '응'). 독립 계획 리뷰 3라운드 끝에 남은 P1 3_1(바뀐 시나리오의 리뷰가 낡음)은 리뷰 규칙대로 main(오케스트레이터)이 넘겨받아 수정(I16: 바뀐 시나리오 목록, 증거 뒤 새 시나리오 리뷰, gate 0 확인)을 직접 읽고 확인함. 오너는 브리프 본문을 읽지 않음." (relayed by planner planner-oco via chat) (code tree 0c6fb013d928439f3157a3b1ad6c0ef35fe8d5eb) (brief sha256:97ee1757947560ebdeaa3c25951bc712aa4fdf0e499dc6e7476d6998facde741)
