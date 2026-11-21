# PosterCraft (Rust)
Crop, remove backgrounds, slice an image across sheets of paper, export PDF and print.

Build on Windows: install Rust (rustup.rs) and NSIS (nsis.sourceforge.io), then

    cargo run --release                      # try the app
    cargo build --release
    "C:\Program Files (x86)\NSIS\makensis.exe" installer\installer.nsi   # -> installer\PosterCraft-Setup-1.0.0.exe

Or push to GitHub and run the "Build Windows installer" workflow.
