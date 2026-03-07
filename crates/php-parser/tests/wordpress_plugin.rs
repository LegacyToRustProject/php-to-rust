/// Integration test: verifies the WordPress plugin fixture can be analyzed
/// and that the expected Rust output structure is valid.
use php_parser::{analyze_project, types::Framework};
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    // cargo test runs from the crate root; fixtures are at workspace root
    Path::new("../../tests/fixtures/10_wordpress_plugin")
}

#[test]
fn analyze_wordpress_plugin_fixture() {
    let input_dir = fixtures_dir().join("input");
    let project = analyze_project(&input_dir).expect("Failed to analyze WordPress plugin fixture");

    // Framework detection
    assert_eq!(project.framework, Some(Framework::WordPress));

    // File count
    assert_eq!(project.files.len(), 1, "Expected exactly 1 PHP file");

    // Class extraction
    let file = &project.files[0];
    assert_eq!(file.classes.len(), 1, "Expected 1 class");

    let class = &file.classes[0];
    assert_eq!(class.name, "My_Counter_Plugin");

    // Methods: get_instance, init, render_counter, enqueue_assets
    let method_names: Vec<&str> = class.methods.iter().map(|m| m.name.as_str()).collect();
    assert!(
        method_names.contains(&"get_instance"),
        "Missing get_instance"
    );
    assert!(method_names.contains(&"init"), "Missing init");
    assert!(
        method_names.contains(&"render_counter"),
        "Missing render_counter"
    );
    assert!(
        method_names.contains(&"enqueue_assets"),
        "Missing enqueue_assets"
    );
}

#[test]
fn expected_rust_output_exists_and_valid() {
    let expected_dir = fixtures_dir().join("expected");
    assert!(
        expected_dir.join("Cargo.toml").exists(),
        "Missing expected Cargo.toml"
    );
    assert!(
        expected_dir.join("src/lib.rs").exists(),
        "Missing expected src/lib.rs"
    );
    assert!(
        expected_dir.join("src/my_plugin.rs").exists(),
        "Missing expected src/my_plugin.rs"
    );

    // Verify the expected Rust code contains key converted constructs
    let rust_code = std::fs::read_to_string(expected_dir.join("src/my_plugin.rs")).unwrap();

    // Struct conversion: PHP class → Rust struct
    assert!(
        rust_code.contains("struct MyCounterPlugin"),
        "Expected struct MyCounterPlugin"
    );

    // Singleton pattern: static instance → OnceLock
    assert!(
        rust_code.contains("OnceLock"),
        "Expected OnceLock for singleton pattern"
    );

    // Method conversion
    assert!(
        rust_code.contains("fn render_counter"),
        "Expected render_counter method"
    );
    assert!(
        rust_code.contains("fn enqueue_assets"),
        "Expected enqueue_assets method"
    );

    // WordPress API mappings
    assert!(
        rust_code.contains("escape::html"),
        "Expected esc_html → escape::html mapping"
    );
    assert!(
        rust_code.contains("assets::enqueue_script"),
        "Expected wp_enqueue_script → assets::enqueue_script mapping"
    );
    assert!(
        rust_code.contains("assets::enqueue_style"),
        "Expected wp_enqueue_style → assets::enqueue_style mapping"
    );
    assert!(
        rust_code.contains("plugin::url"),
        "Expected plugins_url → plugin::url mapping"
    );
}
