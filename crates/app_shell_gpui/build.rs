// Static ffmpeg (avformat/avcodec) references Windows system libraries the
// ffmpeg-sys crate does not emit: Schannel/SSPI, CryptoAPI, Media
// Foundation, and DirectShow interface GUIDs.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        for lib in [
            "secur32", "crypt32", "ncrypt", "mfuuid", "strmiids", "ole32", "user32", "mfplat",
        ] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}
