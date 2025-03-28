//! Script to hash an email, to demo this functionality in Rust

use sha2::{Digest, Sha256};
use std::io::{self, Write};

fn sha256_email(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn main() {
    print!("Enter an email address: ");
    io::stdout().flush().unwrap();

    let mut email = String::new();
    io::stdin()
        .read_line(&mut email)
        .expect("Failed to read input");

    let hashed = sha256_email(&email);
    println!("SHA-256 hash: {}", hashed);
}
