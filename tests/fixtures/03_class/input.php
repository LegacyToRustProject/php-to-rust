<?php
class User {
    private string $name;
    private int $age;

    public function __construct(string $name, int $age) {
        $this->name = $name;
        $this->age = $age;
    }

    public function getName(): string {
        return $this->name;
    }

    public function getAge(): int {
        return $this->age;
    }

    public function greet(): string {
        return "Hello, I'm " . $this->name . " and I'm " . $this->age . " years old.";
    }
}

$user = new User("Alice", 30);
echo $user->greet();
