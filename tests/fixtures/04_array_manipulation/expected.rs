fn main() {
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    let doubled: Vec<i64> = numbers
        .iter()
        .filter(|n| *n % 2 == 0)
        .map(|n| n * 2)
        .collect();

    let result: Vec<String> = doubled.iter().map(|n| n.to_string()).collect();
    print!("{}", result.join(", "));
}
