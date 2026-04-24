# UNVET - USAGI.NETWORK Virtual Eye Tracker

UNVETは仮想アイトラッキングアプリです。

iFacialMocap(iPhone/iPad)やVMC Protocol(VMC/Waidayo/WebcamMotionCapture/VSeeFace/etc.)などのフェイストラッキングを入力ソースとしてフェイストラッキングデータを受け取り、ETS2/ATSのヘッドトラッキングやマウス/キーボードなどの出力バックエンドに変換して仮想的なアイトラッキング体験を提供します。

※アイトラッキング専用のハードウェアは必要ありません。というか対応していません。

## 想定ユーザー

- TwitchなどでVStreamerとしてアバターをフェイストラッキングで配信している民
- 特に配信はしていなくてもアバターで楽しんでいる民
- ETS2/ATSが好きな民、特にハンドルコントローラーを使っていて視線（カメラ）操作に難儀している民

## 一般的な使い方

1. `unvet-desktop.exe` を起動する。
2. `Input Source` で "VMC / OSC UDP" など入力したい元を選択する。
    - iFacialMocapを使う場合は基本的には "iFacialMocap UDP" を選択
    - WaidayoあるいはVMC対応アプリを使う場合は "VMC / OSC UDP" を選択
      - 受信するポート番号を変更したい場合は VMC/OSC UDP Port で指定
3. `Output Backend` で "ETS2 / ATS" など出力したい先を選択する
4. フェイストラッキングアプリのリセットやキャリブレーション等があれば行う
5. 正面（中心）を向いた状態でUNVETの `Recalibrate Neutral Pose` ボタンを押下
6. ETS2/ATSなどの対応ゲームを起動してゲーム内でヘッドトラッキングを有効にするなどして楽しむ
    - ETS2/ATSの場合はゲーム内のキーバインドでヘッドトラッキングON/OFFを切り替えできるようにすると便利です
    - 画面に対していまひとつ視線入力があっていないと感じた場合は、 `Axis Tuning` で調整してみてください。入力ソースによってYaw, Pitchが反転したり、お使いの画面解像度などによってYaw, Pitchに適度な倍率をかけるといい感じになるかもしれません。
    - 入力ソース側の安定性に応じて `Output Easing` を調整するのもよいかもしれません。

※UNVETには出力クラッチ機能があります。デフォルトではCTRL+SHIFT+Eで出力するかどうかをトグルできます。Absolute Pointer出力などで解除したくなった際に使うとよいかもしれません。

## おすすめの入力ソース

### Waidayo; Face ID に対応した世代の iPhone/iPad を使える場合

FaceID に対応した世代の iPhone/iPad を使える場合はおそらく最高の選択肢です。実際に試した中で最も安定していて高精度に思い通りの動作を実現できました。iFacialMocapも悪くないですが、Waidayoの方がかなり安定したフェイストラッキングの入力ソースとして使え、必要に応じてUNVET以外にも複数のアプリで同時受信できる点も嬉しい場合が多いです。

### そこそこ性能の良い Webcam + VMC対応アプリ; (WebcamMotionCapture/VSeeFace/etc.)

そこそこ性能の良い Webcam とは、グローバルシャッター方式で640x480以上の解像度で60fps以上のフレームレートを出せて、かつお使いの環境に合わせてしっかり顔を捉えてフェイストラッキングに適した設置がされた状態のカメラを指します。

もちろん、ローリングシャッター方式で320x240程度の解像度で30fps程度のフレームレートのカメラでも大抵のWebcamフェイストラッキング対応アプリで動作は可能ですが、あまり精度よくトラッキングできないと思います。

恐らくアプリ選びよりもカメラ選びの方が重要なポイントになります。

アプリ側でも例えばWebcamMotionCaptureであれば「トラッキングスムーズ」などの設定を調整したり、各種フェイストラッキング関連の調整を行うことである程度はカメラ性能をカバーできます。また、NVIDIA Broadcastなどの映像フィルタリングアプリを組み合わせるなどして、できるだけ安定して顔を捉えられるようにするのも効果的な場合があります。

## `unvet-desktop` と `unvet-cli` の違い

- desktop: GUIで操作できるコントロールパネルアプリ。入力ソースや出力バックエンドの切り替え、リアルタイムのトラッキング状態の表示などが可能。少し重い。
- cli: クロイガメンで動いてログがでるマニアック版。desktop版で設定が完成済みであとはとにかく軽く動かしたい場合などに便利かもしれはい。少し軽い。

どちらも同じ設定ファイルを読み込んで同じ動作をします。リッチなGUIが欲しいかどうかで使い分けて下さい。よくわからない民はとりあえずdesktop版を使いましょう。

## NPClient/TrackIR 互換レイヤーについて

ETS2/ATSのヘッドトラッキングは、もともとはNPClientというソフトウェアを介してTrackIRというハードウェアからの入力を受け取る形で実装されています。UNVETがETS2/ATSのヘッドトラッキング出力に対応するために、このNPClient/TrackIRのプロトコルレベルでの互換性を実装する必要がありました。

この互換レイヤーは、UNVETのコア処理とは独立したモジュールとして実装されており、UNVETのETS2/ATS出力バックエンドがこの互換レイヤーを介してゲームと通信する形になります。これにより、UNVETはNPClient/TrackIRと同等のインターフェースを提供しつつ、独自のコア処理や将来的な拡張も柔軟に行えるようになっています。また、互換レイヤーのインストールはUNVETの実行時に自動的に行われるため、ユーザーは特別な手順を踏むことなくETS2/ATSでのヘッドトラッキングを利用できます。

互換レイヤーのアンインストールには `unvet-uninstall-compatible-layers.exe` を利用できます。

- 実行場所: `unvet-desktop.exe` と同じフォルダー
- 権限: 通常は管理者権限不要（HKCU 配下のみ操作）
- 動作: このインストール先を指している互換レジストリーキーだけを削除し、同梱互換ファイル（`NPClient64.dll` / `NPClient.dll` / `TrackIR.exe`）を削除
- 事前確認: `unvet-uninstall-compatible-layers.exe --dry-run`

## Acknowledgements

This project was inspired by head-tracking tools such as opentrack:
<https://github.com/opentrack/opentrack> (ISC License)

UNVET is an independent implementation written in Rust.
This repository does not include opentrack source files.
Some interoperability behaviors and protocol-level constants are intentionally
implemented for compatibility with established head-tracking ecosystems.
For attribution and licensing details related to this compatibility work, see
THIRD_PARTY_NOTICES.md.

## License

[MIT](LICENSE)

## Author

[usagi / USAGI.NETWORK](https://usagi.network)
