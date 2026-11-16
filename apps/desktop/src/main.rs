use gpui_kit::{
    AppContext as _, Bounds, Menu, MenuItem, WindowBounds, WindowOptions, application,
    component::TitleBar, init, open_window, px, size,
};

mod assets;
mod auth;
mod biometric_service;
mod components;
mod layout;
mod login;
mod mock_data;
mod pages;
mod reports_service;
mod services;
mod settings_store;
mod state;
mod theme;

use crate::{
    layout::{
        FocusGlobalSearch, GoDashboard, GoSettings, QuitApp, RunDiagnostics, ShowAbout,
        ToggleSidebar, ToggleTheme,
    },
    services::DesktopError,
};

fn main() {
    application()
        .with_assets(assets::AppAssets)
        .run(|cx| {
            init(cx);
            cx.bind_keys([gpui_kit::KeyBinding::new("ctrl-k", FocusGlobalSearch, None)]);

            cx.set_menus([
                Menu::new("Archivo").items([
                    MenuItem::action("Inicio (Dashboard)", GoDashboard),
                    MenuItem::action("Configuración", GoSettings),
                    MenuItem::separator(),
                    MenuItem::action("Salir", QuitApp),
                ]),
                Menu::new("Ver").items([
                    MenuItem::action("Alternar menú lateral", ToggleSidebar),
                    MenuItem::action("Cambiar tema claro / oscuro", ToggleTheme),
                    MenuItem::separator(),
                    MenuItem::action("Verificar entorno local", RunDiagnostics),
                ]),
                Menu::new("Ayuda").items([MenuItem::action("Acerca de Comedor", ShowAbout)]),
            ]);

            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(360.), px(320.))),
                ..TitleBar::window_options()
            };

            if let Err(error) =
                open_window(options, cx, |window, cx| cx.new(|cx| layout::DesktopApp::new(window, cx)))
            {
                let typed = DesktopError::WindowCreation(format!("{error:#}"));
                eprintln!("{typed}");
                cx.quit();
            }
        });
}
