fn id<T>(x: T) -> T {
    x
}

fn main() {
    println!("{}", id(4) + id(4));
}
