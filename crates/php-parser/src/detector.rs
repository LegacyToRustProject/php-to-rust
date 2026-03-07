use crate::types::{Framework, PhpVersion};
use regex::Regex;
use std::sync::LazyLock;

static UNION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\)\s*:\s*\w+\|\w+").expect("invalid UNION_RE"));

static TYPED_PROP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(public|protected|private)\s+(readonly\s+)?\w+\s+\$")
        .expect("invalid TYPED_PROP_RE")
});

static RETURN_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\)\s*:\s*\??\w+").expect("invalid RETURN_TYPE_RE"));

static SCALAR_TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"function\s+\w+\s*\(\s*(int|string|float|bool)\s").expect("invalid SCALAR_TYPE_RE")
});

/// Detect PHP version from source code indicators.
pub fn detect_version(sources: &[&str]) -> PhpVersion {
    let mut has_typed_properties = false;
    let mut has_union_types = false;
    let mut has_strict_types = false;
    let mut has_return_types = false;
    let mut has_scalar_types = false;
    let mut has_namespaces = false;

    for source in sources {
        if source.contains("declare(strict_types=1)")
            || source.contains("declare(strict_types = 1)")
        {
            has_strict_types = true;
        }
        if UNION_RE.is_match(source) {
            has_union_types = true;
        }
        if source.contains("match(") || source.contains("match (") {
            has_union_types = true;
        }
        if TYPED_PROP_RE.is_match(source) {
            has_typed_properties = true;
        }
        if RETURN_TYPE_RE.is_match(source) {
            has_return_types = true;
        }
        if SCALAR_TYPE_RE.is_match(source) {
            has_scalar_types = true;
        }
        if source.contains("namespace ") {
            has_namespaces = true;
        }
    }

    if has_union_types || has_strict_types {
        PhpVersion::Php8
    } else if has_typed_properties || has_return_types || has_scalar_types {
        PhpVersion::Php7
    } else if has_namespaces {
        PhpVersion::Php5
    } else {
        PhpVersion::Unknown
    }
}

/// Detect framework from source code patterns.
pub fn detect_framework(sources: &[&str]) -> Option<Framework> {
    let mut wp_score = 0;
    let mut laravel_score = 0;
    let mut symfony_score = 0;

    let wp_patterns = [
        "add_action",
        "add_filter",
        "wp_",
        "WP_",
        "get_option",
        "update_option",
        "the_content",
        "the_title",
        "wp_enqueue_",
        "register_activation_hook",
        "plugins_url",
        "ABSPATH",
    ];

    let laravel_patterns = [
        "Route::",
        "Eloquent",
        "Illuminate\\",
        "artisan",
        "->middleware(",
        "App\\Http",
        "App\\Models",
        "use App\\",
    ];

    let symfony_patterns = [
        "Symfony\\",
        "AbstractController",
        "#[Route(",
        "->createQueryBuilder(",
        "Doctrine\\",
    ];

    for source in sources {
        for pattern in &wp_patterns {
            if source.contains(pattern) {
                wp_score += 1;
            }
        }
        for pattern in &laravel_patterns {
            if source.contains(pattern) {
                laravel_score += 1;
            }
        }
        for pattern in &symfony_patterns {
            if source.contains(pattern) {
                symfony_score += 1;
            }
        }
    }

    let max = wp_score.max(laravel_score).max(symfony_score);
    if max == 0 {
        return None;
    }

    if wp_score == max {
        Some(Framework::WordPress)
    } else if laravel_score == max {
        Some(Framework::Laravel)
    } else {
        Some(Framework::Symfony)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_wordpress() {
        let src = "<?php add_action('init', 'my_func'); wp_enqueue_script('app');";
        assert_eq!(detect_framework(&[src]), Some(Framework::WordPress));
    }

    #[test]
    fn detect_laravel() {
        let src = "<?php use Illuminate\\Http\\Request; Route::get('/', fn() => view('welcome'));";
        assert_eq!(detect_framework(&[src]), Some(Framework::Laravel));
    }

    #[test]
    fn detect_php8() {
        let src = "<?php declare(strict_types=1); function foo(): int|string {}";
        assert_eq!(detect_version(&[src]), PhpVersion::Php8);
    }

    #[test]
    fn detect_php7() {
        let src = "<?php function foo(int $x): string { return (string)$x; }";
        assert_eq!(detect_version(&[src]), PhpVersion::Php7);
    }

    #[test]
    fn detect_unknown() {
        let src = "<?php echo 'hello';";
        assert_eq!(detect_version(&[src]), PhpVersion::Unknown);
    }
}
