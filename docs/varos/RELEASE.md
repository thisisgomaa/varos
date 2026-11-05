# Building a Varos preview release

Status: recipe used for 0.1 Preview (2026-11-05). Run from a clean, pushed `main`.

1. Gates: `CARGO_BUILD_JOBS=4 cargo xtask ci` must pass (all steps, including `layers` and `wasm`).
2. Build with the product version and **source paths remapped**, so no local folder or user name is
   embedded in the binaries (Rust keeps source paths for panic messages). Use the encoded form;
   plain `RUSTFLAGS` splits on the space in folder names:

   ```sh
   export CARGO_ENCODED_RUSTFLAGS="$(printf '%s\x1f%s\x1f%s' \
     "--remap-path-prefix=$HOME/.cargo=/cargo" \
     "--remap-path-prefix=$HOME/.rustup=/rustup" \
     "--remap-path-prefix=$PWD=/varos")"
   VAROS_PRODUCT_VERSION=0.1.0-preview.1 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 cargo xtask bundle
   ```

   Check: `strings dist/Varos.app/Contents/MacOS/* | grep -c "$USER"` must print 0.
3. Bundle identity: `xtask bundle` writes the engine version; set the product version, a build
   number and the source commit, then sign the whole bundle ad hoc:

   ```sh
   plutil -replace CFBundleShortVersionString -string 0.1.0-preview.1 dist/Varos.app/Contents/Info.plist
   plutil -replace CFBundleVersion -string <yyyymmdd.n> dist/Varos.app/Contents/Info.plist
   plutil -replace VarosSourceCommit -string "$(git rev-parse HEAD)" dist/Varos.app/Contents/Info.plist
   codesign --force --deep --sign - dist/Varos.app && codesign --verify --deep --strict dist/Varos.app
   ```

4. Archive and checksum: `ditto -c -k --sequesterRsrc --keepParent dist/Varos.app
   Varos-<version>-arm64.zip`, then `shasum -a 256` it. Unzip into a temporary folder and verify the
   signature, version and architecture (`file`) from the unzipped copy.
5. Publish as a GitHub **pre-release** with the zip, its SHA-256 and bilingual notes. The app is not
   notarized: the notes must explain right-click → Open / Open Anyway.
