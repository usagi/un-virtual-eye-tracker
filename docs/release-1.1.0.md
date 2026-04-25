# UNVET v1.1.0 Plan

## Scope Lock

v1.1.0 で追加する機能は 1 つだけに固定する。

- VMC / OSC パススルー（raw UDP forward + Desktop GUI 設定）

## Why This Release

Waidayo の VMC / OSC 送信先を 1 つのポート（例: 39540）に設定すると、
その受信を掴んだアプリ以外が同じデータを受け取りにくい。
UNVET が受信した VMC / OSC UDP パケットを複数のローカル宛先へ複製転送できれば、
UNVET 本体利用と他アプリ連携を同時に行える。

## Config Image (TOML)

```toml
[vmc_osc_passthrough]
enabled = true
targets = [
  "127.0.0.1:39539",
  "127.0.0.1:39541",
]
mode = "raw_udp_forward"
```

補足:

- `targets` は `host:port` 文字列配列として扱う。
- TOML として成立させるため、各要素は文字列で記述する。

## Functional Spec

1. `input.source = "vmc_osc"` のときのみ有効。
2. `enabled = true` の場合、受信した各 UDP datagram を `targets` 全宛先へそのまま転送する。
3. UNVET 内部の既存解析パイプラインは従来どおり継続する（転送は副作用処理）。
4. ある宛先への送信失敗は全体停止にしない。ログ記録して次フレームへ進む。
5. 宛先重複は除外する。
6. 自己ループ防止のため、受信ソケットと同一宛先（例: `127.0.0.1:<input.vmc_osc_port>`）は転送対象から除外する。
7. Desktop GUI から `enabled` / `mode` / `targets` を編集できる。
8. GUI からの変更は再起動不要でランタイムへ反映できる。

## GUI Spec (Desktop)

- 対象画面: 現在の Input / VMC 設定エリアに `VMC / OSC Passthrough` セクションを追加
- `enabled`: トグル
- `mode`: セレクト（v1.1.0 では `raw_udp_forward` のみ）
- `targets`: `host:port` の複数行リスト編集（追加 / 削除）
- バリデーション:
  - 空行は保存しない
  - `host:port` 形式でない値は弾く
  - ポートは 1-65535
  - 重複宛先は除外

## Out of Scope (v1.1.0)

- 新規 output backend 追加
- マッピングロジック変更
- OSC メッセージ変換（raw のみ）
- GUI の高度機能（宛先自動検出、疎通チェック、プリセット管理）

## Implementation Units

- `theta-1 feat(config): add vmc_osc_passthrough config model`
- `theta-1 feat(input-vmc): add raw UDP passthrough fan-out`
- `theta-1 feat(ui): add desktop controls for passthrough enable/mode/targets`
- `theta-1 test(input-vmc): add passthrough forwarding tests`
- `theta-1 test(ui): add command-level validation for passthrough settings updates`
- `theta-1 docs: add passthrough usage notes`

## Acceptance Criteria

- 1 つの受信パケットが複数 `targets` へ 1 回ずつ転送される。
- 1 宛先の送信失敗時でも他宛先への転送は継続される。
- `enabled = false` で転送は完全停止する。
- 受信ソケット宛の自己転送は行われない。
- GUI から `enabled` / `mode` / `targets` を更新すると再起動なしで反映される。
- バリデーション違反の入力は GUI で拒否され、既存の有効設定は保持される。
