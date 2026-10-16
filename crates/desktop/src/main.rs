//! Ponto de entrada do binário desktop do LearnVault.
// Em release no Windows, mantém o processo sem console anexado.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    learnvault_desktop::run();
}
