<?php
/**
 * Plugin Name: My Counter Plugin
 * Description: A simple counter shortcode plugin
 * Version: 1.0.0
 */

if (!defined('ABSPATH')) {
    exit;
}

class My_Counter_Plugin {
    private static ?My_Counter_Plugin $instance = null;

    public static function get_instance(): self {
        if (self::$instance === null) {
            self::$instance = new self();
        }
        return self::$instance;
    }

    public function init(): void {
        add_shortcode('counter', [$this, 'render_counter']);
        add_action('wp_enqueue_scripts', [$this, 'enqueue_assets']);
    }

    public function render_counter(array $atts): string {
        $atts = shortcode_atts([
            'start' => 0,
            'step'  => 1,
            'label' => 'Count',
        ], $atts);

        $start = intval($atts['start']);
        $step  = intval($atts['step']);
        $label = esc_html($atts['label']);

        return sprintf(
            '<div class="my-counter" data-start="%d" data-step="%d">
                <span class="counter-label">%s</span>
                <span class="counter-value">%d</span>
                <button class="counter-btn">+</button>
            </div>',
            $start,
            $step,
            $label,
            $start
        );
    }

    public function enqueue_assets(): void {
        wp_enqueue_style(
            'my-counter-style',
            plugins_url('assets/style.css', __FILE__)
        );
        wp_enqueue_script(
            'my-counter-script',
            plugins_url('assets/counter.js', __FILE__),
            [],
            '1.0.0',
            true
        );
    }
}

My_Counter_Plugin::get_instance()->init();
