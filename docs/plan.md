# U.N. Virtual Eye Tracker ; iFacialMocap / Waidayo ベース視線入力アプリ 開発計画書

略称: UNVET

## 0. 文書情報

- 文書名: iFacialMocap / Waidayo ベース視線入力アプリ 開発計画書
- 対象: Windows 向け単独アプリケーション
- 主目的:
  - iPhone / iPad 側で取得した顔・眼球トラッキング情報を用い、PC ゲーム向けの視線・カメラ入力を実現する
  - 初期重点対象は ETS2 / ATS
- 最終成果物イメージ:
  - 単独で起動できる軽量アプリ
  - iFacialMocap を直接受信可能
  - 将来的に VMC 互換入力も受信可能
  - 複数の出力方式を切り替え可能
  - 実用レベルの平滑化・キャリブレーション・感度調整を備える

---

## 1. 背景と狙い

ETS2 / ATS では、ハンドルコントローラー利用時に視点操作の快適性が重要になる。
従来のゲームパッド右スティックによる視点操作は自然で扱いやすい一方、ハンコン主体のプレイでは視点操作のために別操作を要求されやすい。

一方、Beam Eye Tracker は OpenTrack なしでの直接ゲームカメラ制御を打ち出しており、視線・頭部トラッキングをゲームに直接橋渡しする構成の有効性を示している。
ただし、Beam は webcam ベースであり、今回の要件で重視される「配信負荷の最小化」「iPhone / iPad 側でのトラッキング処理完結」とは少し方向が異なる。
そのため、本計画では **iFacialMocap / Waidayo を入力源とし、Windows 上の単独アプリで受信・正規化・出力を行う独自実装** を目指す。

---

## 2. 参考情報整理

### 2.1 iFacialMocap

- UDP / TCP で PC ソフトを介さず直接データ受信可能
- UDP は iOS 側 49983/UDP に開始文字列を送ると、PC 側 49983 に最大 60 FPS で返信
- 受信データには BlendShape 群に加え、`head`、`rightEye`、`leftEye` のオイラー角が含まれる
- 角度は degree で送られる
- TCP モードもあり、PC 側 49986/TCP で受信可能

### 2.2 VMC Protocol

- OSC ベースのモーション通信仕様
- Bone、BlendShape、カメラ、トラッキング座標などをやり取りできる
- Waidayo / Warudo / VSeeFace などの周辺エコシステムとの互換層として有効

### 2.3 Beam Eye Tracker

- 近年は一部ゲームに対して OpenTrack なしの direct game camera control を提供
- SDK 上でも tracking stream から game camera state を生成する思想が確認できる
- 本プロジェクトでは Beam を直接使うのではなく、**同様の設計思想を自前アプリで再現する**

### 2.4 VSeeFace / Webcam Motion Capture / Warudo

- いずれも iFacialMocap または VMC を入力として扱える系統がある
- ただし本プロジェクトでは前段アプリ必須設計を避ける
- 将来の互換入力として VMC を受けられる構造にしておけば、これらを必要に応じて前段に置ける

### 2.5 Windows 入力出力系

- `SendInput` によりキーボード / マウスの合成入力が可能
- XInput は既存ゲームとの互換性が高い一方、仮想デバイス実装は別レイヤーを要する
- DirectInput はレガシー寄り
- GameInput は「入力を読む」ための API であり、仮想ゲームパッド生成の代替ではない
- 仮想 XInput は ViGEm 系で技術的には実現可能だが、ドライバー導入前提で配布性に課題がある

---

## 3. プロジェクト方針

### 3.1 基本方針

1. **単独アプリ完結を基本形とする**
2. 入力はまず **iFacialMocap 直接受信** を最優先とする
3. 将来拡張として **VMC 入力** を追加する
4. 出力は複数 backend を持つ
5. 初版では **ETS2 / ATS 専用出力 + マウス + キーボード** を優先する
6. 仮想 XInput は拡張機能として扱う
7. 設定、キャリブレーション、平滑化、感度調整を実装初期から重視する

### 3.2 非方針

- OpenTrack 必須化はしない
- VSeeFace / WMC / Warudo 必須化はしない
- 初版から仮想ゲームパッドを主軸にしない
- 初版から全ゲーム対応を狙わない
- 初版から複雑な 3D アバター描画は行わない

---

## 4. 要求仕様

## 4.1 機能要求

### 入力

- iFacialMocap UDP 受信
- iFacialMocap TCP 受信
- 入力ソース切替
- 将来の VMC 受信追加を見越した抽象化

### トラッキング処理

- head / leftEye / rightEye の抽出
- 視線 yaw / pitch 推定
- 頭部 yaw / pitch / roll 推定
- 中立姿勢キャリブレーション
- 感度、デッドゾーン、平滑化調整
- confidence / active 判定

### 出力

- ETS2 / ATS 専用 yaw / pitch 出力
- 仮想マウス XY 相対移動
- 仮想キーボード 4方向入力
- 将来拡張:
  - 仮想 XInput 右スティック
  - 仮想 DirectInput / HID 系
  - 仮想ペン座標

### UI / 設定

- 接続状態表示
- 現在値モニター
- キャリブレーション実行
- 出力方式選択
- プロファイル保存 / 読込
- ゲーム別設定

### 運用

- 軽量動作
- 常駐 / 非常駐切替
- ホットキーによる ON/OFF
- 一時停止
- トラブルシュート用ログ

## 4.2 非機能要求

### 性能

- 入力受信から出力までの遅延を極小化
- GUI 有無で処理系を分離し、無駄な描画負荷を避ける
- 30 FPS 低下時でも破綻しにくい補間 / 平滑化

### 保守性

- 入力 / コア処理 / 出力 / UI を分離
- backend 追加しやすい構造
- 設定ファイルとロジックの分離

### 安全性

- 暴走入力を避ける停止機構
- フォーカス外動作の制御
- 出力無効化の即時ホットキー

### 配布性

- 初版はドライバー不要構成で成立させる
- 仮想 XInput は optional feature として分離

---

## 5. システム構成案

```text
[iPhone/iPad]
  └─ iFacialMocap / Waidayo
        ↓
[Input Receiver Layer]
  ├─ iFacialMocap UDP Receiver
  ├─ iFacialMocap TCP Receiver
  └─ (future) VMC Receiver
        ↓
[Tracking Normalize Layer]
  ├─ Parser
  ├─ TrackingFrame Normalizer
  ├─ Calibration
  ├─ Filter / Smoothing
  ├─ Mapping
  └─ Confidence / State Judge
        ↓
[Output Backend Layer]
  ├─ ETS2/ATS Direct Output
  ├─ Mouse Output
  ├─ Keyboard Output
  └─ (future) Virtual XInput Output
        ↓
[Config / UI / Logging]
```

---

## 6. 内部データモデル案

```rust
pub struct TrackingFrame {
    pub timestamp_ms: u64,
    pub head_yaw_deg: f32,
    pub head_pitch_deg: f32,
    pub head_roll_deg: f32,
    pub eye_yaw_deg: f32,
    pub eye_pitch_deg: f32,
    pub left_eye_yaw_deg: f32,
    pub left_eye_pitch_deg: f32,
    pub right_eye_yaw_deg: f32,
    pub right_eye_pitch_deg: f32,
    pub confidence: f32,
    pub active: bool,
}

pub struct OutputFrame {
    pub look_yaw_norm: f32,   // -1.0 .. +1.0
    pub look_pitch_norm: f32, // -1.0 .. +1.0
    pub confidence: f32,
    pub active: bool,
}
```

---

## 7. 出力方式方針

## 7.1 初版対象

### A. ETS2 / ATS 専用出力

- 本命
- 最優先
- ゲームに合わせた専用カーブと応答調整を持つ
- 「自然さ」重視

### B. 仮想マウス出力

- 汎用性が高い
- relative move を基本とする
- 角度ではなく速度へ変換する

### C. 仮想キーボード出力

- 4方向キーまたは WASD
- 閾値とヒステリシスで制御
- fallback として強い

## 7.2 将来対象

### D. 仮想 XInput

- 技術的には可能
- ViGEm 系依存
- ドライバー導入が必要
- optional 扱いとする

### E. 仮想 DirectInput / HID

- 必要性を再評価してから
- 初版では保留

### F. 仮想ペン

- 本件の主用途には優先度が低い
- 将来の特殊用途向け

---

## 8. トラッキング変換方針

### 8.1 視線・頭部の混合

初期値案:

- 左右: `0.7 * eye + 0.3 * head`
- 上下: `0.4 * eye + 0.6 * head`

### 8.2 デッドゾーン

- 中央 5〜8%
- 微小ノイズによる視点ぶれ防止

### 8.3 平滑化

- EMA を基本とする
- 初期係数 `alpha = 0.18` 前後で調整

### 8.4 飽和と感度

- 視線だけで急激に端まで行かないよう制限
- 頭込みで最終可動域を拡張

### 8.5 状態判定

- トラッキング断、無効、アプリ非アクティブ時の fail-safe を実装
- confidence 低下時の減衰停止

---

## 9. 開発フェイズ計画

# フェイズα: 企画・仕様確定・技術検証

## α-1. 問題定義とスコープ固定

### 目的

- 対象ゲーム、対象OS、対象入力源、対象出力方式を固定する

### 完了条件

- 本計画書 v1 を確定
- 初版スコープと除外対象を明文化

### 想定コミット

- `docs: add project overview and scope definition`
- `docs: define v1 target platforms and non-goals`

## α-2. 技術検証: iFacialMocap UDP/TCP

### 目的

- 実際に受信可能か
- 受信フォーマットをコードで安定解釈できるか

### 作業

- UDP 開始コマンド送信
- 49983/UDP 受信確認
- TCP 49986 受信確認
- parser 試作
- ログ保存

### 完了条件

- サンプルログ取得成功
- head / leftEye / rightEye 抽出成功

### 想定コミット

- `feat(proto): add iFacialMocap UDP receive test`
- `feat(proto): add iFacialMocap TCP receive test`
- `feat(parser): parse iFacialMocap head and eye fields`
- `test(parser): add sample frame fixtures`

## α-3. 技術検証: 出力手段

### 目的

- ETS2/ATS 専用出力候補
- マウス
- キーボード
の実装可能性を確認する

### 作業

- mouse relative move 試験
- keyboard press/release 試験
- ETS2/ATS 向け最小出力方法の確認
- 仮想 XInput は feasibility だけ整理

### 完了条件

- 各方式の PoC 成功可否を判断
- v1 出力 backend 優先順位を確定

### 想定コミット

- `feat(proto): add mouse output proof-of-concept`
- `feat(proto): add keyboard output proof-of-concept`
- `docs: summarize output backend feasibility`

## α-4. アーキテクチャ確定

### 目的

- モジュール境界を決める

### 完了条件

- crate / module 構成を確定
- interface 草案作成

### 想定コミット

- `docs: define architecture and module boundaries`
- `chore: scaffold workspace crates`

---

# フェイズβ: コア受信・正規化基盤実装

## β-1. プロジェクト雛形

### 作業

- Rust workspace 作成
- `app`, `core`, `input_ifacialmocap`, `output_mouse`, `output_keyboard`, `config`, `ui` など分割
- logging / error handling / config 基盤導入

### 完了条件

- アプリ起動
- 設定読込
- ログ出力成功

### 想定コミット

- `chore: initialize workspace and base crates`
- `feat(core): add config loading`
- `feat(core): add structured logging`
- `feat(core): add unified error handling`

## β-2. iFacialMocap UDP Receiver

### 作業

- 接続開始送信
- 非同期受信
- パケット処理
- 接続状態監視

### 完了条件

- リアルタイム受信成功
- フレーム破損時の継続動作

### 想定コミット

- `feat(input): implement iFacialMocap UDP receiver`
- `feat(input): add receiver state management`
- `test(input): add UDP frame parsing tests`

## β-3. iFacialMocap TCP Receiver

### 作業

- TCP 開始 / 停止制御
- 区切り文字処理
- 分割フレーム再構成

### 完了条件

- TCP モードで継続受信可能

### 想定コミット

- `feat(input): implement iFacialMocap TCP receiver`
- `feat(input): add TCP frame reassembly`
- `test(input): add TCP stream parsing tests`

## β-4. TrackingFrame 正規化

### 作業

- head / eye 抽出
- 左右眼統合
- 欠損時の扱い
- confidence / active 判定

### 完了条件

- 生入力から TrackingFrame を安定生成

### 想定コミット

- `feat(core): add normalized TrackingFrame model`
- `feat(core): derive eye yaw/pitch from raw eye angles`
- `feat(core): add frame validity and confidence logic`

---

# フェイズγ: キャリブレーション・変換・平滑化

## γ-1. 中立姿勢キャリブレーション

### 作業

- 現在姿勢を基準にゼロ点設定
- 再キャリブレーション
- 保存 / 読込

### 完了条件

- 中央注視時に出力中央を再現

### 想定コミット

- `feat(calib): add neutral pose calibration`
- `feat(calib): persist calibration data`

## γ-2. デッドゾーン / 感度 / カーブ

### 作業

- 線形 / 非線形カーブ
- 中央デッドゾーン
- 軸別感度

### 完了条件

- UI から調整可能
- 反応が極端に不自然でない

### 想定コミット

- `feat(map): add axis sensitivity and deadzone`
- `feat(map): add response curve presets`

## γ-3. 平滑化 / ノイズ抑制

### 作業

- EMA 導入
- confidence 連動抑制
- 入力断時の穏当な停止

### 完了条件

- 微振動が許容範囲
- 入力途絶で暴れない

### 想定コミット

- `feat(filter): add exponential smoothing`
- `feat(filter): add confidence-aware damping`
- `test(filter): add smoothing behavior tests`

## γ-4. 頭部 + 視線混合

### 作業

- 軸別 blend 係数
- プリセット
- ゲーム別設定

### 完了条件

- ETS2/ATS で自然な基本挙動

### 想定コミット

- `feat(map): mix head and eye tracking`
- `feat(config): add per-game mapping profiles`

---

# フェイズδ: 出力 backend 実装（v1）

## δ-1. マウス出力

### 作業

- relative move
- スピード変換
- 有効 / 無効切替

### 完了条件

- 汎用 3D アプリで視点移動可能

### 想定コミット

- `feat(output-mouse): implement relative mouse backend`
- `feat(output-mouse): add speed mapping and clamp`

## δ-2. キーボード出力

### 作業

- 4方向 press/release 制御
- ヒステリシス
- キーバインド設定

### 完了条件

- バタつきなく 4方向制御できる

### 想定コミット

- `feat(output-keyboard): implement directional key backend`
- `feat(output-keyboard): add hysteresis thresholds`
- `feat(output-keyboard): support custom keybinds`

## δ-3. ETS2/ATS 専用出力

### 作業

- 専用 backend 実装
- ゲーム内挙動に合わせた係数調整
- look-back 系拡張余地確保

### 完了条件

- ETS2/ATS で実用レベルの挙動

### 想定コミット

- `feat(output-ets2): implement ets2/ats dedicated backend`
- `feat(output-ets2): add truck sim response presets`
- `test(output-ets2): add backend mapping tests`

## δ-4. 出力 backend 切替管理

### 作業

- backend interface 実装
- 単一 active backend
- 設定切替

### 完了条件

- UI / config から backend 切替可能

### 想定コミット

- `feat(output): add backend abstraction layer`
- `feat(output): add runtime backend switching`

---

# フェイズε: UI / UX / 設定管理

## ε-1. 最小 UI

### 作業

- 接続状態
- 現在の yaw/pitch 表示
- ON/OFF
- backend 選択

### 完了条件

- CLI なしで基本操作可能

### 想定コミット

- `feat(ui): add main control window`
- `feat(ui): show tracking status and live values`

## ε-2. 調整 UI

### 作業

- キャリブレーション
- 感度
- デッドゾーン
- 平滑化
- プロファイル保存

### 完了条件

- ユーザーが自己調整できる

### 想定コミット

- `feat(ui): add calibration controls`
- `feat(ui): add mapping and smoothing settings`
- `feat(config): add profile save and load`

## ε-3. ホットキー / トレイ / 一時停止

### 作業

- 出力 ON/OFF
- 一時停止
- 再キャリブレーション
- 常駐化

### 完了条件

- ゲームプレイ中の運用が現実的

### 想定コミット

- `feat(ui): add hotkeys for enable pause recalibrate`
- `feat(ui): add tray integration`

---

# フェイズζ: 品質向上・検証・パッケージング

## ζ-1. 実機テスト

### 作業

- iPhone / iPad 実接続
- UDP / TCP 比較
- ETS2 / ATS 実プレイテスト
- 30 FPS / 60 FPS 変動試験

### 完了条件

- 主要ユースケースで安定動作

### 想定コミット

- `test: add real-device validation notes`
- `fix: adjust tracking stability under low fps`

## ζ-2. 異常系テスト

### 作業

- ネットワーク断
- フォーカス外
- トラッキング喪失
- 設定破損

### 完了条件

- 暴走入力しない

### 想定コミット

- `test: add fail-safe and disconnect scenarios`
- `fix: prevent runaway output on signal loss`

## ζ-3. パッケージング

### 作業

- 配布ビルド
- 設定ファイル配置
- README
- FAQ
- ログ採取手順

### 完了条件

- 他者が導入可能

### 想定コミット

- `docs: add user setup guide`
- `docs: add troubleshooting guide`
- `build: add release packaging workflow`

---

# フェイズη: 拡張計画（v1.1 以降）

## η-1. VMC 受信

### 想定コミット

- `feat(input-vmc): add VMC receiver`
- `feat(input-vmc): map VMC data into TrackingFrame`

## η-2. 仮想 XInput

### 前提

- optional feature
- ドライバー導入前提
- 初版とは切り離す

### 想定コミット

- `feat(output-xinput): add ViGEm-based virtual xinput backend`
- `docs: add optional driver setup for virtual xinput`

## η-3. ゲーム別プリセット追加

### 想定コミット

- `feat(config): add preset packs for supported games`

## η-4. 視線ジェスチャ / look-back / UI 操作補助

### 想定コミット

- `feat(map): add gesture-based quick look actions`
- `feat(ui): add advanced interaction presets`

---

## 10. 推奨リポジトリ構成

```text
project-root/
  Cargo.toml
  crates/
    app/
    core/
    input-ifacialmocap/
    input-vmc/
    output-ets2/
    output-mouse/
    output-keyboard/
    output-xinput/
    config/
    ui/
  docs/
    overview.md
    architecture.md
    protocol-ifacialmocap.md
    roadmap.md
  assets/
  tests/
```

---

## 11. リスクと対策

### R1. ETS2/ATS 専用出力の実装難所

- 対策:
  - 初期は mouse / keyboard backend で先に全体を通す
  - 専用 backend は独立モジュールとして後追いで調整

### R2. iFacialMocap 側の FPS 低下

- 対策:
  - UDP/TCP 切替可能
  - 補間と平滑化を実装
  - 30 FPS 動作を前提に設計

### R3. 仮想 XInput の配布性

- 対策:
  - v1 対象外
  - optional 扱い
  - 本体と疎結合化

### R4. 視線のみだと挙動が不自然

- 対策:
  - 頭部情報を混合
  - ゲーム別プリセット
  - キャリブレーションと感度調整

### R5. ゲーム中の誤作動

- 対策:
  - 即時無効ホットキー
  - フォーカス制御
  - confidence 低下時停止
  - active 判定

---

## 12. 最小実用版（MVP）定義

### MVP 条件

- iFacialMocap UDP 受信
- TrackingFrame 正規化
- キャリブレーション
- 平滑化
- ETS2/ATS 専用出力または mouse backend のどちらかで実用
- UI から ON/OFF と感度調整可能
- 設定保存可能

### MVP 完了の判断基準

- ETS2 / ATS を 30 分以上プレイして破綻しない
- 中央ぶれが許容範囲
- 視点移動が過敏すぎず、遅すぎない
- トラッキング断で暴走しない

---

## 13. 実装着手順の推奨

実際の着手順は以下を推奨する。

1. フェイズαを完了させる
2. β-2 の UDP 受信まで最速で実装する
3. β-4 まで到達して TrackingFrame を安定生成する
4. γ 系で自然な出力へ整える
5. δ-1 / δ-2 で mouse / keyboard backend を先に完成させる
6. その後 δ-3 の ETS2/ATS 専用 backend を詰める
7. UI と設定を載せて MVP 完了
8. その後 η 系の拡張へ進む

---

## 14. まとめ

本プロジェクトの中核は、**前段アプリに依存しない単独アプリとして、iFacialMocap / Waidayo 系トラッキング情報を直接受け、ゲーム向け視線入力へ変換すること** にある。
設計上は次の判断が重要だ。

- 基本形は単独アプリ
- iFacialMocap 直接受信を最優先
- VMC は互換入力として後から足せる設計
- 初版は ETS2/ATS・マウス・キーボードを優先
- 仮想 XInput は拡張扱い
- フェイズとコミット粒度を明確にし、逐次開発しやすい形で進める

この計画に従えば、技術検証から MVP 完成、さらに将来拡張まで、比較的破綻しにくい進め方になる。
