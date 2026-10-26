# 0.30 계획을 canon 변경으로 옮기기

<!-- canon:kind chore -->

<!-- canon:section intent role=context -->
## Intent

canon 레포는 자기 작업 계획을 외부 형식(openspec 변경 51개, superpowers 계획 19개와 노트·스펙)으로 남겨 왔고, canon 자신의 brief와 변경은 한 번도 쓰지 않았다. canon이 brief, 시나리오, 리뷰를 모두 다루므로 0.30부터 canon 레포의 계획은 canon 변경으로만 남긴다.

0.30 순서: 이 변경 → 스펙 보강 → 코드 정리 → clippy CI → 레저 이름 변경 → 레코드 연결. 여섯 brief는 함께 승인받는다.

<!-- canon:section category role=context -->
## Category

infra

<!-- canon:section invariants role=normative -->
## Invariants

- ==**canon 제품 동작은 바뀌지 않는다.**== openspec·superpowers 계획 어댑터는 다른 사용자를 위한 기능으로 남는다. 그 테스트와 conformance 케이스는 자기 fixture로 계속 돈다.
- canon 자기 코퍼스의 `canon gate check --json`과 `canon status --json` 출력이 이 변경 전후로 같다.
- 기존 evidence, review, finding 레코드는 그대로 유효하다. 그 안의 옛 계획 참조(task id 190개, 계획 링크 572개, 파일 참조 6개)는 문자열로 남는다.

변경 내용:

1. canon 레포 설정의 계획 소스를 빈 목록으로 둔다. 키를 지우면 기본 openspec 소스가 되살아나므로 지우지 않는다.
2. 실제 openspec 파일을 읽던 테스트 하나는 자기 fixture 사본을 읽게 한다.
3. 레포의 openspec 디렉터리와 superpowers 계획·노트·스펙을 지운다. 이력은 git에 남는다.
4. README, ARCHITECTURE, 설계 노트, 예제 README의 계획 링크를 고친다. 웹사이트 예제 문서는 사용자용이라 그대로 둔다.
5. 파생 레코드인 change의 archive도 레저와 같이 git에서 제외한다. 지금은 레저의 change만 제외되어 `canon change new`와 `subject adopt`가 추적되지 않는 archive 파일을 남긴다.

<!-- canon:section checks role=normative -->
## Checks

- ==**변경 전후 `canon gate check --json`과 `canon status --json`을 바이트 비교한다.**==
- `canon init --check`의 계획 항목이 통과한다.
- `cargo test --workspace`와 conformance 케이스가 통과한다.
- 설정 파일, 테스트, openspec의 비 마크다운 파일이 코드 트리에 들어가므로 ingest 시나리오는 새 evidence가 필요하다. 0.30 마지막 evidence 일괄 기록이 이를 덮는다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 계획 어댑터 기능 제거.
- 옛 레코드 안의 계획 참조 문자열 수정.
- 승인된 정책 파일 첫 줄의 openspec 언급(바꾸면 정책 재승인이 필요하다).

Approved: Journey Hwang 2026-10-26T12:59:26Z — "레츠고" (brief sha256:7b71d3ce72b0640787dfa5115ca940a5cd899ffd360f31a95692b9b6a8b0ed06)
