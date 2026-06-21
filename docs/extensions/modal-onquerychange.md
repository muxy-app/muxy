# Extension Modal API: onQueryChange & feed()

## 개요

`muxy.modal.open()`에 `onQueryChange` 콜백을 제공하면, 사용자가 검색창에 타이핑할 때마다 실시간으로 쿼리를 받아서 직접 검색 결과를 구성할 수 있습니다. 이는 기본 제공되는 클라이언트 사이드 필터링을 대체하는 방식입니다.

## 사용처

- 파일 내용 검색 (`rg`, `grep` 등)
- 외부 API 기반 실시간 검색
- 대용량 데이터셋 (10K+ 아이템) 서버 사이드 필터링
- 복잡한 검색 로직 (정규식, 퍼지 매칭 등)

## API

### `muxy.modal.open(opts)`

```typescript
muxy.modal.open({
  placeholder: "Find in files...",
  items: [],           // 초기 아이템 (빈 배열 가능)
  onQueryChange(query) {
    // 사용자가 타이핑할 때마다 호출
    // query: string (최대 500자, null byte 제거됨)
  },
  onSelect(choice) {
    // 사용자가 아이템 선택 또는 Esc로 닫을 때
    // choice: { id, title, subtitle } | null
  }
});
```

**필드:**

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `placeholder` | `string` | ❌ | 검색창 placeholder 텍스트 |
| `items` | `ModalItem[]` | ✅ | 초기 아이템 목록. 함수로도 제공 가능 |
| `onQueryChange` | `(query: string) => void` | ❌ | 제공 시 클라이언트 필터링 비활성화 |
| `onSelect` | `(choice: ModalItem \| null) => void` | ❌ | 선택/닫힘 콜백 |

### `muxy.modal.feed(items)`

```typescript
muxy.modal.feed([
  { id: "unique-id", title: "Display Title", subtitle: "Description" }
]);
```

- 현재 열린 모달의 아이템 목록을 **완전히 교체**합니다
- 기존 아이템은 모두 제거되고 새 목록으로 대체됩니다
- 빈 배열 `[]` 전달로 결과를 지울 수 있습니다

### `muxy.modal.finish()`

```typescript
// 수동으로 세션 종료 (선택적)
await muxy.modal.finish();
```

- 일반적으로 필요 없음 (자동으로 dismiss 시 종료됨)
- 명시적으로 모달을 닫고 싶을 때 사용

## 동작 방식

### with `onQueryChange` vs without

| 상황 | without `onQueryChange` | with `onQueryChange` |
|------|------------------------|---------------------|
| 모달 열림 | `items(emit)` → `modal.finish` → `modal.await` | `items(emit)` → 세션 유지 |
| 사용자 타이핑 | 클라이언트 사이드 필터링 (title/subtitle) | `onQueryChange(query)` 호출 |
| 아이템 업데이트 | N/A | `muxy.modal.feed(items)`로 직접 제어 |
| 모달 닫힘 | `modal.await` resolve | `onSelect(null)` 호출 후 세션 종료 |
| `modal.finish` | `items(emit)` 직후 자동 | dismiss 시까지 deferred |

### 데이터 흐름

```
사용자 타이핑
  → PaletteOverlay (SwiftUI)
    → ExtensionModalService.queryChanged(query)
      → onQueryChangeHandler?(query)           // webview/runScript
      → NotificationSocketServer.pushModalQueryChange()  // background
        → HostBridge.handleModalQueryChangeLine()
          → JSContext.__muxyModalQueryChange(requestID, query)
            → 등록된 onQueryChange 콜백 실행
```

## 예제: 파일 내용 검색 (find-in-files)

```javascript
muxy.modal.open({
  placeholder: "Find in files...",
  emptyLabel: "Start typing to search",
  noMatchLabel: "No matches",
  items: [],
  onQueryChange(query) {
    // 2글자 미만이면 빈 결과
    if (query.length < 2) {
      muxy.modal.feed([]);
      return;
    }

    // rg로 검색 실행
    const result = muxy.exec(["rg", "-n", "--no-ignore", "-e", query, "."]);
    if (!result || !result.stdout) {
      muxy.modal.feed([]);
      return;
    }

    // 결과 파싱
    const items = result.stdout
      .split("\n")
      .filter(Boolean)
      .map(line => {
        const parts = line.split(":");
        if (parts.length < 3) return null;
        const [file, lineNum, ...contentParts] = parts;
        const content = contentParts.join(":").slice(0, 200);
        return {
          id: `${file}:${lineNum}`,
          title: content,
          subtitle: `${file}:${lineNum}`
        };
      })
      .filter(Boolean);

    muxy.modal.feed(items);
  },
  onSelect(choice) {
    if (!choice) return;
    const [file, lineNum] = choice.id.split(":");
    // 파일 열기 로직
  }
});
```

## 예제: 외부 API 검색

```javascript
muxy.modal.open({
  placeholder: "Search users...",
  items: [],
  async onQueryChange(query) {
    if (query.length < 2) {
      muxy.modal.feed([]);
      return;
    }

    try {
      const response = await muxy.http.fetch(
        `https://api.example.com/users?q=${encodeURIComponent(query)}`
      );
      const users = JSON.parse(response.body);
      muxy.modal.feed(users.map(u => ({
        id: u.id,
        title: u.name,
        subtitle: u.email
      })));
    } catch (err) {
      console.error("Search failed:", err);
      muxy.modal.feed([]);
    }
  }
});
```

## 보안 고려사항

### 입력값 제한

Muxy는 `onQueryChange`로 전달되는 쿼리를 자동으로 정제합니다:

| 제한 | 값 | 설명 |
|------|-----|------|
| 최대 길이 | 500자 | 초과 시 잘림 |
| Null byte | 제거 | `\u0000` → `""` |
| 호출 빈도 | 최소 50ms | 더 빠른 타이핑은 무시됨 |

### feed() 제한

| 제한 | 값 | 설명 |
|------|-----|------|
| 최대 아이템 | 100,000개 | 초과 시 무시됨 |
| 텍스트 길이 | 200자 | id, title, subtitle 각각 |
| 호출 빈도 | 최소 16ms | 더 빠른 호출은 무시됨 |
| 중복 ID | 제거 | 같은 ID는 첫 번째만 유지 |

## 한글 입력 (IME)

한글 입력 시 **조합 중인 문자는 전달되지 않습니다**:

| 사용자 입력 | 전달되는 값 | 시점 |
|------------|-----------|------|
| ㅎ | (없음) | 조합 중 |
| 하 | (없음) | 조합 중 |
| 한 | (없음) | 조합 중 |
| 한글 | `"한글"` | 조합 완료 (space/enter) |

이는 macOS `NSTextView.hasMarkedText()`로 감지하여, 조합 완료 후에만 `onQueryChange`를 호출합니다.

## 디버깅

### 로그 확인

```bash
# Extension 로그 실시간 확인
tail -f ~/.config/muxy/extensions/<extension-name>/logs/output.log
```

### 콘솔 로깅

```javascript
onQueryChange(query) {
  console.log("Query:", JSON.stringify(query));
  console.log("Length:", query.length);
  console.log("Bytes:", new TextEncoder().encode(query).length);
}
```

### Safari Web Inspector (webview)

1. Safari → Settings → Advanced → "Show Develop menu" 활성화
2. Muxy에서 webview extension 실행
3. Safari → Develop → Muxy → 해당 webview 선택
4. Console에서 `muxy.modal.open()` 직접 테스트

### 일반적인 문제

| 증상 | 원인 | 해결 |
|------|------|------|
| `onQueryChange` 호출 안 됨 | `onQueryChange` 함수가 아님 | `typeof opts.onQueryChange === 'function'` 확인 |
| 한글 중간 상태 전달됨 | IME 처리 안 됨 | Muxy 버전 확인 (최신) |
| feed() 반영 안 됨 | 세션 종료됨 | dismiss 전에 호출해야 함 |
| 쿼리가 잘림 | 500자 제한 | 쿼리 길이 확인 |

## 참고

- `onQueryChange` 제공 시 클라이언트 사이드 필터링은 **완전히 비활성화**됩니다
- `items`는 함수 `(emit) => void` 또는 배열로 제공 가능
- `emit` 함수는 비동기적으로 호출할 수 있습니다 (스트리밍)
- `muxy.modal.feed()`는 현재 열린 모달에만 작동합니다 (세션 없으면 무시됨)
