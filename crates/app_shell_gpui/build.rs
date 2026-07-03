// Static ffmpeg references platform system libraries the ffmpeg-sys crate
// does not emit. Windows: Schannel/SSPI, CryptoAPI, Media Foundation,
// DirectShow GUIDs. macOS: the AV/Security frameworks ffmpeg links against.
fn main() {
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("windows") => {
            for lib in [
                "secur32", "crypt32", "ncrypt", "mfuuid", "strmiids", "ole32", "user32", "mfplat",
            ] {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
        Ok("macos") => {
            for framework in [
                "CoreFoundation",
                "CoreServices",
                "CoreMedia",
                "CoreVideo",
                "CoreAudio",
                "AudioToolbox",
                "VideoToolbox",
                "Security",
            ] {
                println!("cargo:rustc-link-lib=framework={framework}");
            }
        }
        _ => {}
    }
}
