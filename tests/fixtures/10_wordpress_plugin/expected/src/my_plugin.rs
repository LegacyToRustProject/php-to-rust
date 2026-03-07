use std::sync::OnceLock;

/// Converted from PHP class `My_Counter_Plugin`.
/// Original: A simple counter shortcode plugin (v1.0.0).
#[derive(Debug)]
pub struct MyCounterPlugin;

static INSTANCE: OnceLock<MyCounterPlugin> = OnceLock::new();

impl MyCounterPlugin {
    /// Singleton access (converted from `get_instance()`).
    pub fn get_instance() -> &'static MyCounterPlugin {
        INSTANCE.get_or_init(|| MyCounterPlugin)
    }

    /// Register hooks (converted from `init()`).
    pub fn init(&self, hooks: &mut hooks::HookRegistry) {
        hooks.add_shortcode("counter", |atts| self.render_counter(atts));
        hooks.add_action("wp_enqueue_scripts", |_| self.enqueue_assets(), 10);
    }

    /// Render the counter shortcode HTML (converted from `render_counter()`).
    pub fn render_counter(&self, atts: &ShortcodeAtts) -> String {
        let start = atts.get_i64("start", 0);
        let step = atts.get_i64("step", 1);
        let label = escape::html(atts.get_str("label", "Count"));

        format!(
            r#"<div class="my-counter" data-start="{start}" data-step="{step}">
                <span class="counter-label">{label}</span>
                <span class="counter-value">{start}</span>
                <button class="counter-btn">+</button>
            </div>"#,
        )
    }

    /// Enqueue frontend assets (converted from `enqueue_assets()`).
    pub fn enqueue_assets(&self) {
        assets::enqueue_style(
            "my-counter-style",
            &plugin::url("assets/style.css", file!()),
            &[],
            None,
        );
        assets::enqueue_script(
            "my-counter-script",
            &plugin::url("assets/counter.js", file!()),
            &[],
            Some("1.0.0"),
            true,
        );
    }
}

// --- Stub modules representing RustPress framework APIs ---
// In a real RustPress plugin, these would be imported from rustpress crates.

pub mod hooks {
    pub struct HookRegistry;
    impl HookRegistry {
        pub fn add_shortcode<F>(&mut self, _tag: &str, _callback: F)
        where
            F: Fn(&super::ShortcodeAtts) -> String + 'static,
        {
        }
        pub fn add_action<F>(&mut self, _hook: &str, _callback: F, _priority: i32)
        where
            F: Fn(&()) + 'static,
        {
        }
    }
}

pub mod escape {
    pub fn html(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
}

pub mod assets {
    pub fn enqueue_style(_handle: &str, _src: &str, _deps: &[&str], _ver: Option<&str>) {}
    pub fn enqueue_script(
        _handle: &str,
        _src: &str,
        _deps: &[&str],
        _ver: Option<&str>,
        _in_footer: bool,
    ) {
    }
}

pub mod plugin {
    pub fn url(relative: &str, _file: &str) -> String {
        format!("/wp-content/plugins/my-counter-plugin/{}", relative)
    }
}

/// Shortcode attributes (WordPress `shortcode_atts` equivalent).
#[derive(Debug, Default)]
pub struct ShortcodeAtts {
    inner: std::collections::HashMap<String, String>,
}

impl ShortcodeAtts {
    pub fn get_str<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.inner.get(key).map(|s| s.as_str()).unwrap_or(default)
    }

    pub fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.inner
            .get(key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_counter_defaults() {
        let plugin = MyCounterPlugin;
        let atts = ShortcodeAtts::default();
        let html = plugin.render_counter(&atts);
        assert!(html.contains(r#"data-start="0""#));
        assert!(html.contains(r#"data-step="1""#));
        assert!(html.contains("Count"));
        assert!(html.contains("counter-btn"));
    }

    #[test]
    fn test_render_counter_custom_atts() {
        let plugin = MyCounterPlugin;
        let mut atts = ShortcodeAtts::default();
        atts.inner.insert("start".to_string(), "5".to_string());
        atts.inner.insert("step".to_string(), "2".to_string());
        atts.inner.insert("label".to_string(), "Score".to_string());
        let html = plugin.render_counter(&atts);
        assert!(html.contains(r#"data-start="5""#));
        assert!(html.contains(r#"data-step="2""#));
        assert!(html.contains("Score"));
    }

    #[test]
    fn test_render_counter_xss_escape() {
        let plugin = MyCounterPlugin;
        let mut atts = ShortcodeAtts::default();
        atts.inner
            .insert("label".to_string(), "<script>alert(1)</script>".to_string());
        let html = plugin.render_counter(&atts);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_singleton() {
        let a = MyCounterPlugin::get_instance();
        let b = MyCounterPlugin::get_instance();
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn test_plugin_url() {
        let url = plugin::url("assets/style.css", file!());
        assert!(url.ends_with("my-counter-plugin/assets/style.css"));
    }
}
