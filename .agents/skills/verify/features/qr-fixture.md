# Encoded QR fixture

Run `./target/debug/falcon x qr-check --video VIDEO --manifest MANIFEST --output-dir NEW_DIRECTORY` on macOS with ffmpeg and Swift/Xcode installed. The video is the optical stimulus, and the manifest supplies expected QR payloads and holds. Inspect extracted PNGs, exact decoded payloads and input hashes in summary.json. Tool failure, no barcode or mismatch must fail. Reusing an evidence directory must fail without modifying prior evidence.

Then run the app-owned test with `x verify --physical-camera`. Physical device metadata and reviewed native optical assertions are separate requirements. A fixture pass cannot establish phone scanning. See [contract](../../../../docs/native-verification.md).
