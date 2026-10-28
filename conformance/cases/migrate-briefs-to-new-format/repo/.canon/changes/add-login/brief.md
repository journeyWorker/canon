# 무인 반납함 영수증

<!-- canon:section intent role=context -->
## 의도

무인 반납함은 책을 넣어도 영수증을 주지 않는다. 2026-09-14 회의에서 반납 직후 영수증을 보여 주기로 결정했다.

<!-- canon:section outcome role=normative -->
## 원하는 결과

이용자는 책을 넣고 몇 초 안에 영수증을 받습니다. ==**영수증이 뜨면 그 책은 대출 목록에서 빠져 있다.**==

<!-- canon:section flow role=normative -->
## 사용자 흐름

1. F1 이용자는 반납함 화면을 보고 → 책을 넣으면 → 영수증 화면이 열린다. ==**영수증은 책 한 권마다 한 장이다.**==

<!-- canon:section detail role=normative -->
## 단계 상세

### F1 반납

- 반납함은 책 태그를 읽고 `loan_list`에서 해당 책을 지운다 (`src/kiosk/return.rs:31`). ==**남은 대출 권수는 반납 후의 값이다.**==

<!-- canon:section acceptance role=normative -->
## 인수 조건

- F1: 대출 중인 책을 넣으면 영수증이 열리고 대출 목록에서 그 책이 사라진다. ==**실패한 반납은 기록을 남기지 않는다.**==

<!-- canon:section questions role=question -->
## 질문과 가정

없음.

<!-- canon:section out-of-scope role=context -->
## 범위 밖

연체료 정산은 다루지 않는다.

<!-- canon:waive prose.length detail "여섯 단계의 규칙을 모두 담아야 한다" -->
