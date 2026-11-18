//! Entry point of LearnVault's desktop binary.
// In a Windows release, it keeps the process without an attached console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    learnvault_desktop::run();
}
