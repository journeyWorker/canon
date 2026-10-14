# Eval 8 결과: canon 0.20 (feat/0.20) on omp (2026-10-15)

`PROTOCOL-8.md` 기준. 레포 `nyan-omp-021`, 로그 `run-l-omp-021-t{1,2}.log`, 바이너리 feat/0.20 22cc85d2.

## 인수 기준

| 기준 | 결과 | 근거 |
|---|---|---|
| 기본 루프 (브리프 멈춤, 승인, shipped, gate clean) | 합격 | 턴 1은 94초 만에 한국어 PRD 브리프(`result-stats`)로 멈췄다. 차단 0, 권고 5. 턴 2에서 `canon brief approve`를 거쳐 shipped. 83/83 evidenced·reviewed, gate clean. 리뷰어는 별도 세션(`ReviewerResult`). |
| trace_check hard checks | 합격 | 멈추기 전 `.canon/` 밖 쓰기 0. 리뷰어 ≠ 구현자(아래 trace_check 수정 후). |
| 성공당 호출 수 (eval 7 대비) | 개선 | eval 7 기능 부분(턴 1–2 + 리뷰어): canon 37회에 시나리오 12개(3.1회/개). eval 8: 23회에 시나리오 11개(2.1회/개). |
| 오류 후 회복 | 합격 | 원인을 판별할 수 있는 canon 실패 뒤에는 모두 다음 호출에서 회복했다. |
| 0.20 기능 사용 | 관찰 | `canon schema` 3회 사용(명령 형식 확인). 첫 호출은 `canon schema "evidence add"`처럼 경로를 한 단어로 따옴표 처리해 `UNKNOWN_COMMAND`를 받았고, 다음 호출에서 고쳤다. |

## 평가 중 고친 것 (feat/0.20)

1. **`canon … | head`가 panic.** Rust는 SIGPIPE를 무시한 채 시작하므로, 읽는 쪽이 닫힌 stdout에 쓰면 `println!`이 panic했다(exit 101). 이제 기본 SIGPIPE 동작으로 조용히 끝난다. `dashboard`는 예외로 계속 무시한다. 수정 전 실패, 수정 후 통과하는 테스트를 넣었다.
2. **`canon schema "evidence add"` 거절.** 이제 한 단어로 따옴표 처리한 경로도 같은 명령으로 해석한다.
3. **trace_check 오판 두 건.** 구현자가 `canon review add --help`만 읽어도 리뷰어로 셌고, 리뷰어가 `/tmp`에 둔 재현 스크립트를 코드 작성으로 셌다. 그래서 두 세션이 모두 "리뷰어 = 구현자"로 실패했었다. 레포 밖 경로와 `--help` 호출을 제외하고 fixture 테스트를 추가했다.

## 그 밖에

- **`--all-scenarios` 일괄 증거 거절.** 기존 시나리오의 최신 증거가 묶은 스크린샷을 이번 배치가 다시 묶지 않으면, 첨부를 몰래 빠뜨리는 것을 막는 규칙(0.17)에 걸려 배치 전체가 거절된다. 에이전트는 새 시나리오 위주로 다시 기록해 회복했다. 0.19 chore의 "테스트 묶음 전체 재결합" 경로와 부딪칠 수 있어 이슈로 남긴다.
- 에이전트는 "일괄 명령이 중간에 끊겨 4건이 대기로 남아 `canon gate promote`로 반영했다"고 보고했다. 직접 재현하지는 않았다.
