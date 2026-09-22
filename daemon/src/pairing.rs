use sha2::{Digest, Sha256};

// Both sides compute the same code regardless of who's initiator —
// sorting the two keys first guarantees that.
pub fn fingerprint(my_pub: &[u8; 32], remote_pub: &[u8; 32]) -> String {
    let (first, second) = if my_pub <= remote_pub {
        (my_pub, remote_pub)
    } else {
        (remote_pub, my_pub)
    };
    let mut hasher = Sha256::new();
    hasher.update(first);
    hasher.update(second);
    let hash = hasher.finalize();

    // Truncate to 3 bytes -> 6-digit decimal code, easier to read
    // aloud/compare than hex.
    let code = u32::from_be_bytes([0, hash[0], hash[1], hash[2]]);
    format!("{:06}", code % 1_000_000)
}

// Extremely simple CLI confirmation for now — real UI comes later
// (Android side, or a desktop notification). This just proves the
// pairing flow works before any UI is built.
pub fn confirm_via_stdin(code: &str, remote_name: &str) -> bool {
    println!(
        "Pairing request from {} — verification code: {}",
        remote_name, code
    );
    println!("Does this match on both devices? [y/N]: ");

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    input.trim().eq_ignore_ascii_case("y")
}
