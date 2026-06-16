// Copyright (C) 2025 Dxian998
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![windows_subsystem = "windows"]

mod app;
mod replacer;
mod remover;
mod utils;

use app::SteamIDApp;
use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([540.0, 476.0])
            .with_min_inner_size([520.0, 476.0])
            .with_drag_and_drop(true)
            .with_maximized(false)
            .with_maximize_button(false)
            .with_icon(utils::load_icon())
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "Stellar Blade SteamID Tool",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(SteamIDApp::default()))
        }),
    )
}