// Release builds run without a console window; debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use wgpu_gs_viewer::run;

fn main() {
    run().unwrap();
}
