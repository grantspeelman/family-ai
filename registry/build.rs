// Rebuild when a migration is added or changed: `embed_migrations!` embeds the
// files in `migrations/` at compile time.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
