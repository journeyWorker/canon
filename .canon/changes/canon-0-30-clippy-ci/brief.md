# 0.30 clippy CI 잡

<!-- canon:kind chore -->

Source: docs/design/rust-coding-rules.md (Accepted, §1, §3)

<!-- canon:section intent role=context -->
## Intent

CI는 지금 clippy를 돌리지 않아서 코딩 규칙을 어긴 코드가 그대로 들어온다. 코드 정리로 경고가 0이 된 상태를 CI가 지키게 한다.

0.30 순서: 계획 이전 → 스펙 보강 → 코드 정리 → 이 변경 → 레저 이름 변경 → 레코드 연결. 이 잡은 코드 정리의 마지막 커밋에 붙인다. 여섯 brief는 함께 승인받는다.

<!-- canon:section category role=context -->
## Category

ci

<!-- canon:section invariants role=normative -->
## Invariants

- ==**canon 바이너리, 출력, 레저는 바뀌지 않는다.**== CI 정의와 검사 스크립트만 추가된다.
- 기존 CI 잡(테스트, 런처, platformer, live tier, 벤치 모양)은 지금 툴체인 그대로 돈다. 툴체인 고정은 새 잡 안에서만 한다.

새 잡은 툴체인 1.99.1로 고정하고 다음을 모두 통과해야 한다:

1. 워크스페이스 전체 대상(테스트 포함) clippy, 경고를 오류로.
2. 라이브러리와 바이너리만 대상으로 같은 검사에 `unwrap_used`를 더해서.
3. live Postgres·R2 테스트 feature를 켠 저장소 crate clippy. 이 feature는 live 테스트 파일만 컴파일 대상에 넣는다.
4. crate 소스에 `pub(super)`나 `pub(in …)`이 있으면 실패하는 검사 스크립트. 이 스크립트는 이 변경이 만든다.

<!-- canon:waive prose.code-citation invariants "규칙 문서가 정한 lint와 가시성 이름이 새 잡이 검사하는 대상 자체다" match="unwrap_used" -->
<!-- canon:waive prose.code-citation invariants "규칙 문서가 정한 lint와 가시성 이름이 새 잡이 검사하는 대상 자체다" match="pub(super)" -->

<!-- canon:section checks role=normative -->
## Checks

- ==**코드 정리가 끝난 커밋에서 새 잡이 통과한다.**==
- 버리는 브랜치에서 네 단계 각각에 위반 하나를 넣어 해당 단계가 실패하는 것을 확인하고, 결과를 리뷰에 남긴다.
- CI 정의 주석에 툴체인 고정을 올릴 때 lint 목록을 같은 변경에서 다시 확인한다고 적는다.
- 검사 스크립트는 코드 트리에 들어가므로, 0.30 마지막 evidence 일괄 기록이 이 커밋 뒤에 있어야 한다. 변경 단위 리뷰는 다른 세션의 reviewer가 한다.

<!-- canon:section out-of-scope role=context -->
## Out of scope

- 코드 수정(코드 정리 변경이 한다).
- rustfmt 검사.
