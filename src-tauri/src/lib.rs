pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
#[cfg(test)]
mod tests {
    #[test]
    fn application_version() {
        assert_eq!(super::version(), "0.1.0");
    }
}
pub mod config;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod excel;
pub mod lifecycle;
pub fn hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(text.as_bytes()))
}
pub mod commands;
pub mod db;
pub mod documents;
pub mod engine;
pub mod importer;
pub mod indexing;
pub mod sync;
