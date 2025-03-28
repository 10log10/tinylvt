const DATABASE_URL: &str = "postgresql://user:password@localhost:5432/tinylvt";

// sqlx migrate add -r add_tables

#[tokio::main]
async fn main() {
    println!("hello world");
}
