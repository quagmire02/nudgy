use std::fs;
use std::path::Path;

fn main() {
    let src = sqlcipher_src::source_dir();
    let headers = src.join(sqlcipher_src::LIBTOMCRYPT_INCLUDE_DIR);

    // libsqlite3-sys compiles SQLCipher against the vendored copy of these
    // headers (OPENSSL_INCLUDE_DIR); both sides must see the same libtomcrypt.
    let vendored = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/libtomcrypt/headers");
    for entry in fs::read_dir(&headers).expect("libtomcrypt headers") {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap();
        let ours = fs::read(vendored.join(name)).unwrap_or_default();
        assert!(
            ours == fs::read(&path).unwrap(),
            "third_party/libtomcrypt/headers/{} differs from sqlcipher-src {}; re-copy the headers",
            name.to_string_lossy(),
            sqlcipher_src::LIBTOMCRYPT_VERSION,
        );
    }

    cc::Build::new()
        .include(&headers)
        .define("LTC_SOURCE", None)
        .files(sqlcipher_src::LIBTOMCRYPT_SOURCES.iter().map(|s| src.join(s)))
        .warnings(false)
        // Named libcrypto so libsqlite3-sys's `dylib=libcrypto` link resolves to it.
        .compile("libcrypto");
    // libtomcrypt's RNG uses CryptGenRandom on Windows.
    println!("cargo:rustc-link-lib=advapi32");
    println!("cargo:rerun-if-changed=../../third_party/libtomcrypt/headers");
}
