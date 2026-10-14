# Eval 6 결과: canon 0.18.0 on omp, 출하된 subject의 범위 확장 (2026-10-14)

`PROTOCOL-6.md` 기준. 레포 `nyan-omp-019`, 로그 `run-j-omp-019-t{1,2}.log`, 세션 `~/.omp/agent/sessions/-Workspace-canon-dogfood-nyan-omp-019/` (메인 + 서브에이전트 5: ArtV2, TestsEnemiesBosses, TestsWeaponsEvoChest, TestsE2E, ReviewerV2).

## 인수 기준

| 기준 | 결과 | 근거 |
|---|---|---|
| (a) 범위 확장은 새 change + 새 브리프, 코드 전 사람 멈춤 1회 | 합격 | 턴 1(106초): `cat-survivors-v2` change를 새로 만들고 브리프·tasks.md만 커밋(c817d37)한 뒤 멈췄다. v1 승인 안에서 reopen하지 않았다. 승인 후 `subject status … verifying`으로 다시 열었다(3ab92b0). |
| (b) 승인 후 사람 없이 `shipped` | 합격 | 턴 2(30분), d5e44e2. |
| (c) 일괄 기록 | 합격 | 시나리오 29→60. `evidence add` 2회(둘 다 `--all-scenarios`, unit/e2e junit), `review add` 1회(`--scenario-ids-from`). 세션 6개 합계 canon 호출 24회(eval 5: 38회), bookkeeping 0. |
| (d) 모든 시나리오 증거 + 독립 리뷰, gate clean | 합격 | `60 evidenced, 60 reviewed, 0 open blockers`, `canon gate check: clean`. 리뷰는 별도 서브에이전트 세션(ReviewerV2.jsonl)이 검사를 다시 돌리고 남겼다. |
| (e) 아트는 pixel-forge CLI 패키지로 | 합격 | 소스 경로 호출을 없애고 `bunx forge export .` → `public/forge`, 런타임 `loadForgePack`으로 로드. pixel-forge 레포는 수정하지 않았다. |

## canon 쪽 관찰

1. **tasks.md 체크박스 0/7.** 출하된 change의 작업 7개가 하나도 체크되지 않았다(`canon gate task` 0회). eval 5에서도 5개를 건너뛰었다. 출하 가드와 `canon status`가 둘 다 이를 보여주지 않으니 에이전트가 건너뛴다. → 이슈로 등록.
2. **리뷰 0건, 근거 스크린샷이 주장과 안 맞음.** 리뷰어는 60개 시나리오에 지적 0건을 남겼다. 첨부한 `v2-boss.png`에는 보스가 없고(배너와 체력바만), `v2-chest.png`에는 상자가 없다. 직접 플레이하니 화면 가장자리의 적이 HUD(무기 칸, 보스 체력바, 처치 수) 위에 그려지는 z-order 문제가 있었고, 리뷰가 놓쳤다. canon은 이미지를 비교하지 않으니 설계대로이지만, #18(리뷰 품질)의 사례다.
3. **세션 ID는 여전히 라벨.** 리뷰어는 실제 별도 세션이었지만 `--session-id review-v2-r1`은 지어낸 값이다(#14).

## 그 밖에

- 서브에이전트(ArtV2)가 처음 그린 스프라이트는 알아보기 어려워서 메인 세션이 다시 그렸다.
- 전체 실행 시뮬레이션에서, 적이 300마리 상한에 차 있으면 보스가 나오지 않는 버그를 에이전트가 스스로 찾아 고쳤다.
- 턴 2 최종 보고가 영어였다(사용자는 한국어). omp 쪽 관찰.
- 직접 확인: 타이틀, 레벨업(새 무기 카드, 진화 조건 표시), 3:00 대형견 등장(경고와 체력바), 새 몬스터 4종 스폰을 브라우저에서 확인했다.
