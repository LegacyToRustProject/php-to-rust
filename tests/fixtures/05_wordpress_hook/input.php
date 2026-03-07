<?php
/**
 * Plugin Name: Hello Plugin
 * Description: A simple WordPress plugin
 */

function hello_init() {
    register_post_type('hello_message', [
        'public' => true,
        'label'  => 'Hello Messages',
    ]);
}
add_action('init', 'hello_init');

function hello_content($content) {
    if (is_single() && get_post_type() === 'hello_message') {
        return '<div class="hello-wrap">' . $content . '</div>';
    }
    return $content;
}
add_filter('the_content', 'hello_content');
