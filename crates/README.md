# 올바로 Rust 크레이트

| 크레이트 | 하는 일 |
|---|---|
| `olbaro-core` | 문서 모델(마크다운 블록, 문장, 원문 오프셋과 UTF-16 변환), `Rule` 트레이트, 규칙별·묶음별 설정, 진단 |
| `olbaro-rules` | 규칙 묶음. 맞춤법(`spelling`), 띄어쓰기(`spacing`), LLM 말투(`llmstyle`, 기본 꺼짐) |
| `olbaro-cli` | 명령줄 도구 `olbaro` |
| `olbaro-wasm` | 브라우저 확장이 쓰는 WASM 바인딩. `Needs::Text` 규칙만 싣는다. 루트에서 `pnpm build:wasm`(wasm-bindgen-cli 0.2.100 필요) |

코어와 규칙은 파일·네트워크·스레드를 쓰지 않아서 `wasm32-unknown-unknown`으로도 빌드된다.

```sh
cargo run -p olbaro-cli -- --enable llmstyle 문서.md        # 지적 목록
cargo run -p olbaro-cli -- --enable llmstyle --count 문서.md  # 규칙별 횟수
cargo run -p olbaro-cli -- --list-rules
```

진단은 세 단계다. 겉 단서로 답이 확실할 때만 오류, 고칠 만한 문체는 경고, 둘 다 맞는 말이지만 한쪽일 가능성이 높아 묻기만 할 때는 안내.
