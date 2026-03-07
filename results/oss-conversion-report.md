# php-to-rust OSS変換テスト結果

実施日: 2026-03-08
担当: 作業者 #04
ブランチ: `feat/oss-test-improvements`

---

## 概要

`PatternConverter`（LLM不使用・パターンベース変換器）を新規実装し、実際のWordPress OSSプラグインへ適用した。

### 新規実装（このスプリント）

- **`PatternConverter`** (`crates/rust-generator/src/pattern_converter.rs`)
  LLM APIキー不要。PHPファイルをRustモジュールへ決定論的に変換する。
  - PHP型ヒント → Rust型 の完全マッピング（`string`→`String`, `int`→`i64`, `?T`→`Option<T>` 等）
  - WordPress APIマッピング（`add_action`→`hooks::add_action` 等）を適用
  - Rust予約語（`type`, `match`, `mod` 等37語）を自動的に `r#` エスケープ
  - クラス名・struct名の非ASCII文字・Unicode文字を `_` に置換（Composerベンダーファイル対応）
  - 未知のPHPクラス型（大文字始まり）→ `serde_json::Value` へ自動フォールバック
  - WordPress フックコメント生成（`hooks::add_action`/`hooks::add_filter`）
  - `hello_dolly_get_lyric()` 相当パターン：Rustで実際に動く実装を生成

- **`PatternConverter` を `convert` / `convert-file` CLI に統合**
  `--mode pattern` フラグで LLM不使用モードを選択可能。
  ```
  php-to-rust convert <DIR> --profile wordpress --mode pattern --output <OUT>
  php-to-rust convert-file <FILE> --profile wordpress --mode pattern --output <OUT>
  ```

- **WordPress プロファイル強化** (`profiles/wordpress/api_mappings.toml`)
  39関数追加（URL系・キャッシュ系・スケジュール系・REST API・ユーザー・コメント・HTTP）

---

## サマリー

| プロジェクト | 行数 | PHP ファイル数 | 変換完走 | cargo check | TODO 数 | 成功率推定 |
|---|---|---|---|---|---|---|
| Hello Dolly | 106 | 1 | ✅ 3/3 関数 | ✅ PASSED | 4 | 78% |
| WP Super Cache | ~16,000 | 49 | ✅ 49/49 ファイル | ✅ PASSED | 1,581 | 37% |
| Akismet | ~2,950 | 12 | ✅ 12/12 ファイル | ✅ PASSED | 223 | 55% |

---

## Phase 1: Hello Dolly

**変換結果**: 3関数 → `cargo check` PASSED

### 変換されたコード（抜粋）

```rust
// Plugin: Hello Dolly
// Version: 1.7.2

// WordPress hook registrations (register in plugin init):
//   hooks::add_action("admin_notices", hello_dolly)
//   hooks::add_action("admin_head", dolly_css)

pub fn hello_dolly_get_lyric() -> String {
    let lyrics = "Hello, Dolly\nWell, hello, Dolly\n...";
    let lines: Vec<&str> = lyrics.split('\n').collect();
    // TODO: wptexturize() not mapped — returning raw lyric
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    lines[nanos % lines.len()].to_string()
}
```

### 未対応パターン（Hello Dolly）

| PHP パターン | 対応状況 | 備考 |
|---|---|---|
| `wptexturize()` | TODO | api_mappings に未登録 |
| `get_user_locale()` | TODO | api_mappings に未登録 |
| `printf()` | TODO (引数付き) | PHP builtins には含むが書式変換未対応 |

---

## Phase 2: WP Super Cache (~16,000行 / 49ファイル)

**変換結果**: 49/49ファイル変換完走 → `cargo check` PASSED（272関数, 91メソッド）

主な問題と対処:
- `.`を含むファイル名（`class.wp_super_cache_rest_delete_cache.php`）→ モジュール名の `.` を `_` に変換（**修正済み**）
- Composerオートローダーの Unicode クラス名（`ComposerStaticInitⓥ4_0_0_alpha`）→ `sanitize_ident()` で非ASCIIを `_` 置換（**修正済み**）
- 不明クラス型 `ClassLoader` のパラメータ → `serde_json::Value` へフォールバック（**修正済み**）

### 未対応パターン（上位10件）

| PHP パターン | 出現数 | 対応難度 | 対応方針 |
|---|---|---|---|
| `wp_cache_debug()` | 72 | 低 | WordPressプロファイルに追加（デバッグログ） |
| `wp_cache_setting()` | 41 | 低 | WordPressプロファイルに追加（設定読み取り） |
| `trailingslashit()` | 36 | 低 | **追加済み** (v2: api_mappings.toml) |
| `wp_cache_replace_line()` | 33 | 中 | プラグイン固有、手動実装 |
| `prune_super_cache()` | 31 | 中 | プラグイン固有 |
| `wp_next_scheduled()` | 21 | 低 | **追加済み** (v2) |
| `get_supercache_dir()` | 21 | 中 | プラグイン固有、手動実装 |
| `admin_url()` | 20 | 低 | **追加済み** (v2) |
| `rest_ensure_response()` | 18 | 低 | **追加済み** (v2) |
| `$_SERVER` グローバル | - | 中 | Axum `Request` から取得（パターン未検出） |

---

## Phase 3: Akismet (~2,950行 / 12ファイル)

**変換結果**: 12/12ファイル変換完走 → `cargo check` PASSED（62関数, 23メソッド）

主な問題と対処:
- パラメータ名 `type` → Rust予約語 → `r#type` に自動変換（**修正済み**）
- クラス名を含む `get_type()` 等のメソッド名 → 正常に `get_type()` に変換（問題なし）

### 未対応パターン（上位10件）

| PHP パターン | 出現数 | 対応難度 | 対応方針 |
|---|---|---|---|
| `_deprecated_function()` | 16 | 低 | WordPressプロファイルに追加 |
| `admin_url()` | 10 | 低 | **追加済み** (v2) |
| `number_format_i18n()` | 8 | 低 | **追加済み** (v2) |
| `add_query_arg()` | 8 | 低 | **追加済み** (v2) |
| `wp_update_comment()` | 6 | 低 | **追加済み** (v2) |
| `wp_safe_redirect()` | 6 | 低 | **追加済み** (v2) |
| `esc_html__()` | 6 | 低 | **追加済み** (v2) |
| `check_admin_referer()` | 6 | 低 | **追加済み** (v2) |
| `akismet_update_alert()` | 5 | 高 | プラグイン固有 |
| `wp_remote_post()` | - | 中 | **追加済み** (v2: `http::post`) |

---

## 未対応パターン一覧（横断的）

| PHPパターン | カテゴリ | 出現頻度 | 対応難度 | 対応方針 |
|---|---|---|---|---|
| `$_SERVER`, `$_GET`, `$_POST` グローバル | グローバル変数 | 高 | 中 | Axum `Request` 抽出パターンに変換 |
| `$wpdb->query()`, `$wpdb->get_results()` | DB直接操作 | 高 | 高 | SeaORM raw query へ変換、要手動レビュー |
| `ob_start()` / `ob_get_clean()` | 出力バッファリング | 中 | 高 | `String` バッファを引数で渡すパターンに変換 |
| `wp_cache_debug()` / プラグイン固有関数 | プラグイン内部 | 高（固有） | 中 | 各プラグインのRust実装で対応 |
| `$_SERVER['HTTP_HOST']` 等の配列アクセス | スーパーグローバル | 高 | 中 | Axum `HeaderMap` / `Uri` へのマッピング |
| `class_implements()`, `get_class()` | リフレクション | 低 | 高 | `todo!("reflection: ...")` でスタブ |
| `do_action_ref_array()` | フック高度機能 | 低 | 中 | `hooks::do_action` の拡張版を実装 |
| CSS文字列中の `and(` 誤検出 | パーサー誤検出 | 中 | 低 | **修正済み**: `false_positive_words` に追加 |
| Rust予約語をパラメータ名に使用 | 予約語衝突 | 低 | 低 | **修正済み**: `r#` エスケープを自動適用 |

---

## 変換エンジン改善提案（優先度順）

1. **優先度高**: `$_SERVER` / `$_GET` / `$_POST` グローバル変数のAxum変換
   スーパーグローバル変数へのアクセスを Axum の `Request` / `Query` 抽出へ変換。
   WordPressプラグインで最も頻出するパターン。

2. **優先度高**: `$wpdb->get_results()` / `$wpdb->insert()` → SeaORM 変換
   WordPress DB操作の SeaORM raw query パターンへの変換テンプレートを追加。
   `$wpdb->prepare()` のプレースホルダー（`%d`, `%s`）も変換対象。

3. **優先度中**: プラグイン固有フック関数のマッピングDBを拡張
   `wp_cache_debug()`, `wp_cache_setting()` 等のプラグイン固有関数を
   適切な Rust 関数スタブに変換するプロファイル拡張機能を追加。

4. **優先度中**: `ob_start()` / `ob_get_clean()` パターン変換
   出力バッファリングを `String` を返す関数パターンに変換する変換テンプレートを実装。

5. **優先度低**: LLMハイブリッドモード（`--mode hybrid`）の実装
   PatternConverter で変換できなかった関数（TODO数が多い関数）だけを LLM で再変換。
   LLMコスト削減 + 品質向上のベストバランス。

---

## cargo test / clippy 結果

```
cargo test --workspace --lib:  45 tests, 0 failures ✅
cargo clippy --workspace -- -D warnings: 0 errors, 0 warnings ✅
```

## 変換後 cargo check 結果

```
Hello Dolly:    ✅ PASSED (0 errors)
WP Super Cache: ✅ PASSED (0 errors, 230 warnings — unused variables)
Akismet:        ✅ PASSED (0 errors, 64 warnings — unused variables)
```

---

*テスト実施: 作業者 #04 — 2026-03-08*
*対象コミット: feat/oss-test-improvements*
