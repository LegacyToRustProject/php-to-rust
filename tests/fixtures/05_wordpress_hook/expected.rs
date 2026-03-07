use rustpress::hooks;
use rustpress::post_types;
use rustpress::context;

fn hello_init() {
    post_types::register("hello_message", PostTypeArgs {
        public: true,
        label: "Hello Messages".to_string(),
        ..Default::default()
    });
}

fn hello_content(content: &str) -> String {
    if context::is_single() && context::get_post_type() == "hello_message" {
        return format!("<div class=\"hello-wrap\">{}</div>", content);
    }
    content.to_string()
}

pub fn register(hooks: &mut hooks::HookRegistry) {
    hooks.add_action("init", hello_init, 10);
    hooks.add_filter("the_content", hello_content, 10);
}
