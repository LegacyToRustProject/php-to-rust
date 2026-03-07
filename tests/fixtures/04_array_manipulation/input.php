<?php
$numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

$evens = array_filter($numbers, function($n) {
    return $n % 2 === 0;
});

$doubled = array_map(function($n) {
    return $n * 2;
}, $evens);

echo implode(", ", $doubled);
